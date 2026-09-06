#!/usr/bin/env bash
# instance-lock.sh — lock de instância Viber para testes multi-agente.
#
# PROBLEMA: vários agentes correm `viber run` de teste em paralelo e se
# matam uns aos outros com pgrep/kill. Este lock garante UMA instância de
# teste de cada vez por máquina: quem não consegue o lock ESPERA ou desiste
# — nunca mata o processo do outro.
#
# ⚠️ REGRAS (ver docs/findings/VIBER_MULTI_AGENT.md):
#   1. Antes de rodar uma instância de teste, ADQUIRA o lock.
#   2. O teste LIBERTA o lock no fim: no modo sourced o acquire instala um
#      `trap ... EXIT` automaticamente; `exec --` liberta ao terminar o
#      comando.
#   3. NUNCA `pkill`/`kill` processos viber alheios. Se `is-locked`, a
#      instância é de outro agente: aguarde ou re-agende o teste.
#   4. Lock órfão é detectado como stale e pode ser assumido — tanto o PID
#      morto como o **lock expirado** (mais velho que VIBER_LOCK_TTL, default
#      2 h): uma janela de teste esquecida aberta deixa de bloquear a máquina
#      para sempre. O `acquire` que assume um lock expirado MATA o processo
#      dono (é a única excepção à regra 3, e só depois do TTL).
#
# USO RECOMENDADO — script de teste (sourced; o trap EXIT liberta sozinho):
#
#   #!/usr/bin/env bash
#   source "$(dirname "$0")/instance-lock.sh"
#   viber_lock_acquire "meu-teste" || exit 1   # trap EXIT já instalado
#   cargo run -p viber -- run examples/simple-rpg/world.xml
#   # release acontece no EXIT — inclusive com erro ou Ctrl-C.
#
# USO ALTERNATIVO — wrapper de um comando:
#
#   scripts/instance-lock.sh exec -- cargo run -p viber -- run world.xml
#
# MODO INTERATIVO — segurar o lock manualmente (Ctrl-C liberta):
#
#   scripts/instance-lock.sh acquire <nome-do-agente>
#
# CONSULTA:
#
#   scripts/instance-lock.sh is-locked   # exit 0 = instância a correr
#   cat /tmp/viber-instance.lock         # PID + timestamp + dono
#
# Subcomandos: acquire [dono] | release | is-locked | reap | exec -- <cmd...>
# Funções (quando sourced): viber_lock_acquire / viber_lock_release /
# viber_lock_is_locked.
#
# Override do caminho do lock: env VIBER_INSTANCE_LOCK=<ficheiro>.
# Override do tempo de vida: env VIBER_LOCK_TTL=<segundos> (0 = sem expiração).
#
# LIMPEZA AUTOMÁTICA: `is-locked` e `acquire` reciclam sozinhos um lock cujo
# dono já morreu (órfão) ou que passou do TTL (instância esquecida). Nenhum
# agente precisa de decidir manualmente se pode matar o quê.

# shellcheck shell=bash

VIBER_INSTANCE_LOCK="${VIBER_INSTANCE_LOCK:-/tmp/viber-instance.lock}"
# Tempo de vida máximo de um lock (segundos). Passado isto o lock é
# considerado ABANDONADO — uma janela de teste esquecida aberta bloqueava a
# máquina indefinidamente e obrigava a intervenção manual. `0` desliga a
# expiração (comportamento antigo).
VIBER_LOCK_TTL="${VIBER_LOCK_TTL:-7200}"

# viber_lock_field <campo>
# Lê um campo (`PID`, `TIMESTAMP`, `HOLDER`, `EPOCH`) do ficheiro de lock.
viber_lock_field() {
    sed -n "s/^$1=//p" "$VIBER_INSTANCE_LOCK" 2>/dev/null | head -1
}

# viber_lock_age_secs
# Idade do lock em segundos. Usa o campo EPOCH; se faltar (lock escrito por
# uma versão antiga do script) cai no mtime do ficheiro. Imprime vazio se não
# houver forma de datar.
viber_lock_age_secs() {
    local epoch now
    epoch="$(viber_lock_field EPOCH)"
    if [[ -z "$epoch" ]]; then
        epoch="$(stat -c %Y "$VIBER_INSTANCE_LOCK" 2>/dev/null)"
    fi
    [[ "$epoch" =~ ^[0-9]+$ ]] || return 0
    now="$(date +%s)"
    echo $(( now - epoch ))
}

# viber_lock_expired
# Exit 0 quando o lock passou do TTL (abandonado).
viber_lock_expired() {
    [[ "$VIBER_LOCK_TTL" -gt 0 ]] || return 1
    local age
    age="$(viber_lock_age_secs)"
    [[ -n "$age" ]] && (( age > VIBER_LOCK_TTL ))
}

