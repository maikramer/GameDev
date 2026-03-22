# Design: Melhorias e otimizações Materialize-CLI (B + C)

**Data:** 2026-03-15  
**Escopo:** Qualidade de código (B) e experiência de uso (C). Abordagem 1 — correções e consistência.

## 1. Qualidade de código

- **Versão única:** `cli.rs` usar `version = env!("CARGO_PKG_VERSION")`; teste de integração checar versão dinamicamente (ou regex).
- **JPEG quality:** Em `io.rs`, ao salvar JPEG, usar API da crate `image` que aceita qualidade; passar `quality`; outros formatos ignoram.
- **Cargo.toml:** Ajustar ou remover `authors = ["Your Name"]`.
- **Validação --quality:** Em `cli.rs`, `value_parser` para `quality` em 0..=100.
- **Struct de paths:** `io.rs` (ou cli) — struct `OutputPaths { height_path, normal_path, metallic_path }`; `get_output_paths` retorna esse struct; `main.rs` usa campos.

## 2. Experiência de uso (CLI)

- **Exemplos no --help:** `after_help` em `cli.rs` com 1–2 exemplos (e.g. `materialize texture.png -o ./out/`).
- **Ajuda --quality:** Texto do help: "JPEG quality 0-100 (ignored for other formats)".
- **--quiet:** Flag `-q`/`--quiet` que suprime as linhas "Generated: ..." em stdout em caso de sucesso.

## 3. Erros

- **main:** Trocar `eprintln!("Error: {}", e)` por `eprintln!("Error: {:#}", e)` para exibir cadeia anyhow.

## Não escopo

- Refatoração de arquitetura; testes unitários adicionais além dos existentes; performance GPU/build.
