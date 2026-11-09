#!/bin/sh
# Host test of the catalogo-demo app (sh + jq), without the launcher: the
# screens it shows and the requests it makes for what the user does.
#   sh examples/modules/catalogo-demo.test.sh
set -u
MODULE=$(cd "$(dirname "$0")/catalogo-demo" && pwd)
DATA=$(mktemp -d)
trap 'rm -rf "${DATA}"' EXIT
FAILED=0
check() {
  if eval "$2"; then echo "ok   $1"; else echo "FAIL $1"; FAILED=1; fi
}
echo '{"tags":true}' >"${DATA}/settings.json"
# The lines a launcher would send, one session; the module's ids start at 11.
run() {
  HOME=${DATA} ARCHR_MODULE_DIR=${MODULE} ARCHR_MODULE_SETTINGS=${DATA}/settings.json sh "${MODULE}/main.sh" >"${DATA}/out"
}
said() { jq -c "select($1)" "${DATA}/out"; }

{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"role":"app"}}'
  echo '{"jsonrpc":"2.0","id":11,"result":{"games":[]}}'
  echo '{"jsonrpc":"2.0","method":"screen.opened","params":{"screen":"catalog"}}'
  echo '{"jsonrpc":"2.0","method":"screen.more","params":{"screen":"catalog","count":3}}'
  echo '{"jsonrpc":"2.0","method":"screen.select","params":{"screen":"catalog","item":"g2"}}'
  echo '{"jsonrpc":"2.0","id":2,"method":"action.invoke","params":{"action":"get","context":{"screen":"game:g2","button":"get"}}}'
  echo '{"jsonrpc":"2.0","id":12,"result":{"game_key":"gb/Blue Lines.gb"}}'
  echo '{"jsonrpc":"2.0","id":3,"method":"action.invoke","params":{"action":"remove","context":{"screen":"game:g2","button":"remove"}}}'
  echo '{"jsonrpc":"2.0","method":"screen.answer","params":{"id":"remove:g2","yes":true}}'
  echo '{"jsonrpc":"2.0","id":4,"method":"shutdown","params":{}}'
} | run

check "ready, its summary and the list of what it installed" \
  '[ "$(said ".id == 1" | jq .result.ready)" = true ] && said ".params.target == \"app:summary\"" | grep -q "@summary" && [ "$(said ".method == \"rom.list\"" | jq .id)" = 11 ]'
check "the catalog: three items, chips, more to come" \
  '[ "$(said ".method == \"screen.show\" and .params.id == \"catalog\"" | head -n 1 | jq "[(.params.items | length), (.params.chips | length), .params.more] | join(\",\")" -r)" = "3,2,true" ]'
check "covers in its own folder, by relative path" \
  '[ -f "${DATA}/cache/covers/g1.png" ] && said ".params.id == \"catalog\"" | head -n 1 | grep -q "\"image\":\"cache/covers/g1.png\""'
check "the fourth on screen.more" \
  '[ "$(said ".method == \"screen.patch\" and .params.append != null" | jq ".params.append[0].id" -r)" = g4 ]'
check "the detail offers Get" \
  '[ "$(said ".params.id == \"game:g2\"" | head -n 1 | jq ".params.buttons[0].action" -r)" = get ]'
check "Get: a bar, then rom.install of a file it made" \
  'said ".params.target == \"button:get\"" | grep -q "\"progress\":60" && [ "$(said ".method == \"rom.install\"" | jq -r "[.id, .params.system, .params.file] | join(\" \")")" = "12 gb downloads/g2.gb" ] && [ "$(wc -c <"${DATA}/downloads/g2.gb")" -eq 32768 ]'
check "installed: Play and Remove, and the tag in the catalog" \
  'said ".params.id == \"game:g2\"" | sed -n 2p | grep -q "\"action\":\"play\"" && said ".params.target == \"item:g2\"" | grep -q "@badge.installed"'
check "Remove asks first" \
  'said ".method == \"screen.confirm\"" | grep -q "\"id\":\"remove:g2\""'
check "after the yes, rom.remove of its own game" \
  '[ "$(said ".method == \"rom.remove\"" | jq -r .params.game_key)" = "gb/Blue Lines.gb" ]'
check "shutdown answered" '[ "$(said ".id == 4" | jq -c .result)" = "{}" ]'
exit "${FAILED}"
