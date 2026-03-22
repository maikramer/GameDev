# Design: CLI preparado para agentes de IA

**Data:** 2026-03-15  
**Objetivo:** Permitir que agentes de IA (ex.: Cursor) invoquem o binário `materialize` com segurança, sabendo quando usar, como invocar e como interpretar o resultado.

## Escopo

- **Uso:** Agentes rodam `materialize` via terminal (não API/MCP, não output JSON).
- **Entregáveis:** AGENTS.md na raiz + regra Cursor em `.cursor/rules/`.

## Arquitetura da solução

| Artefato | Propósito |
|----------|-----------|
| **AGENTS.md** | Memória do projeto para qualquer agente: o que é o CLI, quando usar, sintaxe, exemplos, códigos de saída. Único ponto de verdade legível por humanos e agentes. |
| **.cursor/rules/materialize-cli.mdc** | Regra Cursor (alwaysApply) para o agente usar o CLI quando a tarefa envolver gerar mapas PBR a partir de texturas; padrão de invocação e checagem de erros. |

## Conteúdo de AGENTS.md

- Descrição em 1–2 frases do Materialize CLI.
- **Quando usar:** gerar mapas PBR (height, normal, metallic) a partir de uma imagem difusa (textura).
- **Quando não usar:** redimensionar, converter formato sem PBR, edição de imagem genérica.
- Sintaxe: `materialize <INPUT> [-o DIR] [-f png|jpg|tga|exr] [-q 0-100] [-v]`.
- Exemplos mínimos (uso básico, saída em diretório, verbose).
- Códigos de saída: 0 = sucesso; não-zero = erro (mensagem em stderr).
- Referência à documentação completa: `docs/cli-api.md`, `docs/README.md`.

## Conteúdo da regra Cursor

- **Frontmatter:** `description`, `alwaysApply: true`.
- **Corpo (conciso, <50 linhas):**
  - Ao precisar gerar mapas PBR a partir de textura, usar o CLI `materialize`.
  - Verificar se o arquivo de entrada existe antes de invocar.
  - Padrão de comando: `materialize <caminho> [-o dir] [-v]`; opcionais `-f`, `-q` conforme necessidade.
  - Após invocação: checar exit code; em falha, ler stderr para diagnóstico.
  - Não usar para outras tarefas (redimensionar, converter formato sem PBR).

## Manutenção

- Atualizar AGENTS.md e a regra quando novos argumentos ou comportamentos forem adicionados ao CLI.
- Manter a regra alinhada às flags realmente implementadas em `src/cli.rs` (evitar documentar flags inexistentes).

## Decisões

- Regra com `alwaysApply: true` para que o contexto do CLI esteja sempre disponível neste repositório.
- Documentação baseada no estado atual do `cli.rs` (sem prefix/suffix até serem implementados).
