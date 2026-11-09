#!/bin/sh
# Demo catalog (archr-launcher module, API 4): an app with its own screens.
# The launcher draws them; this only says what they hold and installs test
# cartridges through rom.install. No network: the catalog and its covers
# come with the module and are copied to HOME, where screens read pictures.
DIR=${ARCHR_MODULE_DIR:-$(dirname "$0")}
CATALOG="$DIR/catalog.json"
# Item id -> game key, as rom.install answered.
STATE="$HOME/installed.json"
FILTER=all
NEXT=10
# "<request id> <what> <item>" of the answer being waited for.
WAIT=""

send() { printf '%s\n' "$1"; }
reply() { send "{\"jsonrpc\":\"2.0\",\"id\":$1,\"result\":$2}"; }
request() { # method, params, what, item
    NEXT=$((NEXT + 1))
    WAIT="$NEXT $3 $4"
    send "{\"jsonrpc\":\"2.0\",\"id\":$NEXT,\"method\":\"$1\",\"params\":$2}"
}
toast() { # text, tone
    send "$(jq -cn --arg t "$1" --arg tone "${2:-}" '{jsonrpc:"2.0",method:"screen.toast",params:{text:$t,tone:$tone}}')"
}
tags() {
    jq -r 'if .tags == false then "false" else "true" end' "$ARCHR_MODULE_SETTINGS" 2>/dev/null || echo true
}
key_of() { jq -r --arg id "$1" '.[$id] // empty' "$STATE"; }
remember() { # item, game key ("" forgets it)
    jq -c --arg id "$1" --arg key "$2" 'if $key == "" then del(.[$id]) else .[$id] = $key end' \
        "$STATE" >"$STATE.tmp" && mv "$STATE.tmp" "$STATE"
}

