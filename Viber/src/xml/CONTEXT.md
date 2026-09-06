# CONTEXT.md — src/xml

Primeira fase do pipeline da engine (`src/main.rs`): ficheiro XML → árvore
`XmlNode` com includes expandidos → consumida por `src/recipes/`, que produz a
IR de entidades e faz o spawn Bevy.

## O que é

- **Strict XML com duas tolerâncias herdadas do engine original:** atributos
  booleanos podem ser escritos bare (`<PointLight shadows>`) e os nomes de
  tags são matched case-insensitive a jusante.
- `XmlNode` mantém os atributos como pares de strings crus — a interpretação
  (vetores, bools, cores, hex) é feita por quem consome, via `values.rs`.
- `include.rs` permite decompor mundos em módulos (`examples/simple-rpg/world/`
  é o exemplo grande: cidades, criaturas, landmarks em ficheiros separados).

## Estado

Completo (Fase 0 ✅). Estável — mexer aqui afeta o parser de TODOS os mundos;
qualquer nova tolerância tem de manter os mundos existamos válidos e aparecer
nos testes. O `analyze` é o arbiter: exit 1 em erro, warnings para
atributos desconhecidos.
