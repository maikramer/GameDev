# Do zero ao jogo com IA — playbook (resumo PT)

Documento de apoio ao fluxo **conteúdo gerado + orquestração + agentes de código**. Versão completa em inglês: [ZERO_TO_GAME_AI.md](ZERO_TO_GAME_AI.md).

## Três camadas de IA no monorepo

1. **Modelos generativos** — Text2D, Texture2D, Text3D, Paint3D, Text2Sound, Skymap2D, etc.: transformam prompts em ficheiros (PNG, GLB, áudio).
2. **Orquestração** — **GameAssets** (`gameassets batch`): perfil YAML + CSV + presets, chamadas determinísticas a CLIs (`*_BIN`).
3. **Agentes / IDE** — Convenções em [AGENTS.md](../AGENTS.md), contexto do motor em [VibeGame/llms.txt](../VibeGame/llms.txt), skill do GameAssets em `GameAssets/src/gameassets/cursor_skill/SKILL.md`.

## Fluxo recomendado

1. Instalar ferramentas ([INSTALLING_PT.md](INSTALLING_PT.md)).
2. Definir `game.yaml` + `manifest.csv` + presets; opcionalmente `gameassets prompts` antes do batch.
3. Correr `gameassets batch` com as flags necessárias (`--with-3d`, `--with-rig`, áudio, etc.).
4. Opcional: validar GLBs com GameDevLab.
5. Copiar outputs para `public/assets/…` do projeto web e usar `loadGltfToScene` ou o exemplo [monorepo-game](../VibeGame/examples/monorepo-game/).
6. Iterar **código e XML** com o assistente, usando `llms.txt` para VibeGame e AGENTS.md para o resto do repositório.

## Handoff técnico

Ver [MONOREPO_GAME_PIPELINE.md](MONOREPO_GAME_PIPELINE.md) (pastas, URLs, `loadGltfToScene`).

## Backlog de I&D (priorizado)

| Prioridade | Tema |
|------------|------|
| Alta | Automação ou script documentado **Animator3D** pós-rig |
| Alta | Ponte **Skymap2D → Three.js** (env/PMREM) |
| Média | Script **export pack** (output → `public/` + manifest JSON) |
| Média | Plano **JSON** para agentes (`gameassets --dry-run` evoluído) |
| Baixa | **Recipe XML** para URL GLB no VibeGame |