prepare() {
    mkdir -p "$HOME/cache/covers" "$HOME/downloads"
    cp "$DIR"/covers/*.png "$HOME/cache/covers/"
    [ -s "$STATE" ] || echo '{}' >"$STATE"
}
# The catalog as screen items: "all" or "installed", and a search text.
items() {
    jq -c --slurpfile s "$STATE" --arg filter "$1" --arg q "$2" --argjson tags "$(tags)" '
        [.[] | select($filter != "installed" or $s[0][.id] != null)
             | select($q == "" or (.title | ascii_downcase | contains($q | ascii_downcase)))
             | {id, title, subtitle: ("Game Boy · " + .developer),
                image: ("cache/covers/" + .id + ".png")}
               + if $tags | not then {}
                 elif $s[0][.id] then {badge: "@badge.installed", tone: "good"}
                 else {badge: "@badge.free", tone: "info"} end]' "$CATALOG"
}
# Three items first: the fourth comes with screen.more.
show_catalog() {
    send "$(jq -cn --argjson all "$(items "$FILTER" "")" --arg f "$FILTER" '{jsonrpc:"2.0",method:"screen.show",params:{
        id:"catalog", title:"@title.catalog", layout:"collection",
        chips:[{id:"all",label:"@chip.all",selected:($f == "all")},
               {id:"installed",label:"@chip.installed",selected:($f == "installed")}],
        items:$all[0:3], more:(($all | length) > 3),
        buttons:{y:{label:"@about",action:"about"}}, footer:"@footer"}}')"
}
more() {
    send "$(jq -cn --argjson all "$(items "$FILTER" "")" '{jsonrpc:"2.0",method:"screen.patch",params:{
        screen:"catalog", append:$all[3:], more:false}}')"
}
search() {
    send "$(jq -cn --argjson all "$(items all "$1")" --arg q "$1" '{jsonrpc:"2.0",method:"screen.show",params:{
        id:"search", title:{key:"@title.search",args:{q:$q}}, layout:"collection", items:$all}}')"
}
show_detail() {
    send "$(jq -c --slurpfile s "$STATE" --arg id "$1" '.[] | select(.id == $id) | $s[0][$id] as $key
        | {jsonrpc:"2.0",method:"screen.show",params:{
        id:("game:" + .id), title:.title, layout:"detail", image:("cache/covers/" + .id + ".png"),
        subtitle:("Game Boy · " + .developer + " · " + (.year | tostring)),
        facts:[{title:"@fact.size",value:.size},
               {title:"@fact.status",value:(if $key then "@status.installed" else "@status.free" end),
                tone:(if $key then "good" else "info" end)}],
        text:.text,
        buttons:(if $key then [{id:"play",label:"@play",icon:"play",action:"play"},
                               {id:"remove",label:"@remove",icon:"trash-2",action:"remove"}]
                 else [{id:"get",label:"@get",icon:"download",action:"get"}] end)}}' "$CATALOG")"
}
# The catalog's tag for one item, after it was installed or removed.
retag() {
    [ "$(tags)" = true ] || return
    if [ -n "$(key_of "$1")" ]; then set -- "$1" "@badge.installed" good; else set -- "$1" "@badge.free" info; fi
    send "{\"jsonrpc\":\"2.0\",\"method\":\"screen.patch\",\"params\":{\"screen\":\"catalog\",\"target\":\"item:$1\",\"badge\":\"$2\",\"tone\":\"$3\"}}"
}
# A test cartridge made here, the button's bar on the way, then rom.install.
get() {
    head -c 32768 /dev/zero >"$HOME/downloads/$1.gb"
    for p in 30 60 90; do
        send "{\"jsonrpc\":\"2.0\",\"method\":\"screen.patch\",\"params\":{\"screen\":\"game:$1\",\"target\":\"button:get\",\"progress\":$p,\"label\":\"@getting\"}}"
    done
    request rom.install "$(jq -c --arg id "$1" '.[] | select(.id == $id) | {system:"gb",
        file:("downloads/" + $id + ".gb"), name:.title, cover:("cache/covers/" + $id + ".png"),
        meta:{developer, description:.text, year}}' "$CATALOG")" install "$1"
}
act() { # action, item
    case "$1" in
        about) toast "@about.text" ;;
        get) get "$2" ;;
        play) request game.launch "{\"system\":\"gb\",\"game_key\":$(jq -cn --arg k "$(key_of "$2")" '$k')}" play "$2" ;;
        # The launcher asks the user itself, and answers {removed}.
        remove) request rom.remove "{\"game_key\":$(jq -cn --arg k "$(key_of "$2")" '$k')}" remove "$2" ;;
    esac
}
# The answer to the request being waited for: $1 the whole line.
answered() {
    set -- "$1" $WAIT
    [ "$2" = "$id" ] || return
    WAIT=""
    if [ -n "$failed" ]; then
        toast "$failed" bad
        [ "$3" = install ] && show_detail "$4"
        return
    fi
    case "$3" in
        install) remember "$4" "$key"; show_detail "$4"; retag "$4"; toast "@installed" good ;;
        remove)
            [ "$removed" = true ] || return
            remember "$4" ""; show_detail "$4"; retag "$4"; toast "@removed"
            ;;
        list)
            # Only what the launcher says is still there counts.
            keys=$(printf '%s' "$1" | jq -c '[.result.games[].game_key]')
            jq -c --argjson keys "$keys" 'with_entries(select(.value as $v | $keys | index($v)))' \
                "$STATE" >"$STATE.tmp" && mv "$STATE.tmp" "$STATE"
            ;;
    esac
}
# What the user does on the screens, as the launcher tells it.
on() { # method
    case "$1" in
        screen.opened) show_catalog ;;
        screen.select) show_detail "$item" ;;
        screen.chip) FILTER=$chip; show_catalog ;;
        screen.more) more ;;
        screen.search) search "$text" ;;
    esac
}
handle() {
    eval "$(printf '%s' "$1" | jq -r '@sh "id=\(.id // "null") method=\(.method // "") action=\(.params.action // "") screen=\(.params.context.screen // "") item=\(.params.item // .params.context.item // "") chip=\(.params.chip // "") text=\(.params.text // "") failed=\(.error.message // "") key=\(.result.game_key // "") removed=\(.result.removed // false)"')" || return
    case "$method" in
        initialize)
            prepare
            reply "$id" '{"ready":true}'
            send "$(jq -cn --arg n "$(jq length "$CATALOG")" '{jsonrpc:"2.0",method:"ui.update",params:{target:"app:summary",value:{key:"@summary",args:{n:$n}}}}')"
            request rom.list '{}' list ""
            ;;
        shutdown) reply "$id" '{}'; exit 0 ;;
        action.invoke) reply "$id" '{}'; act "$action" "${screen#game:}" ;;
        screen.*) on "$method" ;;
        "") answered "$1" ;;
        *) [ "$id" = null ] || send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"error\":{\"code\":-32601,\"message\":\"$method\"}}" ;;
    esac
}
while IFS= read -r line; do
    handle "$line"
done
