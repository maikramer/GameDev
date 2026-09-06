# AGENTS.md — src/recipes

Escopo: XML expandido (`src/xml`) → **IR de entidades** com nomenclatura Bevy
→ spawn Bevy. É a camada que decide o que cada tag do mundo significa.

## Ficheiros

| Ficheiro | Responsabilidade |
|----------|------------------|
| `mod.rs` | IR + `KNOWN_TAGS` (ortografia canónica **lowercase**; tag fora da lista = skip no-op, reportado pelo `analyze`). Também parse dos specs de física (`parse_collider`/`parse_body` de `crate::physics`) |
| `spawn.rs` | spawn Bevy a partir da IR: primitivas, luzes, câmaras, glTF, spawners, terreno, UI, HUD… |
| `transform.rs` | `euler` (graus XYZ) → quat; `rotation` (quat `x y z w`) ganha sobre `euler` |

## Regras

- Nomenclatura **Bevy** (`translation`, `euler`, `half-size`, `base-color`,
  `metallic`) — nunca Unity/three.js (`position`, `rotation.z`…).
- Atributo desconhecido = **warning** (nunca erro); tag desconhecida = skip
  no-op. `analyze --strict` promove ambos a erro.
- **Adicionar uma tag nova:** entar em `KNOWN_TAGS` (lowercase) + recipe em
  `mod.rs` + handler de spawn em `spawn.rs` + linha no `AGENTS.md` da raiz.
  Se tiver runtime novo, o relatório de cobertura do `analyze` deve deixar de
  a listar.
- Vetores/bools vêm sempre de `src/xml/values.rs` (broadcast de 1 valor;
  2 valores = erro; não-finito rejeitado).
- Spawners (`StaticSpawner`/`DynamicSpawner`) têm de espelhar a função pura
  `compute_placements` de `src/spawner.rs` — colocação determinística por
  `seed` (SplitMix64), rejeitando água/estrada/declive/sobreposição.

## Verificar

```bash
cd Viber && cargo test
cargo run -- analyze examples/simple-rpg/world.xml   # relatório de cobertura
```
