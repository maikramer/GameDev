# CONTEXT.md — src/ui

A UI declarativa substituiu milhares de linhas de builders Rust que viviam em
`src/hud/`: um mundo agora muda o seu HUD e os seus menus editando **XML +
CSS + um script Luau**, sem recompilar a engine.

## As três camadas (uma responsabilidade cada)

| Camada | Onde vive | Responsabilidade |
|--------|-----------|------------------|
| Estrutura | `<UiRoot>` no world XML | que elementos existem e como se aninham |
| Apresentação | `<UiStyle>` / ficheiro `.css` | tamanhos, cores, estados (`:hover`), layout |
| Dados e comportamento | `bind="…"` + `viber.ui.*` (Luau) | que valores aparecem e o que os botões fazem |

## Estado

Fase 2 ✅. No `simple-rpg` já vivem aqui: o HUD com tabs (`world/hud.xml` +
`ui/hud.css` + `scripts/ui/hud.lua`) e o menu de jogo [Q] com tabs
Missões/Mochila/Talentos/Loja/Controlos/Sistema (`world/menu.xml` +
`ui/menu.css` + `scripts/ui/menu.lua`), incluindo a loja. `src/hud/` ficou
só com os widgets "vivos" que desenham dados do mundo frame a frame.

Exemplos vivos para copiar padrões (bindings, listas, modais):
`examples/simple-rpg/world/*.xml` e `examples/simple-rpg/ui/*.css`.