# viber_lock_reap <pid>
# Encerra o dono de um lock EXPIRADO (SIGTERM, depois SIGKILL). Só o
# `acquire` chama isto, e só depois do TTL — nunca sobre um lock vivo dentro
# do prazo (essa é a regra "não matar instâncias de outros agentes").
viber_lock_reap() {
    local pid="$1"
    [[ "$pid" =~ ^[0-9]+$ ]] || return 0
    kill -0 "$pid" 2>/dev/null || return 0
    echo "AVISO: a encerrar instância abandonada (PID $pid, > ${VIBER_LOCK_TTL}s)." >&2
    # O dono do lock é o wrapper (`exec --`), não o jogo: matar só o wrapper
    # deixava a janela do Bevy viva. Descer aos filhos primeiro.
    pkill -TERM -P "$pid" 2>/dev/null || true
    kill -TERM "$pid" 2>/dev/null || true
    for _ in $(seq 1 20); do
        kill -0 "$pid" 2>/dev/null || return 0
        sleep 0.25
    done
    pkill -KILL -P "$pid" 2>/dev/null || true
    kill -KILL "$pid" 2>/dev/null || true
}

# viber_lock_is_locked [quiet]
# Exit 0 se o lock existe e é de um processo VIVO; exit 1 caso contrário
# (sem lock, ou lock stale de processo morto). Imprime o estado.
viber_lock_is_locked() {
    local quiet="${1:-}"
    local pid holder
    if [[ ! -f "$VIBER_INSTANCE_LOCK" ]]; then
        [[ "$quiet" == "quiet" ]] || echo "FREE: sem lock ($VIBER_INSTANCE_LOCK)"
        return 1
    fi
    pid="$(viber_lock_field PID)"
    if [[ -z "$pid" ]] || ! kill -0 "$pid" 2>/dev/null; then
        [[ "$quiet" == "quiet" ]] || echo "STALE: lock órfão (PID '${pid:-?}' morto) — acquire assume"
        return 1
    fi
    if viber_lock_expired; then
        [[ "$quiet" == "quiet" ]] || echo "EXPIRED: lock com $(viber_lock_age_secs)s > TTL ${VIBER_LOCK_TTL}s (PID $pid) — acquire assume e encerra o dono"
        return 1
    fi
    holder="$(viber_lock_field HOLDER)"
    [[ "$quiet" == "quiet" ]] || echo "LOCKED: instância a correr (PID $pid, dono: ${holder:-?}, há $(viber_lock_age_secs)s)"
    return 0
}

# viber_lock_acquire [dono]
# Adquire o lock atómicamente (noclobber). Falha (exit 1) se outro processo
# VIVO o detém — nessa caso NÃO matar: aguardar. Assume locks stale.
# Com VIBER_LOCK_SOURCED=1 (setado quando o script é sourced) instala um
# `trap viber_lock_release EXIT` (+ INT/TERM) para libertar no fim do teste.
viber_lock_acquire() {
    local holder="${1:-${VIBER_LOCK_HOLDER:-unknown}}"
    if viber_lock_is_locked quiet; then
        local cur_pid cur_ts cur_holder
        cur_pid="$(viber_lock_field PID)"
        cur_ts="$(viber_lock_field TIMESTAMP)"
        cur_holder="$(viber_lock_field HOLDER)"
        echo "ERRO: instância Viber já a correr (PID $cur_pid, dono: '${cur_holder:-?}', desde $cur_ts)." >&2
        echo "      NÃO faça kill/pkill — é a instância de teste de outro agente." >&2
        echo "      Aguarde o fim do dono (poll: instance-lock.sh is-locked) ou re-agende." >&2
        echo "      O lock recicla-se sozinho ao fim de ${VIBER_LOCK_TTL}s (VIBER_LOCK_TTL)." >&2
        return 1
    fi
    if [[ -f "$VIBER_INSTANCE_LOCK" ]]; then
        # Lock expirado: o dono ainda pode estar vivo (janela esquecida
        # aberta). Encerrá-lo aqui é o que impede que um teste abandonado
        # bloqueie a máquina — só acontece DEPOIS do TTL.
        if viber_lock_expired; then
            viber_lock_reap "$(viber_lock_field PID)"
        fi
        echo "AVISO: lock stale/expirado detectado ($(tr '\n' ' ' < "$VIBER_INSTANCE_LOCK")) — a assumir." >&2
        rm -f "$VIBER_INSTANCE_LOCK"
    fi
    # Criação atómica: noclobber falha se outro processo criar entretanto.
    if ! (set -o noclobber; printf 'PID=%s\nTIMESTAMP=%s\nEPOCH=%s\nHOLDER=%s\n' "$$" "$(date -Is)" "$(date +%s)" "$holder" > "$VIBER_INSTANCE_LOCK") 2>/dev/null; then
        echo "ERRO: perdeu a corrida pelo lock ($(tr '\n' ' ' < "$VIBER_INSTANCE_LOCK" 2>/dev/null))." >&2
        return 1
    fi
    # Modo sourced/script: garante libertação no fim (EXIT, Ctrl-C, kill TERM).
    if [[ "${VIBER_LOCK_SOURCED:-0}" == "1" ]]; then
        trap 'viber_lock_release' EXIT
        trap 'viber_lock_release; trap - INT; kill -INT $$' INT
        trap 'viber_lock_release; trap - TERM; kill -TERM $$' TERM
    fi
    echo "LOCK adquirido por '${holder}' (PID $$) → $VIBER_INSTANCE_LOCK"
    return 0
}

