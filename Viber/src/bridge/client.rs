//! Cliente do debug bridge — JSON-RPC/BRP sobre HTTP/1.1 cru (std, sem deps).
//! Usado pelos subcomandos `viber debug …`.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use serde_json::{Value, json};

/// Cap da resposta lida do socket — um peer hostil que goteja bytes não
/// pode fazer OOM do CLI num `read_to_end` sem limite.
const MAX_RESPONSE: u64 = 256 << 20;

pub struct BridgeClient {
    pub host: String,
    pub port: u16,
}

impl BridgeClient {
    pub fn localhost(port: u16) -> Self {
        Self {
            host: "127.0.0.1".into(),
            port,
        }
    }

    /// POST JSON-RPC e devolve o campo `result` (bail no `error` BRP).
    pub fn call(&self, method: &str, params: Value) -> Result<Value> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": method,
            "params": params,
        });
        let body = serde_json::to_vec(&request)?;
        let mut stream = self.connect()?;
        let http = format!(
            "POST / HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.host,
            self.port,
            body.len()
        );
        stream.write_all(http.as_bytes())?;
        stream.write_all(&body)?;
        stream.flush()?;

        let mut raw = Vec::new();
        stream.take(MAX_RESPONSE).read_to_end(&mut raw)?;
        if raw.len() as u64 >= MAX_RESPONSE {
            bail!(
                "resposta do bridge excede o cap de {} MB",
                MAX_RESPONSE >> 20
            );
        }
        let (headers, response_body) = split_headers(&raw).context("resposta HTTP sem corpo")?;
        // Com `Connection: close` o corpo é o resto do stream; se vier
        // chunked, descodifica (tamanhos de chunk são em BYTES — decoder
        // byte-a-byte para não desalinhar com UTF-8 no payload).
        let chunked = is_chunked(headers);
        let payload = if chunked {
            String::from_utf8(decode_chunked(response_body)?).context("chunk inválido (UTF-8)")?
        } else {
            String::from_utf8_lossy(response_body).into_owned()
        };

        let status = http_status(headers);
        let parsed: Value = serde_json::from_str(payload.trim()).with_context(|| {
            if status == 200 {
                "resposta não é JSON-RPC".to_string()
            } else {
                format!("resposta não é JSON-RPC (bridge devolveu HTTP {status})")
            }
        })?;
        if let Some(error) = parsed.get("error") {
            bail!(
                "bridge error {}: {}",
                error.get("code").and_then(Value::as_i64).unwrap_or(0),
                error.get("message").and_then(Value::as_str).unwrap_or("?")
            );
        }
        Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Liga ao bridge com retry — o servidor HTTP do `bevy_remote` faz bind
    /// de forma assíncrona (task pool), logo logo após `viber run --bridge` o
    /// primeiro connect pode falhar (EAGAIN/refused).
    fn connect(&self) -> Result<TcpStream> {
        const ATTEMPTS: usize = 20;
        const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
        // Resolve 1× fora do loop — o retry existe para o bind assíncrono do
        // servidor, não para pagar DNS/connect bloqueado em cada tentativa.
        let addr = (self.host.as_str(), self.port)
            .to_socket_addrs()?
            .next()
            .context("endereço do bridge não resolve")?;
        let mut last = None;
        for attempt in 0..ATTEMPTS {
            match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
                Ok(stream) => {
                    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
                    return Ok(stream);
                }
                Err(error) => {
                    last = Some(error);
                    if attempt + 1 < ATTEMPTS {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        }
        bail!(
            "a ligar ao bridge {}:{} (a engine corre com `viber run --bridge`?): {}",
            self.host,
            self.port,
            last.expect("pelo menos uma tentativa")
        )
    }

    /// Pede uma captura e faz polling de `viber.screenshot_status` até o PNG
    /// chegar (a captura completa ao fim de ~1-3 frames do render).
    pub fn screenshot(&self, timeout_ms: u64) -> Result<(Vec<u8>, String)> {
        let request = self.call("viber.screenshot", json!({}))?;
        let id = request
            .get("id")
            .and_then(Value::as_u64)
            .context("resposta sem capture id")?;
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            if Instant::now() >= deadline {
                bail!("timeout ({timeout_ms} ms) à espera do screenshot");
            }
            std::thread::sleep(Duration::from_millis(50));
            // Um Err transitório num poll não deve abortar enquanto houver
            // deadline — só propaga quando o tempo esgotou.
            let status = match self.call("viber.screenshot_status", json!({ "id": id })) {
                Ok(status) => status,
                Err(error) => {
                    if Instant::now() < deadline {
                        continue;
                    }
                    return Err(error);
                }
            };
            match status.get("status").and_then(Value::as_str) {
                Some("captured") => {
                    let b64 = status
                        .get("png_base64")
                        .and_then(Value::as_str)
                        .context("captura sem png")?;
                    let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
                    let path = status
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                        .to_string();
                    return Ok((bytes, path));
                }
                Some("pending") => continue,
                Some("error") => {
                    // O servidor falhou a leitura do PNG — falhar já com a
                    // causa em vez de girar até timeout.
                    let cause = status
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("desconhecida");
                    bail!("captura falhou: {cause}");
                }
                other => bail!("estado inesperado da captura: {:?}", other),
            }
        }
    }

    pub fn screenshot_to_file(&self, output: &Path, timeout_ms: u64) -> Result<String> {
        let (bytes, source_path) = self.screenshot(timeout_ms)?;
        std::fs::write(output, &bytes)
            .with_context(|| format!("a escrever {}", output.display()))?;
        Ok(source_path)
    }

    pub fn probe(&self) -> Result<Value> {
        self.call("viber.ping", json!({}))
    }

    pub fn tree(&self) -> Result<Value> {
        self.call("viber.tree", json!({}))
    }

    pub fn logs(&self, limit: usize) -> Result<Value> {
        self.call("viber.logs", json!({ "limit": limit }))
    }

    pub fn prof(&self) -> Result<Value> {
        self.call("viber.profiler", json!({}))
    }

    /// Executa Luau na engine (`viber.lua`) — devolve
    /// `{ ok, result|error, applied, warnings }`.
    pub fn lua(&self, code: &str) -> Result<Value> {
        self.call("viber.lua", json!({ "code": code }))
    }

    pub fn key(&self, key: &str, text: Option<String>, shift: bool) -> Result<Value> {
        let mut params = json!({ "key": normalize_key(key) });
        if let Some(text) = text {
            params["text"] = json!(text);
        }
        if shift {
            params["shift"] = json!(true);
        }
        self.call("viber.input.key", params)
    }

    pub fn text(&self, text: &str) -> Result<Value> {
        self.call("viber.input.text", json!({ "text": text }))
    }

    pub fn click(&self, x: f32, y: f32, button: &str) -> Result<Value> {
        self.call(
            "viber.input.click",
            json!({ "x": x, "y": y, "button": normalize_mouse(button) }),
        )
    }

    pub fn move_cursor(&self, x: f32, y: f32) -> Result<Value> {
        self.call("viber.input.move", json!({ "x": x, "y": y }))
    }
}

