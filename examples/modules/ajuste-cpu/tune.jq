# The module's logic, one launcher message at a time. In: $st[0] (the
# state), $msg, $opps (the CPU's ceilings, kHz, lowest first), $gid (the
# message's game), $item (the item an action is about), $full (battery
# µAh). Out: the new state (or null), then one line per launcher message.

# Sessions shorter than this many measured seconds do not count (the probe
# skips the first 30 s, so about two minutes of play).
def least: ($ENV.ARCHR_TUNE_MIN // "90" | tonumber);
# A second the busiest thread spends above this (%) could not keep the pace.
def limit: 92;
# Systems that need the most this CPU has.
def heavy: IN("n64", "psp", "dreamcast", "saturn");
def stable_sessions: 3;

def top: $opps | last // 0;
def mhz: "\(. / 1000 | floor) MHz";
def below($c): [$opps[] | select(. < $c)] | last;
def above($c): [$opps[] | select(. > $c)] | first;
def speed($h): if ($h.n // 0) > 0 then ($h.speed / $h.n | floor) else null end;
def current($h): if ($h.man // 0) > 0 then $h.ma / $h.man else null end;
# Battery saved at the game's ceiling against the highest one, in %.
def saving:
  (current(.hist[.cap | tostring])) as $c
  | (current(.hist[top | tostring])) as $m
  | if $c != null and $m != null and $m > 0 and .cap < top
    then ((1 - $c / $m) * 100 | round) else null end;
def minutes:
  (current(.hist[.cap | tostring])) as $c
  | (current(.hist[top | tostring])) as $m
  | if $c != null and $m != null and $c > 0 and $m > 0 and $full > 0 and .cap < top
    then ($full / 1000 * 60 / $c - $full / 1000 * 60 / $m | round) else null end;
def minus: "−\(.)%";
def out: .status == "out" or .pinned == true;
def order: if out then 2 elif .status == "learned" then 0 else 1 end;

def reply($result): {jsonrpc: "2.0", id: $msg.id, result: $result};
def update($target; $fields): {jsonrpc: "2.0", method: "ui.update", params: ({target: $target} + $fields)};

def item($id):
  . as $g
  | {id: $id, title: .name, action: "detalhes"}
  + if out then {
      subtitle: {key: (if .pinned then "@sub.pinned" else "@sub.out" end), args: {system: .sysname}},
      badge: "@badge.out", tone: "muted",
      detail: {key: "@detail.always", args: {mhz: (top | mhz)}}
    } elif .status == "learned" then {
      subtitle: {key: "@sub.learned", args: {system: .sysname, speed: (speed(.hist[.cap | tostring]) // 100), n: .sessions}},
      value: (.cap | mhz),
      detail: ($g | saving | if . == null or . <= 0 then "" else {key: "@detail.saving", args: {pct: .}} end)
    } else {
      subtitle: {key: "@sub.testing", args: {system: .sysname, mhz: (.cap | mhz)}},
      badge: "@badge.learning", tone: "info",
      detail: {key: "@detail.session", args: {n: ([.stable + 1, stable_sessions] | min), of: stable_sessions}}
    } end;

def page:
  .games as $games
  | [$games | to_entries[] | select(.value.status == "learned" and (.value.pinned | not)) | .value] as $learned
  | [$learned[] | saving | select(. != null)] as $savings
  | update("row:resumo"; {items: [
      {title: "@stat.saving", value: (if $savings == [] then "—" else ($savings | add / length | round | minus) end),
       subtitle: "@stat.saving_sub", tone: (if $savings == [] then "plain" else "good" end)},
      {title: "@stat.learned", value: ($learned | length | tostring), subtitle: "@stat.learned_sub"},
      {title: "@stat.learning", value: ([$games[] | select(.status == "learning" and (.pinned | not))] | length | tostring),
       subtitle: {key: "@stat.learning_sub", args: {n: stable_sessions}}}
    ]}),
    update("row:jogos"; {items: (
      [$games | to_entries | sort_by((.value | order), .value.name) | .[:20][] | .key as $id | .value | item($id)]
      | if . == [] then [{title: "@empty", subtitle: "@empty_sub"}] else . end)});

def detail($id):
  .games[$id] as $g
  | if $g == null then empty else
    update("row:cabeca"; {items: [{
      id: $id, title: $g.name,
      subtitle: {key: "@sub.core", args: {system: $g.sysname, core: $g.core}},
      badge: (if $g | out then "@badge.out" elif $g.status == "learned" then "@badge.learned" else "@badge.learning" end),
      tone: (if $g | out then "muted" elif $g.status == "learned" then "good" else "info" end),
      game: {system: $g.system, game_key: $g.key}
    }]}),
    update("row:barras"; {items: [$g.hist | to_entries | sort_by(.key | tonumber) | .[] | (.key | tonumber) as $khz
      | speed(.value) as $speed | {
        title: ($khz | mhz), value: "\($speed)%", progress: $speed,
        tone: (if $khz == $g.cap then "info" elif $speed < 100 or ($g.floor != null and $khz <= $g.floor) then "bad" else "plain" end)
      }]}),
    update("row:fatos"; {items: ([
      {title: (if $g.status == "learned" then "@fact.cap" else "@fact.testing" end), value: ($g.cap | mhz)},
      {title: "@fact.sessions", value: ($g.sessions | tostring)},
      (speed($g.hist[$g.cap | tostring]) | if . == null then empty else {title: "@fact.limit", value: "\(100 - .)%"} end),
      ($g | saving | if . == null then empty else {title: "@fact.saving", value: minus, tone: "good"} end),
      ($g | minutes | if . == null or . <= 0 then empty else {title: "@fact.battery", value: {key: "@fact.minutes", args: {n: .}}} end)
    ])})
  end;

# A game as the launcher names it, with what is already known about it.
def known($game):
  (.games[$gid] // {cap: top, floor: null, stable: 0, sessions: 0, hist: {},
    status: (if ($game.system | heavy) then "out" else "learning" end)})
  + {name: $game.name, system: $game.system, key: $game.game_key,
     sysname: ($game.system_name // $game.system), core: ($game.core // $game.emulator // "")};

# One measured session at ceiling $cap.
def learn($perf):
  ($perf.cpu_max_khz) as $cap
  | (([$perf.seconds, 1] | max)) as $seconds
  | ($perf.busy_p95 >= limit or $perf.limit_seconds >= 2) as $lost
  | ([100 - ($perf.limit_seconds * 100 / $seconds | floor), 0] | max) as $speed
  | .sessions += 1
  | .hist[$cap | tostring] |= ((. // {n: 0, speed: 0, ma: 0, man: 0})
      | .n += 1 | .speed += $speed
      | if $perf.current_ma != null then .ma += $perf.current_ma | .man += 1 else . end)
  | if out then .
    elif $lost and $cap >= top and .status != "learned" then .status = "out"
    elif $lost then .floor = $cap | .cap = (above($cap) // top) | (if .status == "learned" then . else .stable = 0 end)
    else (below($cap)) as $next
      | if .status != "learned" and $next != null and (.floor == null or $next > .floor) then .cap = $next | .stable = 0
        else .stable += 1 | if .stable >= stable_sessions then .status = "learned" else . end end
    end;

$st[0] as $s
| ($msg.params // {}) as $p
| if $msg.method == "initialize" then
    null, reply({ready: true}), ($s | page)
  elif $msg.method == "launch.before" then
    ($s | known($p.game)) as $g
    | (if ($opps | index($g.cap)) == null then $g + {cap: top} else $g end) as $g
    | ($s | .games[$gid] = $g | .pending = {id: $gid, key: $p.game.game_key}),
      reply(if ($g | out) or $g.cap >= top or top == 0 then {decision: "allow"}
            else {decision: "modify", changes: {cpu_max_khz: $g.cap}} end)
  elif $msg.method == "event" then
    $p.data as $d
    | if $s.pending != null and $s.pending.key == $d.game.game_key and $s.games[$s.pending.id] != null
         and ($d.perf.seconds // 0) >= least and ($opps | index($d.perf.cpu_max_khz)) != null
      then ($s | .games[$s.pending.id] |= learn($d.perf) | .pending = null) as $n
        | $n, ($n | page)
      else ($s | .pending = null), ($s | page) end
  elif $msg.method == "action.invoke" then
    if $p.action == "reaprender" and $s.games[$item] != null then
      ($s | .games[$item] |= (.cap = top | .floor = null | .stable = 0 | .sessions = 0 | .hist = {} | .pinned = false
        | .status = (if (.system | heavy) then "out" else "learning" end))) as $n
      | $n, reply({}), ($n | page, detail($item)),
        {jsonrpc: "2.0", method: "ui.notify", params: {key: "@notify.relearn", args: {name: $n.games[$item].name}}}
    elif $p.action == "maximo" and $s.games[$item] != null then
      ($s | .games[$item].pinned = (.games[$item].pinned | not)) as $n
      | $n, reply({}), ($n | page, detail($item)),
        {jsonrpc: "2.0", method: "ui.notify", params: {key: (if $n.games[$item].pinned then "@notify.max" else "@notify.auto" end),
          args: {name: $n.games[$item].name}}}
    elif $p.action == "detalhes" then
      null, reply({}), ($s | detail($item))
    else null, reply({}) end
  else null end