# viber_lock_release
# Remove o lock se fôrmos o dono registado (ou se esse PID já estiver morto).
# Idempotente: sem lock, sai 0.
viber_lock_release() {
    if [[ ! -f "$VIBER_INSTANCE_LOCK" ]]; then
        return 0
    fi
    local pid
    pid="$(viber_lock_field PID)"
    if [[ "$pid" == "$$" ]] || ! kill -0 "$pid" 2>/dev/null; then
        rm -f "$VIBER_INSTANCE_LOCK"
        echo "LOCK libertado (PID $$)"
    else
        echo "AVISO: lock pertence a outro processo vivo (PID $pid) — não libertado." >&2
        return 1
    fi
}

# ── Modo sourced vs CLI ─────────────────────────────────────────────────
# Sourced (padrão dos testes): expõe as funções e marca o modo para o
# acquire instalar o trap EXIT. CLI: roda o case de subcomandos.
if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
    VIBER_LOCK_SOURCED=1
    return 0 2>/dev/null || true
fi

case "${1:-}" in
    acquire)
        # Modo interativo: adquire e mantém o lock enquanto este processo
        # viver (Ctrl-C / SIGTERM libertam via trap). Para testes
        # automáticos prefira sourced + viber_lock_acquire, ou `exec --`.
        VIBER_LOCK_SOURCED=1
        viber_lock_acquire "${2:-interactive}" || exit 1
        while sleep 3600; do :; done
        ;;
    release)
        viber_lock_release
        ;;
    is-locked)
        viber_lock_is_locked
        ;;
    reap)
        # Limpeza explícita: liberta o lock se estiver órfão ou expirado
        # (encerrando o dono nesse caso). Não toca num lock vivo dentro do TTL.
        if [[ ! -f "$VIBER_INSTANCE_LOCK" ]]; then
            echo "FREE: sem lock ($VIBER_INSTANCE_LOCK)"
            exit 0
        fi
        if viber_lock_is_locked quiet; then
            echo "LOCKED: lock vivo e dentro do TTL (PID $(viber_lock_field PID)) — nada a fazer." >&2
            exit 1
        fi
        viber_lock_expired && viber_lock_reap "$(viber_lock_field PID)"
        rm -f "$VIBER_INSTANCE_LOCK"
        echo "LOCK reciclado (órfão/expirado)"
        ;;
    exec)
        shift
        [[ "${1:-}" == "--" ]] && shift
        if [[ $# -eq 0 ]]; then
            echo "ERRO: uso: instance-lock.sh exec -- <comando [args...]>" >&2
            exit 2
        fi
        VIBER_LOCK_SOURCED=1
        viber_lock_acquire "exec: $*" || exit 1
        # Em background + `wait`: com o comando em foreground o bash só
        # processa sinais DEPOIS de o filho sair, por isso um SIGTERM ao
        # wrapper não encerrava a janela do jogo (o lock ficava preso com o
        # dono "vivo"). Assim os sinais são reenviados ao filho na hora.
        "$@" &
        VIBER_LOCK_CHILD=$!
        trap 'kill -TERM "$VIBER_LOCK_CHILD" 2>/dev/null; viber_lock_release' EXIT
        trap 'kill -TERM "$VIBER_LOCK_CHILD" 2>/dev/null' INT TERM
        wait "$VIBER_LOCK_CHILD"
        exec_rc=$?
        # `wait` interrompido por sinal devolve >128 antes de o filho sair.
        while kill -0 "$VIBER_LOCK_CHILD" 2>/dev/null; do
            wait "$VIBER_LOCK_CHILD" 2>/dev/null && break
            sleep 0.2
        done
        exit "$exec_rc"
        ;;
    ""|-h|--help|help)
        # Imprime o cabeçalho até à linha do shellcheck (calculada, para o
        # help não desalinhar sempre que o cabeçalho cresce).
        head -n "$(($(grep -n '^# shellcheck shell=bash' "$0" | cut -d: -f1) - 1))" "$0" \
            | sed -n '2,$p' | sed -n 's/^# \{0,1\}//p'
        ;;
    *)
        echo "ERRO: subcomando desconhecido '$1' (ver: instance-lock.sh --help)." >&2
        exit 2
        ;;
esac