/// Separa headers do corpo na resposta HTTP crua (bytes).
fn split_headers(raw: &[u8]) -> Option<(&[u8], &[u8])> {
    let index = raw.windows(4).position(|window| window == b"\r\n\r\n")?;
    Some((&raw[..index], &raw[index + 4..]))
}

/// Código HTTP do status line ("HTTP/1.1 200 OK" → 200; 0 se ilegível).
fn http_status(headers: &[u8]) -> u16 {
    String::from_utf8_lossy(headers)
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0)
}

/// `Transfer-Encoding: chunked` por linha de header — a busca por substring
/// perdia variantes legais sem espaço após `:` (`Transfer-Encoding:chunked`).
fn is_chunked(headers: &[u8]) -> bool {
    String::from_utf8_lossy(headers)
        .lines()
        .filter_map(|line| line.split_once(':'))
        .any(|(name, value)| {
            name.trim().eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
        })
}

/// Decode de corpo HTTP chunked byte-a-byte (tamanhos de chunk são em bytes).
fn decode_chunked(body: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(pos) = rest.windows(2).position(|window| window == b"\r\n") {
        let size_line = &rest[..pos];
        let size_text = std::str::from_utf8(size_line)
            .map_err(|_| anyhow::anyhow!("chunk size inválido"))?
            .split(';')
            .next()
            .unwrap_or("0")
            .trim();
        let size = usize::from_str_radix(size_text, 16).context("chunk size inválido")?;
        if size == 0 {
            break;
        }
        let start = pos + 2;
        // checked_add — um chunk size tipo "ffffffffffffffff" vindo da rede
        // faria wrap e panic no slice.
        let end = start
            .checked_add(size)
            .ok_or_else(|| anyhow::anyhow!("chunk size inválido"))?;
        if end > rest.len() {
            bail!("resposta chunked truncada");
        }
        out.extend_from_slice(&rest[start..end]);
        rest = &rest[end..];
        if rest.starts_with(b"\r\n") {
            rest = &rest[2..];
        }
    }
    Ok(out)
}

