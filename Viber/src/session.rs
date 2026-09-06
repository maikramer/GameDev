//! Sessão partilhada de QA — uma engine por mundo, com lease de uso
//! ("liberado" / "ocupado, aguarde") para agentes paralelos partilharem a
//! mesma instância `viber run --bridge` sem esgotar a GPU (uma engine nova
//! por agente causou OOM de VRAM a 2026-09-02).
//!
//! Contrato (o mutex é o SO, não convenção):
//! - `<cache>/viber/session-<slug>/lease.json` publicado com hard link
//!   atómico (`publish_new`: só cria se o destino não existe, conteúdo
//!   COMPLETO desde o primeiro instante); dois `claim` concorrentes → um
//!   ganha, o outro recebe `Busy`.
//! - Lease com TTL (`expires_at`): agente que morre sem `release` não
//!   bloqueia ninguém — o próximo `claim` rouba o lease expirado.
//! - `engine.json` guarda pid/porta/mundo/log da engine partilhada; é
//!   independente do lease (a engine vive entre claims).
//!
//! CLI: `viber session status|up|down|claim|touch|release` (glue em `main.rs`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// TTL default do lease (s). QA de agente são 1–2 chamadas; 5 min dá folga
/// para screenshots + debug, e expira sozinho se o agente morrer.
pub const DEFAULT_TTL_SECS: u64 = 300;
/// Intervalo de polling do `claim --wait` (s).
const WAIT_POLL: Duration = Duration::from_secs(2);
/// "Restante" reportado quando o lease.json existe mas não é parseável
/// (verdadeiramente corrupto, ou resquício de estados antigos): curto de
/// propósito, para o poll do `claim --wait` re-tentar depressa.
const LEASE_PENDING: Duration = Duration::from_millis(200);

// ---------------------------------------------------------------- paths

/// Caminhos de uma sessão (um mundo = uma sessão). `for_base` existe para
/// testes; produção usa [`SessionPaths::for_world`].
pub struct SessionPaths {
    base: PathBuf,
}

impl SessionPaths {
    /// Sessão do mundo: `<cache>/viber/session-<slug>/`.
    pub fn for_world(world: &Path) -> Self {
        let world = std::fs::canonicalize(world).unwrap_or_else(|_| world.to_path_buf());
        Self::for_base(
            cache_root()
                .join("viber")
                .join("session")
                .join(session_slug(&world)),
        )
    }

    /// Direto sobre a pasta da sessão (testes).
    pub fn for_base(base: PathBuf) -> Self {
        Self { base }
    }

    /// Raiz onde vivem TODAS as sessões (`<cache>/viber/session/`).
    pub fn sessions_root() -> PathBuf {
        cache_root().join("viber").join("session")
    }

    /// Todas as sessões conhecidas, por slug — alimenta `viber session list`
    /// e a descoberta de porta do `viber debug`.
    pub fn all() -> Vec<(String, Self)> {
        let root = Self::sessions_root();
        let Ok(entries) = std::fs::read_dir(&root) else {
            return Vec::new();
        };
        let mut out: Vec<(String, Self)> = entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| {
                let slug = entry.file_name().to_str()?.to_string();
                Some((slug, Self::for_base(entry.path())))
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    fn lease_file(&self) -> PathBuf {
        self.base.join("lease.json")
    }

    fn engine_file(&self) -> PathBuf {
        self.base.join("engine.json")
    }

    /// Log da engine partilhada: `<cache>/viber/logs/<slug>.log`.
    pub fn log_file(&self) -> PathBuf {
        cache_root().join("viber").join("logs").join(format!(
            "{}.log",
            self.base
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("mundo")
        ))
    }

    fn ensure_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base)
            .with_context(|| format!("a criar {}", self.base.display()))
    }
}

/// Raiz de cache: `XDG_CACHE_HOME` ou `~/.cache`.
fn cache_root() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(xdg);
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cache")
}

/// Hash FNV-1a 64 — determinístico ENTRE processos (o `DefaultHasher` de
/// std é semeado à sorte e não serve para nomes de pasta partilhados).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Slug curto e estável do mundo: `<nome>-<8 hex do caminho absoluto>`.
fn session_slug(world: &Path) -> String {
    let name = world
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("mundo")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();
    let hash = format!("{:08x}", fnv1a(world.to_string_lossy().as_bytes()) as u32);
    format!("{name}-{hash}")
}

