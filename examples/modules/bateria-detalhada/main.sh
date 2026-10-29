#!/bin/bash
# Bateria detalhada: a daemon module for the archr-launcher (API 1).
# One JSON-RPC object per line on stdin/stdout; stderr is the log.
PS=/sys/class/power_supply
watts=true
interval=15

send() { printf '%s\n' "$1"; }
reply() { send "{\"jsonrpc\":\"2.0\",\"id\":$1,\"result\":$2}"; }
# Settings live in a file only the launcher writes.
load() {
    watts=$(jq -r 'if .watts == false then "false" else "true" end' "$ARCHR_MODULE_SETTINGS" 2>/dev/null)
    interval=$(jq -r '.intervalo // "15"' "$ARCHR_MODULE_SETTINGS" 2>/dev/null)
    watts=${watts:-true}
    interval=${interval:-15}
}
value() { [ -r "$1/$2" ] && tr -d '\n' < "$1/$2"; }
battery() {
    for dir in "$PS"/*; do
        [ "$(value "$dir" type)" = Battery ] && { printf '%s' "$dir"; return; }
    done
}
update() {
    local dir now health
    dir=$(battery)
    if [ -z "$dir" ]; then
        # No battery (a PC): the tile goes away.
        send '{"jsonrpc":"2.0","method":"ui.update","params":{"target":"tile:bateria","hidden":true}}'
        return
    fi
    now=$(jq -cn --arg pct "$(value "$dir" capacity)" --arg st "$(value "$dir" status)" \
        --arg ua "$(value "$dir" current_now)" --arg uv "$(value "$dir" voltage_now)" \
        --argjson watts "$watts" '
        if $watts and $st == "Discharging" and $ua != "" and $uv != "" then
            {key: "@st.watts", args: {pct: $pct,
             w: ((($ua | tonumber | fabs) * ($uv | tonumber) / 1e11 | round) / 10 | tostring)}}
        elif $st == "Charging" or $st == "Full" then {key: ("@st." + $st), args: {pct: $pct}}
        else {key: "@st.Discharging", args: {pct: $pct}} end')
    health=$(jq -cn --arg full "$(value "$dir" charge_full)" --arg design "$(value "$dir" charge_full_design)" '
        if $full != "" and $design != "" and ($design | tonumber) > 0
        then "\(($full | tonumber) * 100 / ($design | tonumber) | floor) %" else "—" end')
    for target in tile:bateria row:agora; do
        send "{\"jsonrpc\":\"2.0\",\"method\":\"ui.update\",\"params\":{\"target\":\"$target\",\"value\":$now}}"
    done
    send "{\"jsonrpc\":\"2.0\",\"method\":\"ui.update\",\"params\":{\"target\":\"row:saude\",\"value\":$health}}"
}
handle() {
    local id method name action
    eval "$(printf '%s' "$1" | jq -r '@sh "id=\(.id // "null") method=\(.method // "") name=\(.params.name // "") action=\(.params.action // "")"')"
    case "$method" in
        initialize) load; reply "$id" '{"ready":true}'; update ;;
        event) [ "$name" = settings.changed ] && { load; update; } ;;
        action.invoke) update; reply "$id" '{}' ;;
        shutdown) reply "$id" '{}'; exit 0 ;;
        *) [ "$id" = null ] || send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"error\":{\"code\":-32601,\"message\":\"$method\"}}" ;;
    esac
}
while true; do
    if IFS= read -r -t "$interval" line; then
        handle "$line"
    elif [ $? -gt 128 ]; then
        update  # time to refresh
    else
        exit 0  # end of input: the launcher is leaving
    fi
done
