# AGENTS.md — src/bridge

Escopo: debug bridge — BRP sobre HTTP (`bevy_remote`) com métodos `viber.*`:
screenshots, input sintético, árvore de entidades, logs, profiler e REPL
Luau. É o equivalente nativo do tooling Chrome DevTools MCP do VibeGame.

## Ficheiros

| Ficheiro | Responsabilidade |
|----------|------------------|
| `mod.rs` | server: porta **15702** (`--bridge PORT` muda), constantes `METHOD_*`, handlers `viber.ping/screenshot/screenshot_status/tree/logs/profiler/lua/input.*` |
| `client.rs` | cliente CLI **std-only** (`viber debug …`); retry no connect (o bind é assíncrono) |
| `logs.rs` | layer de tracing → ring-buffer de 1000 entradas |
| `lua.rs` | método `viber.lua`: compila o chunk na env persistente da VM do `LuaScriptHost`, **player como self**; devolve `{ok, result\|error, applied, warnings}` |
| `tests.rs` | App mínima + bridge real em loopback (`cargo test`) |

## Regras

- Handlers correm como **sistemas exclusivos em `RemoteLast`** (depois de
  `Last`) — nunca bloquear um handler à espera de render.
- Screenshot é **request + poll** (`viber.screenshot` →
  `viber.screenshot_status`): a captura precisa de frames de render.
- REPL Luau: leituras vêm de um **snapshot do início da chamada**; escritas
  aplicam **no mesmo frame** (antes dos sistemas de gameplay). Sem guard de
  instruções — `while true do end` congela o frame (risco aceite, igual a um
  script de página no Chrome).
- Porta do cliente: `--port` ou `VIBER_BRIDGE_PORT`.
- Os métodos BRP builtin (`world.query`, `world.spawn_entity`,
  `world.mutate_components`, …) ficam também expostos — inspecção/mutação
  live do ECS.

## Verificar

```bash
cd Viber && cargo test          # tests.rs sobe uma App com bridge em loopback
viber run worlds/hello.xml --bridge &   # engine com bridge
viber debug probe && viber debug tree --json
```
