## Plan Generated: water-underwater-shader-interaction

**Key Decisions**:
- Use existing WaterPlugin architecture (water-material.ts, planar-reflection.ts, systems.ts, components.ts, plugin.ts) to implement underwater shader and interaction.
- Add an underwater mode toggle driven by camera position relative to water surface; render underwater color grading and optional fog when submerged.
- Ensure collision system allows walking on water and prevents sinking; introduce a water plane collision as part of the WaterComponent data.
- Implement foam/noise stability improvements to reduce shoreline flicker; consider a small normal-map-based foam with fixed random seeds for stability.
- Expose configuration defaults for water level, tint, wave params, and reflection toggles; provide a QA-friendly test matrix.

**Scope**: IN: 5 core water files; underwater shader mode; collision integration; optional underwater post-processing flag; QA scenarios. OUT: Non-water engine subsystems.

## TL;DR
> Summary: Plane the plan for an underwater shader and collision interactions with a single, cohesive water plugin, covering shader work, ECS integration, and basic QA.
> Deliverables: Updated water-material.ts with underwater logic, planar-reflection.ts adjustments, collision plane support, underwater post-process toggle, and a small set of QA tests.
> Effort: Large
> Parallel: YES - multiple tasks can run concurrently (shader, ECS, tests)
> Critical Path: Water shader → Planar reflection → Collision integration → QA tests

## Context
- Original request: Improve water rendering for VibeGame with underwater shader and player-water contact; fix observed noise/flicker; keep player able to walk on water.
- Findings from Metis/Oracle: Core water plugin exists as a five-file module and can be extended to support underwater visuals and collision handling.
- Decision: Implement underwater shader state via a camera-relative flag and keep existing water-plane collision behavior intact.

## Work Objectives
- Core Objective: Deliver a working underwater shader with stable water reflections and player-water interaction, within the existing WaterPlugin.
- Deliverables: water-material.ts under underwater mode, planar-reflection.ts stability improvements, components.ts to include waterLevel and collision flags, plugin.ts defaults updated, optional underwater post-processing toggle.
- Definition of Done: All tasks complete, tests cover happy path; no flicker; player can walk on water; underwater visuals render as expected; docs updated.
- Must Have: Underwater shader state toggle, water surface remains walkable, stable reflections, performance-conscious defaults.
- Must NOT Have: Any changes to non-water engine systems; introduce heavy post-processing by default.

## Verification Strategy
- Test decision: TDD + tests-after
- QA policy: Automated scenarios for water walkability, underwater visuals, and flicker reduction.
- Evidence: .sisyphus/evidence/task-water-underwater-shader-interaction.md (to be produced by QA run)

## Execution Strategy
### Parallel Execution Waves
- Wave 1: Shader underwater mode toggle logic; collision interaction; basic tests.
- Wave 2: Planar reflection tweaks; foam/noise stability adjustments; optional underwater post-processing flag.
- Wave 3: Integration tests and QA scenarios.

## TODOs
- [ ] 1. Implement underwater-mode in water-material.ts
  - What to do: Add a per-water uniform (underwaterActive) and depth-based shading; adjust refraction and foam when camera is submerged; ensure transition is smooth.
  - Must Not: Introduce heavy branching that hurts performance; break existing water visuals when underwater is disabled.
  - Recommended Agent Profile: Category: unspecified-high; Skills: water-shader, glsl
  - Parallelization: Wave 1
  - References: VibeGame/src/plugins/water/water-material.ts, foam/noise texture usage
  - Acceptance Criteria: When underwaterActive == 1, shader uses underwater tint, reduced reflection, proper refraction, and foam adapts with depth; otherwise, default visuals remain.

- [ ] 2. Update planar-reflection.ts to be underwater-aware
  - What to do: Gate planar reflections when underwater; provide a cheaper fallback or fade-out as depth increases.
  - Must Not: Disable reflections globally when just shallowly submerged.
  - Parallelization: Wave 1
  - Acceptance Criteria: Reflections fade or switch to fallback once underwater transitions are detected; performance stays stable.

- [ ] 3. Extend components.ts and systems.ts for waterLevel and underwater state
  - What to do: Add waterLevel field to WaterComponent; create UnderwaterState in Systems to compute underwaterFade from camera Y vs waterLevel; push uniforms to materials.
  - Acceptance Criteria: Water entities expose waterLevel and underwaterFade; shader uniforms update per frame without jitter.

- [ ] 4. Extend plugin.ts defaults to include underwater flags
  - What to do: Add underwaterEnabled (default false), underwaterFogColor, underwaterFogDensity, underwaterPostProcess (default false).
  - Acceptance Criteria: Projects continue to work with defaults; underwater options accessible in configuration.

- [ ] 5. Add lightweight underwater-postprocessing toggle
  - What to do: Implement a minimal post-process pass toggled by underwaterPostProcess; affects fogColor and subtle color grading under water.
  - Acceptance Criteria: When enabled, underwater scenes look distinct; non-underwater scenes unchanged.

- [ ] 6. Create QA scenarios and validation scaffolding
  - What to do: Add test scaffolding (manually verifiable steps) for underwater transition, walk-on-water behavior, and flicker/stability checks.
  - Acceptance Criteria: 3-5 repeatable QA tests with expected outcomes and minimal automation hooks.
- [ ] 2. Ensure planar-reflection.ts correctly samples reflections when underwater mode toggles; guard against artifacts.
- [ ] 3. Extend components.ts with waterLevel, collision flags, and underwater state; adjust systems.ts to apply uniforms accordingly.
- [ ] 4. Update plugin.ts with defaults for underwater behavior and the post-processing toggle.
- [ ] 5. Create QA scenarios for underwater transition, walkability, and flicker stability; automate where possible.
- [ ] 6. Add lightweight tests for water shader configuration and collision handling.

## Final Verification Wave
- F1 Plan Compliance Audit — oracle
- F2 Code Quality Review — unspecified-high
- F3 Real Manual QA — unspecified-high
- F4 Scope Fidelity Check — deep

## Commit Strategy
- Commit messages will follow Conventional Commits, e.g., feat(water): implement underwater shader mode and collision integration

## Success Criteria
- Underwater shader renders with depth tint and refraction; reflection sampling remains stable; flicker reduced on shorelines; player walkable on water; QA scenarios pass.