/// Normaliza aliases amigáveis para variantes serde do `KeyCode`.
/// Ex.: `a` → `KeyA`, `1` → `Digit1`, `up` → `ArrowUp`, `esc` → `Escape`.
#[must_use]
pub fn normalize_key(raw: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    let named: Option<&str> = match lower.as_str() {
        "space" | "spacebar" => Some("Space"),
        "enter" | "return" => Some("Enter"),
        "esc" | "escape" => Some("Escape"),
        "tab" => Some("Tab"),
        "backspace" => Some("Backspace"),
        "delete" | "del" => Some("Delete"),
        "insert" => Some("Insert"),
        "home" => Some("Home"),
        "end" => Some("End"),
        "pageup" => Some("PageUp"),
        "pagedown" => Some("PageDown"),
        "up" | "arrowup" => Some("ArrowUp"),
        "down" | "arrowdown" => Some("ArrowDown"),
        "left" | "arrowleft" => Some("ArrowLeft"),
        "right" | "arrowright" => Some("ArrowRight"),
        "shift" => Some("ShiftLeft"),
        "ctrl" | "control" => Some("ControlLeft"),
        "alt" => Some("AltLeft"),
        "capslock" => Some("CapsLock"),
        "f1" => Some("F1"),
        "f2" => Some("F2"),
        "f3" => Some("F3"),
        "f4" => Some("F4"),
        "f5" => Some("F5"),
        "f6" => Some("F6"),
        "f7" => Some("F7"),
        "f8" => Some("F8"),
        "f9" => Some("F9"),
        "f10" => Some("F10"),
        "f11" => Some("F11"),
        "f12" => Some("F12"),
        _ => None,
    };
    if let Some(name) = named {
        return name.to_string();
    }
    let mut chars = trimmed.chars();
    if let (Some(first), None) = (chars.next(), chars.next()) {
        if first.is_ascii_alphabetic() {
            return format!("Key{}", first.to_ascii_uppercase());
        }
        if let Some(digit) = first.to_digit(10) {
            return format!("Digit{digit}");
        }
    }
    trimmed.to_string()
}

fn normalize_mouse(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "left" | "l" => "Left".into(),
        "right" | "r" => "Right".into(),
        "middle" | "m" => "Middle".into(),
        "back" => "Back".into(),
        "forward" => "Forward".into(),
        other => other.to_string(),
    }
}

/// Resolve a porta do bridge: flag CLI → env `VIBER_BRIDGE_PORT` → engine
/// viva registada numa sessão → default.
///
/// O passo da sessão é o que faz `viber debug prof` funcionar sem ninguém
/// exportar `VIBER_BRIDGE_PORT`: desde que `session up` passou a escolher a
/// primeira porta livre, 15702 deixou de ser uma aposta segura. Uma única
/// engine viva ganha; com várias, a que corre o mundo do diretório atual
/// (senão, a de porta mais baixa, para ser determinístico).
pub fn resolve_port(flag: Option<u16>) -> u16 {
    if let Some(port) = flag {
        return port;
    }
    if let Some(raw) = std::env::var("VIBER_BRIDGE_PORT").ok() {
        match raw.parse::<u16>() {
            Ok(port) => return port,
            // Um typo não pode cair noutra engine em silêncio (mutaria o
            // mundo errado) — avisa e segue para a descoberta.
            Err(_) => eprintln!("viber: VIBER_BRIDGE_PORT inválido: `{raw}` — a ignorar"),
        }
    }
    if let Some((port, world)) = session_port() {
        eprintln!("viber: bridge descoberto em :{port} (sessão: {world})");
        return port;
    }
    super::DEFAULT_BRIDGE_PORT
}

/// TCP connect com timeout curto — descoberta não pode pagar os 2 s de
/// retry do `connect()` por sessão morta.
pub fn port_alive(port: u16) -> bool {
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_millis(250)).is_ok()
}

/// Porta de uma engine de sessão que responde ao `ping`.
/// Porta de uma engine viva registada no registo de sessões, com o mundo
/// (para a mensagem de descoberta). Pré-filtra por TCP rápido e confirma
/// com o probe HTTP pela ordem de preferência — a primeira que responde
/// ganha (uma engine wedged não esconde as saudáveis).
fn session_port() -> Option<(u16, String)> {
    let cwd = std::env::current_dir().ok();
    let mut live: Vec<(bool, u16, String)> = crate::session::SessionPaths::all()
        .iter()
        .filter_map(|(_, paths)| paths.engine_info())
        .filter(|engine| port_alive(engine.port))
        .map(|engine| {
            // Preferir a engine cujo mundo está debaixo do cwd — num
            // checkout com vários mundos é quase sempre a certa.
            let local = cwd
                .as_ref()
                .is_some_and(|cwd| std::path::Path::new(&engine.world).starts_with(cwd));
            (!local, engine.port, engine.world)
        })
        .collect();
    live.sort();
    for (_, port, world) in &live {
        if BridgeClient::localhost(*port).probe().is_ok() {
            return Some((*port, world.clone()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_alive_matches_tcp_listener() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        assert!(port_alive(port), "porta com listener devia estar viva");
        // Porta efémera provavelmente morta — o importante é ser RÁPIDO e
        // devolver false (sem os retries de 2 s do connect() do cliente).
        let start = std::time::Instant::now();
        let dead = port_alive(9); // discard — quase sempre fechado
        assert!(!dead, "porta 9 não devia estar viva");
        assert!(
            start.elapsed() < std::time::Duration::from_secs(1),
            "pre-check devia ser rápido, levou {:?}",
            start.elapsed()
        );
    }
}
