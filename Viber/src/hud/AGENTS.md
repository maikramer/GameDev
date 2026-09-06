# AGENTS.md — src/hud

Escopo: widgets bevy_ui que **desenham dados vivos do mundo** frame a frame.
Painéis, barras e menus estáticos do jogo vivem agora na UI declarativa
(`src/ui/`) — não os trazer de volta para aqui.

## Ficheiros

| Ficheiro | Responsabilidade |
|----------|------------------|
| `assets.rs` | `HudAssets`: font display (Cinzel), texturas geradas, paleta de painéis, primitivas de texto |
| `elements.rs` | builders das tags HUD do mundo (`HealthBar`, `XpBar`, `Minimap`, `Compass`, `InteractionPrompt`, …) + `spawn_hud` / `spawn_resource_chip` |
| `vitals.rs` | fill de HP/XP espelhando os vitals |
| `interact.rs` | prompt de interação [E], balão de diálogo, painéis por tecla (`HudPrompt`/`HudBalloon`/`HudToggle`) |
| `compass.rs` / `minimap.rs` | compasso com ticks e distâncias por setor; minimapa com seta e dots de quest numerados |
| `profiler_window.rs` | janela do profiler (F3/P) — FPS, frame-time, entidades, scripts ativos, chunks |
| `menu.rs` | cola legada do menu (o menu real é o modal declarativo em `world/menu.xml`) |
| `widgets.rs` | helpers de construção |

## Regras

- Aqui fica só o que é **animado/vivo** (minimapa, compasso, prompt,
  balão, profiler). Estática e layouts → `src/ui/` (XML+CSS do mundo).
- Nametags (pílulas flutuantes nome+distância sobre NPCs) foram **removidas a
  pedido do autor** (2026-09-06) — poluíam a vista e ficavam fora de layout.
  Não recriar sem decisão; o marcador de quest continua no minimapa e na
  esfera 3D do `DialogueNPC`.
- A tag `BossBar` é aceite no XML mas **não tem UI** (removida no cleanup) —
  não recriar sem decisão.
- Estilos pontuais são aceitáveis; paleta/fonte vem de `assets.rs` — mudanças
  de identidade visual são aí.

## Verificar

```bash
cd Viber && cargo test
cargo run -- run examples/simple-rpg/world.xml   # ver HUD; F3/P profiler
```