// ---------------------------------------------------------------- lease

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Lease {
    /// Quem reclama (nome da tarefa/agente, ex.: "qa-sky").
    pub owner: String,
    /// PID do processo que reclamou — `release`/`touch` exigem dono.
    pub pid: u32,
    /// Unix ms em que o lease expira.
    pub expires_at_ms: u64,
}

#[derive(Debug)]
pub enum ClaimOutcome {
    Acquired {
        ttl: Duration,
    },
    /// Ocupado por outro agente — aguardar ou fazer outro trabalho.
    Busy {
        owner: String,
        remaining: Duration,
    },
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Nome de ficheiro tmp ÚNICO por processo+chamada — um `.tmp` partilhado
/// entre dois processos a escrever o MESMO destino interpolava as duas
/// escritas (o 2.º `File::create` truncava o 1.º a meio) e o segundo
/// `rename` falhava com ENOENT.
fn unique_tmp(path: &Path) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.{}.tmp", std::process::id(), seq));
    path.with_file_name(name)
}

fn write_tmp(tmp: &Path, contents: &str) -> Result<()> {
    let mut file =
        std::fs::File::create(tmp).with_context(|| format!("a criar {}", tmp.display()))?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    Ok(())
}

/// Escrita atómica (SOBRESCREVE o destino): tmp único + `sync_all` + rename
/// no mesmo diretório. Um crash a meio não deixa JSON parcial no disco —
/// que seria lido como "corrupto = livre/sem sessão" pelos outros processos.
fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let tmp = unique_tmp(path);
    write_tmp(&tmp, contents)?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("a promover {} → {}", tmp.display(), path.display()))
}

/// Publica um ficheiro NOVO com árbitro atómico: conteúdo completo num tmp
/// único + hard link para o destino (`link` falha com `AlreadyExists` se o
/// destino já existe). O par `create_new` + rename anterior deixava o
/// destino VAZIO entre um e outro — um `claim` concorrente lia JSON vazio,
/// tratava-o como corrupto, apagava o lease acabado de criar e ganhava
/// TAMBÉM: dois donos para a mesma sessão.
fn publish_new(path: &Path, contents: &str) -> Result<bool> {
    let tmp = unique_tmp(path);
    let outcome = write_tmp(&tmp, contents).and_then(|_| match std::fs::hard_link(&tmp, path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("a ligar {} → {}", tmp.display(), path.display()))
        }
    });
    let _ = std::fs::remove_file(&tmp);
    outcome
}

/// O nome do dono tem de bater para `release`/`touch` — protege contra
/// `release` acidental de um agente paralelo (cada CLI é um processo novo,
/// portanto o pid nunca é critério).
fn check_owner(lease: &Lease, owner: Option<&str>) -> Result<()> {
    match owner {
        Some(owner) if owner == lease.owner => Ok(()),
        Some(owner) => bail!("lease pertence a '{}', não a '{}'", lease.owner, owner),
        None => bail!(
            "lease pertence a '{}' (pid {}) — passe --owner para manipulá-lo",
            lease.owner,
            lease.pid
        ),
    }
}

