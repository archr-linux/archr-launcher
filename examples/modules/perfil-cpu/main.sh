#!/bin/sh
# Perfil de CPU: a one-shot module for the archr-launcher (API 1).
# One JSON line comes in, at most one answer goes out, then it exits.
IFS= read -r msg || exit 0
eval "$(printf '%s' "$msg" | jq -r '@sh "id=\(.id // "null") method=\(.method // "") action=\(.params.action // "") system=\(.params.context.system // "")"')"

send() { printf '%s\n' "$1"; }
reply() { send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}"; }
setting() {
    value=$(jq -r --arg k "$1" '.[$k] // empty' "$ARCHR_MODULE_SETTINGS" 2>/dev/null)
    printf '%s' "${value:-$2}"
}

if [ "$method" != action.invoke ]; then
    # Events need no answer; unknown requests get "method not found".
    [ "$id" = null ] || send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"error\":{\"code\":-32601,\"message\":\"$method\"}}"
    exit 0
fi

case "$action" in
sugerir)
    case "$system" in
        n64|psp|dreamcast|saturn|nds|psx) governor=$(setting pesados performance) ;;
        nes|gb|gbc|gamegear|mastersystem|atari2600|pcengine) governor=$(setting leves powersave) ;;
        *) governor=schedutil ;;
    esac
    send "$(jq -cn --arg s "$system" --arg g "$governor" \
        '{jsonrpc: "2.0", method: "ui.notify", params: {key: ("@suggest." + $g), args: {system: $s}, level: "info"}}')"
    ;;
verificar)
    cpu=/sys/devices/system/cpu/cpu0/cpufreq
    governor=$(cat "$cpu/scaling_governor" 2>/dev/null || echo "?")
    khz=$(cat "$cpu/scaling_cur_freq" 2>/dev/null || echo 0)
    send "$(jq -cn --arg g "$governor" --arg mhz "$((khz / 1000))" \
        '{jsonrpc: "2.0", method: "ui.update", params: {target: "row:agora", value: {key: "@now", args: {gov: $g, mhz: $mhz}}}}')"
    ;;
esac
reply
