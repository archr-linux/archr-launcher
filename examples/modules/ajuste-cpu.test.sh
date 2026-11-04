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

# More games than one update takes: learning ones first, the rest counted.
for n in $(seq 1 24); do session "gba/g${n}" 50 0 0 >/dev/null; done
# shellcheck disable=SC2034 # read by check's eval
items=$(grep '"row:jogos"' "${DATA}/last" | tail -n 1 | jq -c '.params.items')
check "at most 20 items" '[ "$(jq length <<<"${items}")" = 20 ]'
check "learning ones first" '[ "$(jq -r ".[0].badge" <<<"${items}")" = "@badge.learning" ]'
check "the rest is counted" '[ "$(jq -r ".[19].title.key" <<<"${items}")" = "@more" ] && [ "$(jq ".[19].title.args.n" <<<"${items}")" = 8 ]'
[ "${FAILED}" = 0 ] || head -c 600 "${DATA}/last"
exit ${FAILED}