impl SessionPaths {
    fn read_lease(&self) -> Option<Lease> {
        let raw = std::fs::read_to_string(self.lease_file()).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Reclama a sessão. Com `wait`, poll até o lease atual expirar/libertar.
    /// Um lease expirado (agente morreu) é roubado automaticamente.
    pub fn claim(
        &self,
        owner: &str,
        ttl: Duration,
        wait: Option<Duration>,
    ) -> Result<ClaimOutcome> {
        self.ensure_dir()?;
        let deadline = wait.map(|w| Instant::now() + w);
        loop {
            match self.try_claim(owner, ttl)? {
                outcome @ ClaimOutcome::Acquired { .. } => return Ok(outcome),
                ClaimOutcome::Busy {
                    owner: busy_owner,
                    remaining,
                } => {
                    let Some(deadline) = deadline else {
                        return Ok(ClaimOutcome::Busy {
                            owner: busy_owner,
                            remaining,
                        });
                    };
                    if Instant::now() >= deadline {
                        return Ok(ClaimOutcome::Busy {
                            owner: busy_owner,
                            remaining,
                        });
                    }
                    let poll = WAIT_POLL.min(remaining + Duration::from_millis(50));
                    std::thread::sleep(poll);
                }
            }
        }
    }

    fn try_claim(&self, owner: &str, ttl: Duration) -> Result<ClaimOutcome> {
        // Lease expirado → rouba. Ficheiro não-parseável NÃO se remove: o
        // `publish_new` publica o lease com conteúdo COMPLETO desde o
        // primeiro instante (nada de janela vazia), portanto não-parseável
        // aqui é ficheiro verdadeiramente corrupto ou alheio — apagar era
        // destruir estado de outro processo (TOCTOU). Devolve Busy; quem
        // chama tem a lógica de exit 3 e o `claim --wait` re-tenta.
        if let Some(lease) = self.read_lease() {
            let remaining = lease.expires_at_ms.saturating_sub(now_ms());
            if remaining > 0 {
                return Ok(ClaimOutcome::Busy {
                    owner: lease.owner,
                    remaining: Duration::from_millis(remaining),
                });
            }
            // Roubo: RE-LER imediatamente antes de apagar. Dois processos podem
            // ter lido o MESMO lease expirado — só removemos se o ficheiro em
            // disco ainda é exatamente aquele snapshot (owner+pid+expires);
            // caso contrário, não apagamos o lease vivo de outro processo.
            match self.read_lease() {
                Some(atual)
                    if atual.owner == lease.owner
                        && atual.pid == lease.pid
                        && atual.expires_at_ms == lease.expires_at_ms =>
                {
                    let _ = std::fs::remove_file(self.lease_file());
                }
                Some(atual) => {
                    // O snapshot envelheceu — o lease mudou entretanto.
                    // Recuar e reavaliar no próximo ciclo do `claim`.
                    let remaining = atual.expires_at_ms.saturating_sub(now_ms()).max(1);
                    return Ok(ClaimOutcome::Busy {
                        owner: atual.owner,
                        remaining: Duration::from_millis(remaining),
                    });
                }
                // Corrupto ou desaparecido desde o snapshot → caminho "livre".
                None => {
                    let _ = std::fs::remove_file(self.lease_file());
                }
            }
        } else if self.lease_file().exists() {
            // Presente mas não-parseável: com publicação atómica não é
            // "escrita a meio" de ninguém — é corrupto ou alheio. Na dúvida,
            // NUNCA remover: Busy com retry curto (o chamador tem a lógica
            // de exit 3; o `claim --wait` re-tenta).
            return Ok(ClaimOutcome::Busy {
                owner: "?".into(),
                remaining: LEASE_PENDING,
            });
        }
        let lease = Lease {
            owner: owner.to_string(),
            pid: std::process::id(),
            expires_at_ms: now_ms() + ttl.as_millis() as u64,
        };
        let raw = serde_json::to_string(&lease).context("a escrever lease.json")?;
        if !publish_new(&self.lease_file(), &raw)? {
            // Perdeu a corrida do `link` — re-ler para devolver quem ganhou.
            let Some(lease) = self.read_lease() else {
                return Ok(ClaimOutcome::Busy {
                    owner: "?".into(),
                    remaining: ttl,
                });
            };
            let remaining = lease.expires_at_ms.saturating_sub(now_ms());
            return Ok(ClaimOutcome::Busy {
                owner: lease.owner,
                remaining: Duration::from_millis(remaining),
            });
        }
        Ok(ClaimOutcome::Acquired { ttl })
    }

    /// Libertação: pelo NOME do dono (cada comando CLI é um processo novo —
    /// o pid nunca batería). Sem owner, recusa e ensina.
    pub fn release(&self, owner: Option<&str>) -> Result<bool> {
        let Some(lease) = self.read_lease() else {
            return Ok(false); // nada a libertar
        };
        check_owner(&lease, owner)?;
        // Re-ler imediatamente antes de apagar: o lease pode ter expirado e
        // sido roubado desde a primeira leitura — nunca apagar o de outro.
        match self.read_lease() {
            Some(atual) if atual.owner == lease.owner && atual.pid == lease.pid => {}
            Some(atual) => bail!(
                "lease roubado entretanto por '{}' (pid {}) — nada a libertar",
                atual.owner,
                atual.pid
            ),
            None => return Ok(false), // desapareceu (roubado/expirado) entretanto
        }
        std::fs::remove_file(self.lease_file()).context("a remover lease.json")?;
        Ok(true)
    }

    /// Renova o lease do dono; devolve o novo tempo restante.
    pub fn touch(&self, owner: Option<&str>, ttl: Duration) -> Result<Duration> {
        let Some(mut lease) = self.read_lease() else {
            bail!("sem lease para renovar — reclame primeiro (`session claim`)");
        };
        check_owner(&lease, owner)?;
        // Re-ler imediatamente antes de escrever: um lease roubado após o TTL
        // não pode ser sobrescrito pelo dono antigo.
        match self.read_lease() {
            Some(atual) if atual.owner == lease.owner && atual.pid == lease.pid => {}
            Some(atual) => bail!(
                "lease roubado entretanto por '{}' (pid {}) — renovação abortada",
                atual.owner,
                atual.pid
            ),
            None => bail!("lease desapareceu entretanto — foi roubado ou libertado"),
        }
        lease.expires_at_ms = now_ms() + ttl.as_millis() as u64;
        let raw = serde_json::to_string(&lease)?;
        write_atomic(&self.lease_file(), &raw).context("a renovar lease.json")?;
        Ok(ttl)
    }

    /// Estado do lease, do ponto de vista de FORA (não exige ser dono):
    /// `None` = livre.
    pub fn busy(&self) -> Option<(String, Duration)> {
        let lease = self.read_lease()?;
        let remaining = lease.expires_at_ms.saturating_sub(now_ms());
        if remaining == 0 {
            None
        } else {
            Some((lease.owner, Duration::from_millis(remaining)))
        }
    }
}

// ---------------------------------------------------------------- engine

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EngineInfo {
    pub pid: u32,
    pub port: u16,
    /// Caminho absoluto do world.xml.
    pub world: String,
    pub started_at_ms: u64,
    pub log: String,
}

impl SessionPaths {
    pub fn engine_info(&self) -> Option<EngineInfo> {
        let raw = std::fs::read_to_string(self.engine_file()).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn write_engine(&self, info: &EngineInfo) -> Result<()> {
        self.ensure_dir()?;
        let raw = serde_json::to_string(info)?;
        write_atomic(&self.engine_file(), &raw).context("a escrever engine.json")
    }

    pub fn clear_engine(&self) {
        let _ = std::fs::remove_file(self.engine_file());
    }
}

/// Slug/paths expostos para o CLI (mensagens e log).
impl SessionPaths {
    pub fn display(&self) -> String {
        self.base.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_session(tag: &str) -> (tempfile::TempDir, SessionPaths) {
        let dir = tempfile::tempdir().expect("tmpdir");
        let paths = SessionPaths::for_base(dir.path().join(tag));
        (dir, paths)
    }

    #[test]
    fn test_claim_release_lifecycle() {
        let (_dir, paths) = tmp_session("lifecycle");
        assert!(paths.busy().is_none(), "sessão nasce livre");

        let first = paths
            .claim("agente-a", Duration::from_secs(60), None)
            .expect("claim A");
        assert!(matches!(first, ClaimOutcome::Acquired { .. }));

        let second = paths
            .claim("agente-b", Duration::from_secs(60), None)
            .expect("claim B falha soft");
        let ClaimOutcome::Busy { owner, remaining } = second else {
            panic!("devia estar ocupado");
        };
        assert_eq!(owner, "agente-a");
        assert!(remaining > Duration::from_secs(50));

        // Release de OUTRO dono não pode (cada CLI é um processo novo — o
        // cheque é pelo nome do owner).
        assert!(
            paths.release(Some("agente-b")).is_err(),
            "não-dono não liberta"
        );
        assert!(paths.release(None).is_err(), "sem --owner recusa e ensina");
        assert!(paths.busy().is_some());

        assert!(paths.release(Some("agente-a")).expect("release do dono"));
        assert!(paths.busy().is_none());

        let again = paths
            .claim("agente-b", Duration::from_secs(60), None)
            .expect("claim B depois do release");
        assert!(matches!(again, ClaimOutcome::Acquired { .. }));
    }

    #[test]
    fn test_expired_lease_is_stolen() {
        let (_dir, paths) = tmp_session("expired");
        paths
            .claim("agente-morto", Duration::from_secs(0), None)
            .expect("claim com ttl 0");
        // TTL 0 → já expirou: o próximo claim rouba sem esperar.
        let stolen = paths
            .claim("agente-novo", Duration::from_secs(60), None)
            .expect("claim rouba lease expirado");
        assert!(matches!(stolen, ClaimOutcome::Acquired { .. }));
    }

    #[test]
    fn test_touch_extends_lease() {
        let (_dir, paths) = tmp_session("touch");
        paths
            .claim("agente", Duration::from_secs(30), None)
            .expect("claim");
        assert!(
            paths
                .touch(Some("outro"), Duration::from_secs(300))
                .is_err()
        );
        let renewed = paths
            .touch(Some("agente"), Duration::from_secs(300))
            .expect("touch do dono");
        assert_eq!(renewed, Duration::from_secs(300));
        let (_, remaining) = paths.busy().expect("ainda ocupado");
        assert!(
            remaining > Duration::from_secs(290),
            "restante {remaining:?}"
        );
    }

    #[test]
    fn test_unparseable_lease_is_busy_and_survives() {
        let (_dir, paths) = tmp_session("unparseable");
        paths.ensure_dir().unwrap();
        // Ficheiro VAZIO: o `publish_new` publica o lease com conteúdo
        // completo desde o 1.º instante, portanto vazio não é "boot a meio"
        // de processo nenhum vivo — é corrupto/alheio. MESMO ASSIM não pode
        // ser roubado: busy, com o ficheiro intacto (nunca remover o que
        // não sabemos parsear).
        std::fs::write(paths.lease_file(), "").unwrap();
        let claim = paths
            .claim("agente-b", Duration::from_secs(60), None)
            .expect("claim devolve (sem wait)");
        assert!(
            matches!(claim, ClaimOutcome::Busy { .. }),
            "lease vazio é boot a meio → ocupado, não livre"
        );
        assert!(paths.lease_file().exists(), "o claim não apaga o lease");
        // JSON quebrado (não vazio): com escrita atómica não devia existir,
        // mas o lado seguro é o mesmo — busy, nunca remoção.
        std::fs::write(paths.lease_file(), "{quebrado").unwrap();
        let claim = paths
            .claim("agente-b", Duration::from_secs(60), None)
            .expect("claim devolve (sem wait)");
        assert!(
            matches!(claim, ClaimOutcome::Busy { .. }),
            "lease não-parseável → ocupado"
        );
        assert!(paths.lease_file().exists());
        // E o `claim --wait` sai limpo quando o boot a meio termina: o dono
        // escreve o conteúdo e o wait re-tenta até o lease expirar.
        std::fs::write(
            paths.lease_file(),
            r#"{"owner":"agente-a","pid":1,"expires_at_ms":0}"#,
        )
        .unwrap();
        let stolen = paths
            .claim("agente-b", Duration::from_secs(60), None)
            .expect("claim rouba lease parseável expirado");
        assert!(matches!(stolen, ClaimOutcome::Acquired { .. }));
    }

    #[test]
    fn test_publish_new_never_overwrites_and_leaves_no_tmp() {
        let (_dir, paths) = tmp_session("publish");
        paths.ensure_dir().unwrap();
        let lease = paths.lease_file();
        std::fs::write(&lease, r#"{"owner":"a"}"#).unwrap();
        // Destino existente → perde a corrida, e NÃO sobrescreve o dono.
        assert!(!publish_new(&lease, r#"{"owner":"b"}"#).expect("publish"));
        assert_eq!(
            std::fs::read_to_string(&lease).unwrap(),
            r#"{"owner":"a"}"#,
            "dono original intacto"
        );
        // Destino ausente → ganha, com o conteúdo COMPLETO visível de imediato.
        std::fs::remove_file(&lease).unwrap();
        assert!(publish_new(&lease, r#"{"owner":"c","pid":1}"#).expect("publish"));
        assert_eq!(
            std::fs::read_to_string(&lease).unwrap(),
            r#"{"owner":"c","pid":1}"#
        );
        let leftovers: Vec<String> = std::fs::read_dir(paths.base.clone())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "tmps órfãos: {leftovers:?}");
    }

    #[test]
    fn test_slug_is_stable_and_distinct() {
        let a = session_slug(Path::new("/tmp/mundos/exemplo.xml"));
        let b = session_slug(Path::new("/tmp/mundos/exemplo.xml"));
        let c = session_slug(Path::new("/tmp/outro/exemplo.xml"));
        assert_eq!(a, b, "mesmo caminho → mesmo slug");
        assert_ne!(a, c, "caminhos diferentes → slugs diferentes");
        assert!(a.starts_with("exemplo-"), "slug mantém o nome: {a}");
    }

    #[test]
    fn test_engine_info_roundtrip() {
        let (_dir, paths) = tmp_session("engine");
        assert!(paths.engine_info().is_none());
        paths
            .write_engine(&EngineInfo {
                pid: 1234,
                port: 15702,
                world: "/m/world.xml".into(),
                started_at_ms: 42,
                log: "/tmp/x.log".into(),
            })
            .expect("write engine");
        let info = paths.engine_info().expect("engine info");
        assert_eq!(info.pid, 1234);
        assert_eq!(info.port, 15702);
        paths.clear_engine();
        assert!(paths.engine_info().is_none());
    }
}
