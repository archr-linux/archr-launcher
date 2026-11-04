#!/bin/bash
# Host test of the ajuste-cpu module (bash + jq): how it learns from the
# measurements the launcher hands it, and the list it sends.
#   bash examples/modules/ajuste-cpu.test.sh
set -u
MODULE=$(cd "$(dirname "$0")/ajuste-cpu" && pwd)
DATA=$(mktemp -d)
trap 'rm -rf "${DATA}"' EXIT
FAILED=0
check() {
  if eval "$2"; then echo "ok   $1"; else echo "FAIL $1"; FAILED=1; fi
}
OPPS='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"cpu":{"opps":[408000,816000,1008000,1416000]}}}'
before() { # game key
  printf '{"jsonrpc":"2.0","id":2,"method":"launch.before","params":{"game":{"system":"%s","game_key":"%s","name":"%s"}}}\n' \
    "${1%%/*}" "$1" "${1##*/}"
}
ended() { # game key, kHz, p95, seconds at the limit, idle seconds
  printf '{"jsonrpc":"2.0","method":"event","params":{"name":"game.ended","data":{"game":{"game_key":"%s"},"perf":{"cpu_max_khz":%s,"seconds":150,"busy_p95":%s,"limit_seconds":%s,"idle_seconds":%s,"current_ma":400}}}}\n' "$@"
}
# One launch and its measurement; prints the ceiling the module chose.
session() { # game key, p95, limit seconds, idle seconds
  local reply cap
  reply=$( { echo "${OPPS}"; before "$1"; } | ARCHR_MODULE_DATA=${DATA} ARCHR_MODULE_DIR=${MODULE} bash "${MODULE}/main.sh")
  cap=$(jq -r 'select(.id == 2) | .result.changes.cpu_max_khz // 1416000' <<<"${reply}")
  { echo "${OPPS}"; ended "$1" "${cap}" "$2" "$3" "$4"; } |
    ARCHR_MODULE_DATA=${DATA} ARCHR_MODULE_DIR=${MODULE} bash "${MODULE}/main.sh" >"${DATA}/last"
  echo "${cap}"
}
game() { jq -c --arg k "$1" '.games[] | select(.key == $k)' "${DATA}/games.json"; }

# Clear room: down one step a session.
check "starts at the top" '[ "$(session snes/a 50 0 0)" = 1416000 ]'
check "then one step down" '[ "$(session snes/a 50 0 0)" = 1008000 ]'
# One lone second at the limit in 150 (a loading) still leaves clear room.
session snes/z 70 1 0 >/dev/null
check "a lone second at the limit: one step down" '[ "$(game snes/z | jq .cap)" = 1008000 ]'
# A paused game (idle most of the time) is no evidence: the ceiling stays.
session snes/a 10 0 120 >/dev/null
check "an idle session does not count" '[ "$(game snes/a | jq .sessions)" = 2 ] && [ "$(game snes/a | jq .cap)" = 816000 ]'
# Busy but not at the limit: no room to go down; steady sessions learn it.
for _ in 1 2 3; do session snes/a 85 0 0 >/dev/null; done
check "no clear room: stays and learns" '[ "$(game snes/a | jq -r .status)" = learned ] && [ "$(game snes/a | jq .cap)" = 816000 ]'
# At the limit: one step up.
session snes/a 97 10 0 >/dev/null
check "at the limit: one step up" '[ "$(game snes/a | jq .cap)" = 1008000 ] && [ "$(game snes/a | jq .floor)" = 816000 ]'
# Already at the limit at the top: left out.
session snes/b 97 40 0 >/dev/null
check "slow at the top: left out" '[ "$(game snes/b | jq -r .status)" = out ]'
check "heavy systems are left out" '[ "$(session n64/c 50 0 0)" = 1416000 ] && [ "$(game n64/c | jq -r .status)" = out ]'

# Pages: 45 games, 18 a page, with the next and previous pages as items.
for n in $(seq 1 41); do session "gba/g$(printf %02d "${n}")" 50 0 0 >/dev/null; done
list() { grep '"row:jogos"' "${DATA}/last" | tail -n 1 | jq -c '.params.items'; }
press() { # action, item; prints what the module said on stderr
  # shellcheck disable=SC2069 # stderr to the caller, stdout to the file
  { echo "${OPPS}"; printf '{"jsonrpc":"2.0","id":3,"method":"action.invoke","params":{"action":"%s","context":{"item":"%s"}}}\n' "$1" "$2"; } |
    ARCHR_MODULE_DATA=${DATA} ARCHR_MODULE_DIR=${MODULE} bash "${MODULE}/main.sh" 2>&1 >"${DATA}/last"
}
check "45 games" '[ "$(jq ".games | length" "${DATA}/games.json")" = 45 ]'
check "page 1: 18 games and the next page" '[ "$(list | jq length)" = 19 ] && [ "$(list | jq -r ".[18].id")" = pagina-seguinte ] && [ "$(list | jq ".[18].title.args.n")" = 27 ]'
check "learning ones first" '[ "$(list | jq -r ".[0].badge")" = "@badge.learning" ]'
check "page items do not open the detail page" '[ "$(list | jq ".[18].open")" = false ] && [ "$(list | jq -r ".[18].action")" = pagina ]'
press pagina pagina-seguinte
check "page 2: back, 18 games, next" '[ "$(list | jq length)" = 20 ] && [ "$(list | jq -r ".[0].id")" = pagina-anterior ] && [ "$(list | jq ".[19].title.args.n")" = 9 ]'
press pagina pagina-seguinte
check "page 3: back and the last 9" '[ "$(list | jq length)" = 10 ] && [ "$(list | jq -r ".[9].id")" != pagina-seguinte ]'
press pagina pagina-anterior
press pagina pagina-anterior
check "back to page 1" '[ "$(list | jq -r ".[0].id")" != pagina-anterior ] && [ "$(list | jq length)" = 19 ]'

# A games.json of version 1.0.0 ("speed", no "busy" nor "limit").
OLD="g$(printf %s psx/old | cksum | cut -d' ' -f1)"
printf '{"games":{"%s":{"cap":816000,"floor":null,"stable":1,"sessions":2,"status":"learning","name":"Old","system":"psx","key":"psx/old","sysname":"PlayStation","core":"x","hist":{"1416000":{"n":1,"speed":100,"ma":44,"man":1},"816000":{"n":1,"speed":100,"ma":30,"man":1}}}},"pending":null}\n' "${OLD}" >"${DATA}/games.json"
# shellcheck disable=SC2034 # read by check's eval
problems=$(press detalhes "${OLD}")
check "old data: the details still come" '[ -z "${problems}" ] && grep -q "row:fatos" "${DATA}/last"'
check "old data: no bar without occupancy" '[ "$(grep "row:barras" "${DATA}/last" | jq ".params.items | length")" = 0 ]'
session psx/old 60 0 0 >/dev/null
check "old data: the next session starts over" '[ "$(game psx/old | jq ".hist[\"816000\"] | [.n, .busy] | join(\",\")" -r)" = "1,60" ]'
[ "${FAILED}" = 0 ] || head -c 600 "${DATA}/last"
exit ${FAILED}
