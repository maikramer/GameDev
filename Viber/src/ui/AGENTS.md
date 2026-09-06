# AGENTS.md — src/ui

Escopo: **UI declarativa** — estrutura XML + folha de estilo CSS-like +
comportamento Luau (`viber.ui.*`). Guia canónico: [`docs/UI.md`](../../docs/UI.md)
(manter em sync!). Referência da API: `docs/LUA_API.md`.

## Ficheiros

| Ficheiro | Responsabilidade |
|----------|------------------|
| `tree.rs` | XML → bevy_ui (`build_ui_tree`, `is_ui_tag`); tags `uitext, uibutton, uibar, uilist, uimodal, uirow, uicolumn, uicooldown, uiicon, uispacer` + `UiPanel`/`UiRoot` |
| `style.rs` | stylesheet do `<UiStyle>` (texto do elemento ou `src="ui/hud.css"`): classes, ids, `:hover`, cascade real |
| `runtime.rs` | componentes bevy_ui: `UiBar`, `UiClicks`, `UiRegistry`, `UiStyleDirty`, `UiCooldown`, … |
| `bind.rs` / `collect.rs` | bindings nomeadas (`bind="…"`) alimentadas do gameplay (`UiData`); `UiPrompt`/`UiToast` |
| `script.rs` | API Luau `viber.ui.*` (comandos enfileirados, `UiScriptState`) |
| `modal.rs` | `UiModal`/`UiTabs`/`UiScroll`/`UiTabPage` — o menu de jogo [Q] e modais autorais (`key=`, `escape-closes`) |
| `list.rs` | `UiList` + `<UiTemplate>`/`<UiEmpty>` — repetidor com `{campo}` substituído |
| `actions.rs` | ações `viber.ui.action("save"|"load"|…)` |
| `menu_data.rs` | dados do menu de jogo que a UI declarativa consome |

## Regras

- **`docs/UI.md` tem de ser atualizado no mesmo cambio** que alterar tags,
  attrs ou API — é o guia que autores de mundo (e agentes) seguem.
- Conteúdo de HUD/menus vive nos ficheiros do mundo (ex.:
  `examples/simple-rpg/world/hud.xml`, `world/menu.xml`, `ui/*.css`) — **não**
  reintroduzir construtores Rust de painéis.
- `UiModalsOpen` é **espelhado** em `MenusOpen` (`src/menus.rs`) — não criar
  segundo estado de "menu aberto".
- Estilos por omissão vivem aqui; cores específicas de mundo vivem no CSS do
  mundo.

## Verificar

```bash
cd Viber && cargo test
cargo run -- analyze examples/simple-rpg/world.xml
cargo run -- run examples/simple-rpg/world.xml   # [Q] abre o menu declarativo
```
