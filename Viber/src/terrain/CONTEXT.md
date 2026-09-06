# CONTEXT.md — src/terrain

Sistema de terreno (Fase 1 ✅): heightfield em chunks com LOD, carved por
features declarativas do XML (`Terrain`, `TerrainPad`, `Lake`, `River`,
`Cliff`, `Road`, `RoadNetwork`) e consultável pelo gameplay em runtime.

## Como se articula

1. **Bootstrap one-shot** (`runtime.rs`): carrega/gera a heightmap → carve
   pads → água → cliffs → estradas (pelo brush engine, com journal) →
   sharpen opt-in + `CliffMask` → spawna chunks, água e ribbons.
2. **Runtime** (`plugin.rs`): LOD dinâmico por distância da câmara (histerese
   + gate anti-thrash), rebuild com budget por frame, cull por
   `render-distance`.
3. **Gameplay** lê sem tocar na mesh: `TerrainRuntime::sample / in_water /
   on_road` (recurso), `WaterBody`, `RoadPath` — é isto que o player, os
   spawners e os scripts usam.

## Estado

Estável; recentemente calibrado (skirt_depth como cap do drop adaptativo com
mínimo útil 6 m, mipmaps + anisotropia nas texturas). Mundo demo:
`worlds/terrain.xml`; uso real: `examples/simple-rpg/world.xml` e módulos
`world/` (pads de cidade, estradas com `RoadNetwork`, lagos/rios no frontier).

Armadilha conhecida fora da pasta: o `AGENTS.md` da raiz documenta que
larguras de estrada < 1.5 texéis viram no-op (promovidas por `min_effective`)
— mundos com estradas finixinhas podem parecer "não carved".
