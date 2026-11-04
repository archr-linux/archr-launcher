#!/bin/sh
# Perfil de CPU: a one-shot module for the archr-launcher (API 2).
# One JSON line comes in, at most one answer goes out, then it exits.
# Reads /sys only and changes nothing: no permission needed.
IFS= read -r msg || exit 0
eval "$(printf '%s' "$msg" | jq -r '@sh "id=\(.id // null | tojson) method=\(.method // "") action=\(.params.action // "") event=\(.params.name // "") system=\(.params.context.system // "")"')"
CPU=/sys/devices/system/cpu/cpu0/cpufreq

send() { printf '%s\n' "$1"; }
# A choice from the settings page, one of the governors it offers.
setting() {
    value=$(jq -r --arg k "$1" '.[$k] // empty' "$ARCHR_MODULE_SETTINGS" 2>/dev/null)
    case "$value" in performance | schedutil | ondemand | powersave) printf '%s' "$value" ;; *) printf '%s' "$2" ;; esac
}
# The governor and clock in use right now, on the page's "Governor" row.
current() {
    governor=$(cat "$CPU/scaling_governor" 2>/dev/null || echo "?")
    khz=$(cat "$CPU/scaling_cur_freq" 2>/dev/null || echo 0)
    send "$(jq -cn --arg g "$governor" --arg mhz "$((khz / 1000))" \
        '{jsonrpc: "2.0", method: "ui.update", params: {target: "row:agora", value: {key: "@now", args: {gov: $g, mhz: $mhz}}}}')"
}

case "$method/$action$event" in
action.invoke/sugerir)
    case "$system" in
        n64 | psp | dreamcast | saturn | nds | psx) governor=$(setting pesados performance) ;;
        nes | gb | gbc | gamegear | mastersystem | atari2600 | pcengine) governor=$(setting leves powersave) ;;
        *) governor=schedutil ;;
    esac
    send "$(jq -cn --arg s "$system" --arg g "$governor" \
        '{jsonrpc: "2.0", method: "ui.notify", params: {key: ("@suggest." + $g), args: {system: $s}}}')"
    ;;
action.invoke/verificar | event/launcher.started) current ;;
esac
# Events need no answer; unknown requests get "method not found".
if [ "$id" != null ]; then
    if [ "$method" = action.invoke ]; then
        send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}"
    else
        send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"error\":{\"code\":-32601,\"message\":\"unknown method\"}}"
    fi
fi
