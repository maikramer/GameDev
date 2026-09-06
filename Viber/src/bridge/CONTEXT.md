# CONTEXT.md — src/bridge

Infraestrutura de QA/automação (Fase 2 ✅): liga uma engine Viber a agentes e
scripts de teste sem browser. Com `viber run world.xml --bridge`, o cliente
`viber debug` dá probe/screenshot/tree/logs/profiler/click/key/move/text e a
REPL `viber debug lua '<código>'` (toda a API `viber.*` + `viber.debug.*`:
teleport, spawn de markers, disable/despawn, heal/damage, …).

## Fluxo de trabalho provado

1. `viber run <mundo> --bridge` (uma engine por vez — engines simultâneas
   esgotam VRAM).
2. `viber debug probe` confirma o bridge vivo.
3. `viber debug screenshot -o shot.png` / `viber debug tree` / `viber debug
   prof` para QA headless; `viber debug lua` para mutações pontuais
   (`viber.debug.tp(0, -20)`, `disable('goblin')`, …).

Screenshots são request+poll (precisam de frames de render); `viber.tree`
devolve transforms **locais** (para world-space usar `viber.debug.entities()`).
Código de teste headless: `tests.rs` sobe o bridge real em loopback.
