#!/bin/bash
# Automatic CPU tuning (archr-launcher module, API 3). Before each game it
# chooses the CPU ceiling it is learning for that game; after the game the
# launcher hands it what archr-perf-probe measured, and tune.jq moves the
# ceiling: one step down after a session at full speed, one step up after a
# slow one. Only the CPU's ceiling of that launch; never the GPU or memory.
DATA="${ARCHR_MODULE_DATA:-$HOME}/games.json"
TUNE="${ARCHR_MODULE_DIR:-$(dirname "$0")}/tune.jq"
OPPS='[]'
FULL=0
read -r FULL 2>/dev/null </sys/class/power_supply/battery/charge_full
[[ "${FULL}" =~ ^[0-9]+$ ]] || FULL=0
[ -s "${DATA}" ] || echo '{"games":{},"pending":null}' >"${DATA}"

# Each game is known by a short id: the launcher limits item ids to 40
# characters and a game key is a path.
game_id() { printf 'g%s' "$(printf '%s' "$1" | cksum | cut -d' ' -f1)"; }

handle() {
    local line=$1 id method name item key gid
    eval "$(jq -r '@sh "id=\(.id // "null") method=\(.method // "") name=\(.params.name // "") item=\(.params.context.item // "") key=\(.params.game.game_key // .params.data.game.game_key // "")"' <<<"${line}")" || return
    case "${method}" in
        initialize) OPPS=$(jq -c '.params.cpu.opps // [] | sort' <<<"${line}") ;;
        shutdown)
            printf '{"jsonrpc":"2.0","id":%s,"result":{}}\n' "${id}"
            exit 0
            ;;
        launch.before | action.invoke) ;;
        event) [ "${name}" = game.ended ] || return ;;
        *)
            [ "${id}" = null ] || printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32601,"message":"%s"}}\n' "${id}" "${method}"
            return
            ;;
    esac
    gid=""
    [ -n "${key}" ] && gid=$(game_id "${key}")
    # The new state first, then the lines for the launcher.
    {
        local state
        read -r state
        if [ "${state}" != null ] && [ -n "${state}" ]; then
            printf '%s\n' "${state}" >"${DATA}.tmp" && mv -f "${DATA}.tmp" "${DATA}"
        fi
        cat
    } < <(jq -c -n --slurpfile st "${DATA}" --argjson msg "${line}" --argjson opps "${OPPS}" \
        --arg gid "${gid}" --arg item "${item}" --argjson full "${FULL}" -f "${TUNE}")
}

while IFS= read -r line; do
    handle "${line}"
done
