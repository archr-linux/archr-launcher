#!/bin/bash
# Bateria detalhada: a daemon module for the archr-launcher (API 2).
# One JSON-RPC object per line on stdin/stdout; stderr is the log.
# Reads /sys/class/power_supply only: no permission needed.
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
    case "$interval" in 5 | 15 | 30) ;; *) interval=15 ;; esac
}
value() { [ -r "$1/$2" ] && tr -d '\n' < "$1/$2"; }
# The first of "now" or "avg" the driver offers (the RK817 has only avg).
either() { value "$1" "$2_now" || value "$1" "$2_avg"; }
battery() {
    for dir in "$PS"/*; do
        [ "$(value "$dir" type)" = Battery ] && { printf '%s' "$dir"; return; }
    done
}
update() {
    local dir state
    dir=$(battery)
    if [ -z "$dir" ]; then
        # No battery (a PC): the tile goes away.
        send '{"jsonrpc":"2.0","method":"ui.update","params":{"target":"tile:bateria","hidden":true}}'
        return
    fi
    # One jq call builds every update: tile and rows.
    state=$(jq -cn --arg pct "$(value "$dir" capacity)" --arg st "$(value "$dir" status)" \
        --arg ua "$(either "$dir" current)" --arg uv "$(either "$dir" voltage)" \
        --arg full "$(value "$dir" charge_full)" --arg design "$(value "$dir" charge_full_design)" \
        --argjson watts "$watts" '
        def num: tonumber? // null;
        ($ua | num) as $a | ($uv | num) as $v |
        (if $a != null and $v != null then (($a | fabs) * $v / 1e11 | round) / 10 else null end) as $w |
        (if $watts and $w != null and $w > 0 and ($st == "Discharging" or $st == "Charging") then
            {key: (if $st == "Charging" then "@st.watts_in" else "@st.watts" end), args: {pct: $pct, w: ($w | tostring)}}
         elif $st == "Charging" or $st == "Full" then {key: ("@st." + $st), args: {pct: $pct}}
         elif $st == "Not charging" then {key: "@st.Idle", args: {pct: $pct}}
         else {key: "@st.Discharging", args: {pct: $pct}} end) as $now |
        (if $v != null then "\(($v / 1e4 | round) / 100) V" else "—" end) as $volts |
        (if ($full | num) != null and ($design | num // 0) > 0
         then "\(($full | num) * 100 / ($design | num) | floor) %" else "—" end) as $health |
        [{target: "tile:bateria", value: $now}, {target: "row:agora", value: $now},
         {target: "row:tensao", value: $volts}, {target: "row:saude", value: $health}][] |
        {jsonrpc: "2.0", method: "ui.update", params: .}')
    [ -n "$state" ] && send "$state"
}
handle() {
    local id method name
    eval "$(printf '%s' "$1" | jq -r '@sh "id=\(.id // null | tojson) method=\(.method // "") name=\(.params.name // "")"')"
    case "$method" in
        initialize) load; reply "$id" '{"ready":true}'; update ;;
        event) [ "$name" = settings.changed ] && { load; update; } ;;
        action.invoke) update; reply "$id" '{}' ;;
        shutdown) reply "$id" '{}'; exit 0 ;;
        *) [ "$id" = null ] || send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"error\":{\"code\":-32601,\"message\":\"unknown method\"}}" ;;
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
