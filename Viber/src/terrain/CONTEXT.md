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

## Saias (skirts) — as duas regras que já se pagaram caro

Ambas as saias do sistema (a de **chunk**, `mesh.rs`, e a lateral da **ribbon
de estrada**, `roads.rs`) existem para tapar uma fenda, e ambas já produziram
o artefacto oposto — desenharam uma linha onde não havia fenda nenhuma:

1. **A saia de chunk sonda só SOBRE a linha de borda** (`skirt_span_probe`).
   Amostrar para fora da borda lê o terreno do vizinho, que o vizinho desenha
   ele próprio: não sela racha nenhuma e, numa crista convexa, descia a saia
   metros abaixo → cortina cinzenta a atravessar o mundo na costura dos
   chunks. UV/cor/**normal** da saia copiam o vértice de borda (normal
   horizontal = fresta com outra luz = risco na costura) e o alfa do
   cliff-factor vai a 0 (senão o declive vertical apanhava o triplanar de
   rocha no meio da relva).
2. **A saia da ribbon não se desenha (alpha 0).** Não há corte
   para tapar — a borda do deck já desvanece a alpha 0 — e a cortina, opaca e
   com o topo `RIBBON_LIFT` acima do chão, virava um risco contínuo
   ao longo da berma.

Regressões: `test_skirt_ignores_the_drop_beyond_a_convex_border`,
`test_ribbon_skirt_never_draws`.
