#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
# Hermetic supervisor contract tests. SESSION_TEST_SHELL selects the child sh.
set -eu
session=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/archr-launcher-session
root=$(mktemp -d "${TMPDIR:-/tmp}/archr-session-test.XXXXXX")
test_runner=
cleanup() {
  if [ -n "$test_runner" ]; then
    kill -TERM "$test_runner" 2>/dev/null || :
    wait "$test_runner" 2>/dev/null || :
  fi
  for file in "$root"/*/game-pids; do
    [ -f "$file" ] || continue
    while IFS= read -r pid; do
      kill -KILL "$pid" 2>/dev/null || :
    done <"$file"
  done
  rm -rf "$root"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
mkdir -p "$root/bin"
export PATH="$root/bin:$PATH"
export ARCHR_SESSION_TEST=1
export ARCHR_SESSION_LAUNCHER=archr-launcher
export ARCHR_SESSION_SWAY_ATTEMPTS=0
export ARCHR_SESSION_SLEEP="$root/bin/sleep"
export ARCHR_SESSION_STOP_SLEEP=/bin/sleep
export ARCHR_SESSION_SAVE_EXIT="$root/bin/save-exit"
export ARCHR_SESSION_PROFILE="$root/profile"
export ARCHR_SESSION_SHELL=${SESSION_TEST_SHELL:-sh}
export ARCHR_TEST_EMULATOR="$root/bin/fake) emulator"
export ARCHR_SESSION_APPLY_LOCALE="$root/bin/apply-locale"
export ARCHR_SESSION_EXPORTS="$root/exports"
export ARCHR_SESSION_USER_CONTROLLER_DB="$root/user-db"
export ARCHR_SESSION_PACKAGED_CONTROLLER_DB="$root/packaged-db"
: >"$ARCHR_SESSION_PACKAGED_CONTROLLER_DB"
# The profile selects pt_BR; it is already generated unless a case says not.
export ARCHR_SESSION_LOCALE_DIR="$root/locale"
mkdir -p "$ARCHR_SESSION_LOCALE_DIR/pt_BR.UTF-8"
: >"$ARCHR_SESSION_LOCALE_DIR/pt_BR.UTF-8/LC_NAME"
# A manager environment imported by the ES-era archr-apply-locale.
export LOCPATH=/storage/.config/emulationstation/locale
cat >"$ARCHR_SESSION_EXPORTS" <<'EOF'
### If you add a variable, export it here.
export  MALI_WAYLAND_AFBC \
	SDL_VIDEODRIVER
EOF
cat >"$ARCHR_SESSION_PROFILE" <<'EOF'
# Deliberately Bash-only to exercise the profile bridge.
function get_setting() { printf '%s\n' pt_BR; }
export MALI_WAYLAND_AFBC=0 SDL_VIDEODRIVER=wayland
# A user database the profile names but nobody created.
export SDL_GAMECONTROLLERCONFIG_FILE=/nonexistent/gamecontrollerdb.txt
EOF

cat >"$root/bin/archr-launcher" <<'EOF'
#!/bin/sh
set -eu
[ "$LANG" = pt_BR.UTF-8 ]
[ -z "${LOCPATH+set}" ]
printf '%s\n' "$SDL_GAMECONTROLLERCONFIG_FILE" >"$CASE_DIR/controller-db"
[ -f "$CASE_DIR/locale-applied" ] || [ ! -e "$ARCHR_SESSION_APPLY_LOCALE" ] ||
  [ -f "$CASE_DIR/detached" ]
[ "$MALI_WAYLAND_AFBC" = 0 ]
[ "$SDL_VIDEODRIVER" = wayland ]
[ "$SDL_AUDIODRIVER" = pulseaudio ]
[ "$(cat "$ARCHR_SESSION_KILL_FILE")" = archr-launcher ]
if [ -f "$CASE_DIR/stale-scope-pid" ]; then
  [ -f "$CASE_DIR/scopes/archr-game-old.scope/stopped" ]
fi
if [ -f "$CASE_DIR/recovered-scope" ]; then
  [ -f "$CASE_DIR/scopes/$(cat "$CASE_DIR/recovered-scope")/stopped" ]
  [ "$(cat "$ARCHR_SESSION_CPUFREQ/policy0/scaling_max_freq")" = 1416000 ] ||
    : >"$CASE_DIR/cpu-stale-on-recovery"
fi
printf '%s\n' "$PPID" >"$CASE_DIR/supervisor-pid"
n=0
[ ! -f "$CASE_DIR/count" ] || n=$(cat "$CASE_DIR/count")
n=$((n + 1))
printf '%s\n' "$n" >"$CASE_DIR/count"
[ "$n" -eq 1 ] || [ ! -e "$ARCHR_SESSION_RUN_DIR/next" ]
if [ -f "$ARCHR_SESSION_RUN_DIR/last-exit" ]; then
  top=1416000
  [ "$(cat "$ARCHR_SESSION_CPUFREQ/boost")" != 1 ] || top=1512000
  [ "$(cat "$ARCHR_SESSION_CPUFREQ/policy0/scaling_max_freq")" = "$top" ]
  : >"$CASE_DIR/cpu-restored-before-launcher"
fi
if [ -f "$CASE_DIR/empty-group-end" ]; then
  date +%s%N >"$CASE_DIR/empty-group-return"
fi
line=$(sed -n "${n}p" "$CASE_DIR/plan")
case "$line" in
  END)
    : >"$CASE_DIR/finished"
    kill -TERM "$PPID"
    ;;
  HOLD)
    trap '' TERM
    : >"$CASE_DIR/launcher-ready"
    exec /bin/sleep 30
    ;;
  CRASH) exit 9 ;;
  WRITE_CRASH) printf '%s\n' 'action reboot' >"$ARCHR_SESSION_RUN_DIR/next"; exit 9 ;;
  CLEAN) exit 0 ;;
  LOUD) head -c 300000 /dev/zero; exit 0 ;;
  *) printf '%s\n' "$line" >"$ARCHR_SESSION_RUN_DIR/next" ;;
esac
EOF
cat >"$root/bin/runemu.sh" <<'EOF'
#!/bin/sh
set -eu
: >"$CASE_DIR/runemu-started"
printf '%s\n' "$ARCHR_TEST_SCOPE" >>"$CASE_DIR/runemu-runs"
# The gate opens only after the session saw the scope registered.
grep -q "^is-active --quiet $ARCHR_TEST_SCOPE active$" "$CASE_DIR/scope-calls" ||
  : >"$CASE_DIR/go-before-active"
# Nothing of the gate reaches the game, neither end.
! ls -l /proc/$$/fd 2>/dev/null | grep -q game-gate || : >"$CASE_DIR/gate-fd-leak"
# The next instruction must be consumed before starting any game.
[ ! -e "$ARCHR_SESSION_RUN_DIR/next" ]
# Do not target the wrapper: it must survive to restore services/governors.
[ ! -e "$ARCHR_SESSION_KILL_FILE" ]
printf '%s\n' "$$" >>"$CASE_DIR/scopes/$ARCHR_TEST_SCOPE/members"
# Same contract as the real runemu: save-exit reads its own $$.
printf '%s\n' demo.zip "$$" nes retroarch core >"$CASE_DIR/current-game"
# Like a per-game module, lower the ceiling at every game start, independently
# of any recovery the supervisor performed before returning to the launcher.
echo 816000 >"$ARCHR_SESSION_CPUFREQ/policy0/scaling_max_freq"
if [ "${1-}" = LONG ] || [ "${1-}" = ORPHAN ] ||
   [ "${1-}" = NORMAL ] || [ "${1-}" = COOPERATIVE ]; then
  # Like the real runemu.sh: NO TERM trap. Cleanup is normal control flow
  # after its emulator exits, including a nonzero/signaled emulator status.
  printf '%s\n' "$$" >"$CASE_DIR/game-pids"
  printf '%s\n' "$$" >"$CASE_DIR/wrapper-pid"
  # Like runemu's set_kill for a 32-bit core: the name is not EMULATOR's.
  printf '%s\n' retroarch32 >"$ARCHR_SESSION_KILL_FILE"
  if [ "$1" != LONG ]; then
    if [ "$1" != COOPERATIVE ]; then
      "$ARCHR_TEST_EMULATOR" STUBBORN &
    fi
    "$ARCHR_TEST_EMULATOR" TERM &
  fi
  ARCHR_TEST_NAME=retroarch32 "$ARCHR_TEST_EMULATOR" &
  child=$!
  printf '%s\n' "$child" >"$CASE_DIR/emulator-pid"
  wait "$child" || :
  /bin/sleep 0.15
  # Model save/quit work: group teardown must not interrupt this window.
  [ ! -f "$CASE_DIR/group-term" ] || : >"$CASE_DIR/premature-term"
  : >"$CASE_DIR/cleanup"
  exit 0
fi
if [ "${1-}" = EMPTY ]; then
  date +%s%N >"$CASE_DIR/empty-group-end"
fi
if [ "${1-}" = LOGCHECK ]; then
  head -c 300000 /dev/zero
  # Check while the game is still running, not only after it exits.
  [ "$(wc -c <"$ARCHR_SESSION_LOG")" -lt 262144 ]
fi
printf '%s\n' "$@" >"$CASE_DIR/args"
printf '%s\n' 'fake-emulator' >"$ARCHR_SESSION_KILL_FILE"
# Model runemu's quit(), which restores the old ES target.
printf '%s\n' emulationstation >"$ARCHR_SESSION_KILL_FILE"
exit 37
EOF
cat >"$ARCHR_TEST_EMULATOR" <<'EOF'
#!/bin/sh
case "${1-}" in
  STUBBORN) trap '' TERM ;;
  # A cooperative helper needs time to flush a save on TERM (not just exit).
  TERM) trap '/bin/sleep 0.3; : >"$CASE_DIR/group-term"; exit 0' TERM ;;
esac
/bin/sleep 30 &
child=$!
printf '%s\n' "$$" "$child" >>"$CASE_DIR/game-pids"
printf '%s\n' "$$" "$child" >>"$CASE_DIR/scopes/$ARCHR_TEST_SCOPE/members"
# What killall would match.
if [ -n "${ARCHR_TEST_NAME-}" ]; then
  mkdir -p "$CASE_DIR/names"
  printf '%s\n' "$$" >>"$CASE_DIR/names/$ARCHR_TEST_NAME"
fi
case "${1-}" in
  STUBBORN) : >"$CASE_DIR/stubborn-ready" ;;
  TERM) : >"$CASE_DIR/term-ready" ;;
  *) : >"$CASE_DIR/game-ready" ;;
esac
wait "$child"
EOF
cat >"$root/bin/systemctl" <<'EOF'
#!/bin/sh
# Barrier: a manager that never answers (the session must time it out).
case "$1" in
  is-active|stop) [ ! -f "$CASE_DIR/systemctl-hang-$1" ] || exec /bin/sleep 30 ;;
esac
alive_members() {
  [ -f "$CASE_DIR/scopes/$1/members" ] || return 0
  while IFS= read -r pid; do
    state=$(ps -o stat= -p "$pid" 2>/dev/null) || continue
    case "$state" in ''|Z*|X*) continue ;; esac
    printf '%s\n' "$pid"
  done <"$CASE_DIR/scopes/$1/members"
}
signal_scope() {
  for pid in $(alive_members "$2"); do kill "-$1" "$pid" 2>/dev/null || :; done
}
case "$1" in
  import-environment) printf '%s\n' "$*" >>"$CASE_DIR/imports" ;;
  list-units)
    printf '%s\n' "$*" >>"$CASE_DIR/scope-calls"
    for unit in "$CASE_DIR"/scopes/archr-game-*.scope; do
      [ -d "$unit" ] && [ ! -f "$unit/stopped" ] || continue
      printf '%s loaded active running\n' "${unit##*/}"
    done
    ;;
  is-active)
    unit=$3
    if [ -f "$CASE_DIR/scopes/$unit/stopped" ] || [ -z "$(alive_members "$unit")" ]; then
      printf '%s inactive\n' "$*" >>"$CASE_DIR/scope-calls"
      exit 3
    fi
    # Barrier: the scope exists, but the session has not seen it yet.
    if [ -f "$CASE_DIR/isactive-barrier" ]; then
      : >"$CASE_DIR/isactive-waiting"
      while [ -f "$CASE_DIR/isactive-barrier" ]; do /bin/sleep 0.01; done
    fi
    printf '%s active\n' "$*" >>"$CASE_DIR/scope-calls"
    ;;
  stop)
    printf '%s\n' "$*" >>"$CASE_DIR/scope-calls"
    unit=$2
    if [ -f "$CASE_DIR/save-exit-calls" ] && [ ! -f "$CASE_DIR/cleanup" ]; then
      : >"$CASE_DIR/residual-before-cleanup"
    fi
    if [ -f "$CASE_DIR/stop-barrier" ]; then
      : >"$CASE_DIR/stop-waiting"
      while [ -f "$CASE_DIR/stop-barrier" ]; do /bin/sleep 0.01; done
    fi
    [ -d "$CASE_DIR/scopes/$unit" ] || exit 5
    # Like systemd with TimeoutStopSec=1: TERM, up to 1 s, then KILL until
    # the cgroup is empty, including processes forked meanwhile.
    signal_scope TERM "$unit"
    deadline=$(($(date +%s%N) + 1000000000))
    while [ -n "$(alive_members "$unit")" ]; do
      if [ "$(date +%s%N)" -ge "$deadline" ]; then
        : >"$CASE_DIR/stop-grace-expired"
        break
      fi
      /bin/sleep 0.01
    done
    while [ -n "$(alive_members "$unit")" ]; do
      signal_scope KILL "$unit"
      /bin/sleep 0.01
    done
    : >"$CASE_DIR/scopes/$unit/stopped"
    ;;
  *) printf '%s\n' "$*" >>"$CASE_DIR/actions" ;;
esac
EOF
cat >"$root/bin/systemd-run" <<'EOF'
#!/bin/sh
if [ "$1" = --scope ]; then
  printf '%s %s\n' "$1" "$2" >>"$CASE_DIR/scope-runs"
  shift
  case "$1" in --unit=archr-game-*.scope) unit=${1#--unit=} ;; *) exit 2 ;; esac
  shift
  [ "$1 $2 $3 $4" = '--collect -p TimeoutStopSec=1 --' ] || exit 2
  shift 4
  export ARCHR_TEST_SCOPE=$unit
  # Barrier: systemd has not registered the scope yet.
  barrier=$CASE_DIR/register-barrier
  [ -f "$barrier" ] || barrier=$CASE_DIR/register-barrier-$unit
  if [ -f "$barrier" ]; then
    : >"$CASE_DIR/register-waiting"
    while [ -f "$barrier" ]; do /bin/sleep 0.01; done
    [ ! -f "$CASE_DIR/register-fail" ] || exit 1
  fi
  mkdir -p "$CASE_DIR/scopes/$unit"
  printf '%s\n' "$$" >"$CASE_DIR/scopes/$unit/members"
  # Real systemd-run --scope execs the command rather than a detached service.
  exec "$@"
fi
# Locale generation uses a detached transient service, not a game scope.
printf '%s\n' "$*" >"$CASE_DIR/detached"
for command; do :; done
"$command" >/dev/null 2>&1 </dev/null &
EOF
cat >"$ARCHR_SESSION_APPLY_LOCALE" <<'EOF'
#!/bin/sh
# Like a failed localedef: report, exit 1 and still let the UI start.
# Detached, it takes long enough that a waiting session would be caught.
[ ! -f "$CASE_DIR/detached" ] || /bin/sleep 2
: >"$CASE_DIR/locale-applied"
echo 'LOCPATH=/storage/.config/emulationstation/locale'
exit 1
EOF
cat >"$root/bin/save-exit" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CASE_DIR/save-exit-calls"
[ "$1" = suspend ] || exit 2
[ ! -f "$CASE_DIR/save-exit-fail" ] || exit 1
if [ -f "$CASE_DIR/save-exit-hang" ]; then
  trap '' TERM
  /bin/sleep 30
fi
{ IFS= read -r rom; IFS= read -r pid; IFS= read -r platform; IFS= read -r emulator; } \
  <"$CASE_DIR/current-game" || exit 2
[ "$pid" = "$(cat "$CASE_DIR/wrapper-pid")" ] || exit 2
# As the real helper: the kill file's names, else EMULATOR; then killall.
targets=
[ ! -r "$ARCHR_SESSION_KILL_FILE" ] || targets=$(grep -v '^-' "$ARCHR_SESSION_KILL_FILE")
[ -n "$targets" ] || targets=$emulator
printf '%s\n' "$targets" >"$CASE_DIR/save-exit-targets"
for name in $targets; do
  [ -f "$CASE_DIR/names/$name" ] || continue
  while IFS= read -r target; do kill -TERM "$target" 2>/dev/null || :; done \
    <"$CASE_DIR/names/$name"
done
while kill -0 "$pid" 2>/dev/null; do /bin/sleep 0.01; done
[ -f "$CASE_DIR/cleanup" ] || exit 1
: >"$CASE_DIR/save-exit-finished"
EOF
cat >"$root/bin/stop-sleep" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CASE_DIR/stop-sleeps"
exec /bin/sleep "$@"
EOF
cat >"$root/bin/swaymsg" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CASE_DIR/sway-calls"
if [ -f "$CASE_DIR/no-output" ]; then
  printf '%s\n' '[{"name": "DSI-1", "active": false}]'
else
  printf '%s\n' '[{"active": true}]'
fi
EOF
cat >"$root/bin/sleep" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CASE_DIR/sleeps"
EOF
chmod +x "$root/bin/"*

setup_case() {
  name=$1
  export CASE_DIR="$root/$name"
  mkdir -p "$CASE_DIR/scopes"
  export ARCHR_SESSION_RUN_DIR="$CASE_DIR/run"
  export ARCHR_SESSION_LOG="$CASE_DIR/session.log"
  export ARCHR_SESSION_KILL_FILE="$CASE_DIR/kill-data"
  # A CPU a module capped at 816 MHz for the game, with boost off.
  export ARCHR_SESSION_CPUFREQ="$CASE_DIR/cpufreq"
  mkdir -p "$ARCHR_SESSION_CPUFREQ/policy0"
  echo "${CASE_BOOST:-0}" >"$ARCHR_SESSION_CPUFREQ/boost"
  echo '408000 816000 1008000 1416000 ' >"$ARCHR_SESSION_CPUFREQ/policy0/scaling_available_frequencies"
  echo '1512000 ' >"$ARCHR_SESSION_CPUFREQ/policy0/scaling_boost_frequencies"
  echo 816000 >"$ARCHR_SESSION_CPUFREQ/policy0/scaling_max_freq"
  printf '%s\n' "$2" >"$CASE_DIR/plan"
}

run_case() {
  setup_case "$@"
  if [ "$name" = log-cap ]; then
    head -c 262140 /dev/zero >"$ARCHR_SESSION_LOG"
  fi
  if [ "$name" = no-output ]; then
    : >"$CASE_DIR/no-output"
  fi
  if [ "$name" = stale ]; then
    mkdir -p "$ARCHR_SESSION_RUN_DIR"
    printf '%s\n' 'action reboot' >"$ARCHR_SESSION_RUN_DIR/next"
  fi
  if [ "$name" = stale-scope ]; then
    /bin/sleep 30 &
    stale_pid=$!
    printf '%s\n' "$stale_pid" >"$CASE_DIR/stale-scope-pid"
    printf '%s\n' "$stale_pid" >"$CASE_DIR/game-pids"
    mkdir -p "$CASE_DIR/scopes/archr-game-old.scope"
    printf '%s\n' "$stale_pid" >"$CASE_DIR/scopes/archr-game-old.scope/members"
  fi
  # END terminates the supervisor; a timeout or an earlier failure is not success.
  status=0
  timeout 15 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 || status=$?
  if { [ "$status" -ne 143 ] && [ "$status" -ne 0 ]; } || [ ! -f "$CASE_DIR/finished" ]; then
    cat "$CASE_DIR/output" >&2
    [ ! -f "$ARCHR_SESSION_LOG" ] || cat "$ARCHR_SESSION_LOG" >&2
    echo "FAIL: $name (session status $status)" >&2
    exit 1
  fi
  [ "$(stat -c %a "$ARCHR_SESSION_RUN_DIR")" = 755 ]
  [ ! -e "$ARCHR_SESSION_RUN_DIR/next" ]
  [ ! -e "$ARCHR_SESSION_RUN_DIR/last-exit.tmp" ]
  [ ! -f "$CASE_DIR/go-before-active" ] ||
    { echo "FAIL: $name: game started before its scope was seen" >&2; exit 1; }
  [ ! -f "$CASE_DIR/gate-fd-leak" ] ||
    { echo "FAIL: $name: the game inherited the gate" >&2; exit 1; }
}

run_case game 'game runemu.sh /roms/demo.zip -Pnes --core=auto --emulator=retroarch --controllers=
END'
[ -f "$CASE_DIR/scope-runs" ] ||
  { echo 'FAIL: game did not launch in a systemd scope' >&2; exit 1; }
[ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = 37 ]
[ ! -f "$CASE_DIR/sleeps" ]
[ -f "$CASE_DIR/locale-applied" ]
[ "$(cat "$CASE_DIR/imports")" = 'import-environment MALI_WAYLAND_AFBC SDL_VIDEODRIVER' ]
[ "$(cat "$CASE_DIR/controller-db")" = "$ARCHR_SESSION_PACKAGED_CONTROLLER_DB" ]
# The fake runemu.sh never lifts the cap, like one that was killed.
[ "$(cat "$ARCHR_SESSION_CPUFREQ/policy0/scaling_max_freq")" = 1416000 ]
[ -f "$CASE_DIR/cpu-restored-before-launcher" ]
printf '%s\n' 'PASS game round-trip, kill-target reset and CPU ceiling before launcher'

# With boost on the boost OPP is the top; two games in a row, same result.
CASE_BOOST=1 run_case cpu-boost 'game runemu.sh demo
game runemu.sh demo
END'
[ "$(cat "$ARCHR_SESSION_CPUFREQ/policy0/scaling_max_freq")" = 1512000 ]
[ "$(grep -c 'Game exited' "$ARCHR_SESSION_LOG")" = 2 ]
[ "$(cut -d ' ' -f2 "$CASE_DIR/scope-runs" | sort -u | wc -l)" -eq 2 ]
printf '%s\n' 'PASS CPU ceiling restored after every game; scope names never reused'

run_case stale-scope 'CLEAN
END'
wait "$stale_pid" 2>/dev/null || :
! kill -0 "$stale_pid" 2>/dev/null
[ -f "$CASE_DIR/scopes/archr-game-old.scope/stopped" ]
grep -q '^stop archr-game-old.scope$' "$CASE_DIR/scope-calls"
printf '%s\n' 'PASS startup clears leftover scopes before the launcher starts'

: >"$ARCHR_SESSION_USER_CONTROLLER_DB"
run_case user-controller-db 'CLEAN
END'
[ "$(cat "$CASE_DIR/controller-db")" = "$ARCHR_SESSION_USER_CONTROLLER_DB" ]
rm -f "$ARCHR_SESSION_USER_CONTROLLER_DB"
printf '%s\n' 'PASS missing inherited controller database falls back'

# Literal ROM includes double/single quotes, spaces, backslash and an unexpanded $.
run_case quoting 'game runemu.sh '\''/roms/It'\''\'\'''\''s "Mega" $cash\ cart.zip'\'' '\''-Psnes'\'' '\''--core=auto'\'' '\''--emulator=retroarch'\'' '\''--controllers=pad one'\''
END'
printf '%s\n' '/roms/It'\''s "Mega" $cash\ cart.zip' '-Psnes' '--core=auto' '--emulator=retroarch' '--controllers=pad one' >"$CASE_DIR/expected"
cmp "$CASE_DIR/expected" "$CASE_DIR/args"
printf '%s\n' 'PASS exact shell quoting'

run_case unknown 'unknown touch must-not-run
action nonsense
game runemu.sh demo
END'
[ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = 37 ]
[ ! -f "$CASE_DIR/actions" ]
grep -q 'Unknown' "$ARCHR_SESSION_LOG"
printf '%s\n' 'PASS unknown instructions ignored'

cat >"$root/bin/error-report" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CASE_DIR/error-reports"
EOF
chmod +x "$root/bin/error-report"
export ARCHR_SESSION_ERROR_REPORT="$root/bin/error-report"
run_case crash 'CRASH
game runemu.sh demo
END'
unset ARCHR_SESSION_ERROR_REPORT
[ "$(cat "$CASE_DIR/sleeps")" = 1 ]
# Only the launcher's crash is reported, never the game's exit.
i=0
while [ ! -s "$CASE_DIR/error-reports" ] && [ "$i" -lt 100 ]; do /bin/sleep 0.01; i=$((i + 1)); done
[ "$(cat "$CASE_DIR/error-reports")" = 'launcher 9' ]
[ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = 37 ]
printf '%s\n' 'PASS crash backoff and recovery'

run_case stale 'CRASH
END'
[ ! -f "$CASE_DIR/actions" ]
[ "$(cat "$CASE_DIR/sleeps")" = 1 ]
printf '%s\n' 'PASS stale request discarded before launcher crash'

run_case no-output "$(printf 'CRASH\n%.0s' 1 2 3 4 5 6 7 8 9 10)
END"
grep -q 'No active output after 10 failures: "name":"DSI-1" "active":false $' "$ARCHR_SESSION_LOG"
[ "$(grep -c '^output \* enable; output \* power on$' "$CASE_DIR/sway-calls")" = 1 ]
printf '%s\n' 'PASS no active output: logged and asked to come up'

run_case failed-handoff 'WRITE_CRASH
END'
[ ! -f "$CASE_DIR/actions" ]
[ "$(cat "$CASE_DIR/sleeps")" = 1 ]
printf '%s\n' 'PASS failed launcher cannot publish an action'

run_case clean 'CLEAN
END'
[ ! -f "$CASE_DIR/actions" ]
[ ! -f "$CASE_DIR/sleeps" ]
printf '%s\n' 'PASS clean exit returns to launcher'

# Missing locale tool and quirk list: the session still starts.
ARCHR_SESSION_APPLY_LOCALE="$root/missing-apply-locale"
ARCHR_SESSION_EXPORTS="$root/missing-exports"
run_case no-locale-tool 'CLEAN
END'
[ ! -f "$CASE_DIR/locale-applied" ]
[ ! -f "$CASE_DIR/imports" ]
printf '%s\n' 'PASS locale tool and quirk list are optional'
ARCHR_SESSION_APPLY_LOCALE="$root/bin/apply-locale"
ARCHR_SESSION_EXPORTS="$root/exports"

# A locale still to be generated must not delay the launcher.
ARCHR_SESSION_LOCALE_DIR="$root/no-locales"
run_case locale-detached 'CLEAN
END'
grep -q -- '--unit=archr-apply-locale --nice=19 -p IOSchedulingClass=idle' "$CASE_DIR/detached"
[ ! -f "$CASE_DIR/locale-applied" ]
[ "$(cat "$CASE_DIR/imports")" = 'import-environment MALI_WAYLAND_AFBC SDL_VIDEODRIVER' ]
printf '%s\n' 'PASS locale generation runs detached; launcher starts at once'
ARCHR_SESSION_LOCALE_DIR="$root/locale"

# A missing compositor consumes exactly the bounded 100 polls, then continues.
ARCHR_SESSION_SWAY_ATTEMPTS=100
export ARCHR_SESSION_SWAY_ATTEMPTS
XDG_RUNTIME_DIR="$root/no-compositor"
export XDG_RUNTIME_DIR
run_case sway-timeout 'CLEAN
END'
[ "$(wc -l <"$CASE_DIR/sleeps" | tr -d ' ')" = 100 ]
[ "$(sort -u "$CASE_DIR/sleeps")" = 0.1 ]
[ ! -f "$CASE_DIR/actions" ]
printf '%s\n' 'PASS bounded sway wait'

ARCHR_SESSION_SWAY_ATTEMPTS=0
export ARCHR_SESSION_SWAY_ATTEMPTS
run_case log-cap 'LOUD
game runemu.sh LOGCHECK
CLEAN
END'
[ "$(wc -c <"$ARCHR_SESSION_LOG")" -lt 262144 ]
[ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = 37 ]
[ ! -f "$CASE_DIR/actions" ]
printf '%s\n' 'PASS session log remains bounded during child output'

# Socket file types are real; only swaymsg's external protocol is faked.
# Short relative bind paths also work when TMPDIR is longer than sun_path.
if command -v python3 >/dev/null 2>&1; then
  XDG_RUNTIME_DIR="$root/compositor"
  export XDG_RUNTIME_DIR
  mkdir -p "$XDG_RUNTIME_DIR"
  python3 -I -c 'import os, socket, sys
os.chdir(sys.argv[1])
for name in ("wayland-1", "sway-ipc.0.sock"):
    s = socket.socket(socket.AF_UNIX)
    s.bind(name)
    s.close()' "$XDG_RUNTIME_DIR"
  WAYLAND_DISPLAY=wayland-1
  ARCHR_SESSION_SWAY_ATTEMPTS=100
  export WAYLAND_DISPLAY ARCHR_SESSION_SWAY_ATTEMPTS
  run_case sway-ready 'CLEAN
END'
  [ ! -f "$CASE_DIR/sleeps" ]
  [ "$(cat "$CASE_DIR/sway-calls")" = "-s $XDG_RUNTIME_DIR/sway-ipc.0.sock -t get_outputs" ]
  printf '%s\n' 'PASS ready compositor starts without delay'
fi

# Exercise both exec-optimized sh -c and an outer shell that must stay alive.
stop_case() {
  export ARCHR_SESSION_SWAY_ATTEMPTS=0
  setup_case "$1" "$2
END"
  case "$1" in
    service-stop-failed) : >"$CASE_DIR/save-exit-fail" ;;
    service-stop-hung) : >"$CASE_DIR/save-exit-hang" ;;
  esac
  timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 &
  test_runner=$!
  i=0
  while [ ! -f "$CASE_DIR/$3" ] && [ "$i" -lt 200 ]; do
    /bin/sleep 0.01
    i=$((i + 1))
  done
  [ -f "$CASE_DIR/$3" ]
  kill -TERM "$(cat "$CASE_DIR/supervisor-pid")"
  status=0
  wait "$test_runner" 2>/dev/null || status=$?
  test_runner=
  [ "$status" -eq 0 ]
}

run_stop_case() {
  stop_case "$1" "$2" game-ready
  [ -f "$CASE_DIR/cleanup" ] || {
    cat "$CASE_DIR/output" "$ARCHR_SESSION_LOG" >&2
    echo 'FAIL: service stop skipped game cleanup' >&2
    exit 1
  }
  # The game was capped and stopped with the service: the ceiling is back.
  [ "$(cat "$ARCHR_SESSION_CPUFREQ/policy0/scaling_max_freq")" = 1416000 ] ||
    { echo 'FAIL: service stop left the CPU ceiling capped' >&2; exit 1; }
  while IFS= read -r pid; do
    if kill -0 "$pid" 2>/dev/null; then
      echo "FAIL: fake game process $pid survived service stop" >&2
      exit 1
    fi
  done <"$CASE_DIR/game-pids"
  [ "$(cat "$CASE_DIR/save-exit-calls")" = suspend ]
  # The kill file still named retroarch32; EMULATOR=retroarch would miss it.
  [ "$(cat "$CASE_DIR/save-exit-targets")" = retroarch32 ] ||
    { echo 'FAIL: save-exit ran without the kill file' >&2; exit 1; }
  [ -f "$CASE_DIR/save-exit-finished" ]
  [ ! -f "$CASE_DIR/residual-before-cleanup" ]
  [ ! -e "$ARCHR_SESSION_KILL_FILE" ]
  grep -q '^stop archr-game-' "$CASE_DIR/scope-calls"
  printf '%s\n' "PASS $1 preserves untrapped wrapper cleanup and leaves no processes"
}
run_stop_case service-stop 'game runemu.sh LONG'
run_stop_case service-stop-shell 'game runemu.sh LONG; :'

for mode in failed missing hung; do
  export ARCHR_SESSION_SAVE_EXIT="$root/bin/save-exit"
  [ "$mode" != missing ] || ARCHR_SESSION_SAVE_EXIT="$root/missing-save-exit"
  started=$(date +%s%N)
  stop_case "service-stop-$mode" 'game runemu.sh LONG' game-ready
  ended=$(date +%s%N)
  awk -v started="$started" -v ended="$ended" 'BEGIN { exit !((ended-started)/1000000 < 6000) }'
  grep -q '^stop archr-game-' "$CASE_DIR/scope-calls"
  [ "$(cat "$ARCHR_SESSION_CPUFREQ/policy0/scaling_max_freq")" = 1416000 ]
  if [ "$mode" = missing ]; then
    [ ! -f "$CASE_DIR/save-exit-calls" ]
  else
    [ "$(cat "$CASE_DIR/save-exit-calls")" = suspend ]
  fi
  while IFS= read -r pid; do
    i=0
    while kill -0 "$pid" 2>/dev/null && [ "$i" -lt 100 ]; do
      /bin/sleep 0.01
      i=$((i+1))
    done
    ! kill -0 "$pid" 2>/dev/null
  done <"$CASE_DIR/game-pids"
  printf '%s\n' "PASS save-exit $mode: bounded scope fallback and CPU restoration"
done
export ARCHR_SESSION_SAVE_EXIT="$root/bin/save-exit"

# The wrapper may die before its emulator. Tracking only wait's PID must fail
# these checks; a TERM-only implementation must also fail (STUBBORN ignores it).
exit_group_case() {
  export ARCHR_SESSION_STOP_SLEEP="$root/bin/stop-sleep"
  setup_case "$1" "game runemu.sh $2$3
END"
  timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 &
  test_runner=$!
  i=0
  while { [ ! -f "$CASE_DIR/game-ready" ] ||
          { [ "$2" != COOPERATIVE ] && [ ! -f "$CASE_DIR/stubborn-ready" ]; } ||
          [ ! -f "$CASE_DIR/term-ready" ]; } && [ "$i" -lt 200 ]; do
    /bin/sleep 0.01
    i=$((i + 1))
  done
  [ -f "$CASE_DIR/game-ready" ] && [ -f "$CASE_DIR/term-ready" ]
  [ "$2" = COOPERATIVE ] || [ -f "$CASE_DIR/stubborn-ready" ]
  wrapper=$(cat "$CASE_DIR/wrapper-pid")
  [ "$(sed -n '2p' "$CASE_DIR/current-game")" = "$wrapper" ]
  [ -f "$CASE_DIR/scope-runs" ]
  [ ! -f "$CASE_DIR/group-term" ]
  if [ "$2" != NORMAL ]; then
    kill -KILL "$wrapper"
  else
    # Normal emulator exit (Select+Start/save-and-quit): runemu must finish its
    # cleanup before the remaining helpers receive any group signal.
    kill -TERM "$(cat "$CASE_DIR/emulator-pid")"
  fi
  status=0
  wait "$test_runner" 2>/dev/null || status=$?
  test_runner=
  [ "$status" -eq 0 ] && [ -f "$CASE_DIR/finished" ]
  [ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = "$4" ]
  [ "$(cat "$ARCHR_SESSION_CPUFREQ/policy0/scaling_max_freq")" = 1416000 ]
  [ -f "$CASE_DIR/cpu-restored-before-launcher" ]
  grep -q '^stop archr-game-' "$CASE_DIR/scope-calls"
  [ ! -f "$CASE_DIR/save-exit-calls" ]
  if [ "$2" = COOPERATIVE ]; then
    [ ! -f "$CASE_DIR/stop-grace-expired" ] ||
      { echo 'FAIL: cooperative group waited the full deadline' >&2; exit 1; }
  fi
  if [ "$2" = NORMAL ]; then
    [ -f "$CASE_DIR/cleanup" ] && [ ! -f "$CASE_DIR/premature-term" ] ||
      { echo 'FAIL: group termination interrupted normal save/cleanup' >&2; exit 1; }
  fi
  # Every recorded cgroup member must be gone (allow init to reap orphans).
  while IFS= read -r pid; do
    i=0
    while kill -0 "$pid" 2>/dev/null && [ "$i" -lt 100 ]; do
      /bin/sleep 0.01
      i=$((i + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
      echo "FAIL: fake cgroup member $pid survived $1" >&2
      exit 1
    fi
  done <"$CASE_DIR/game-pids"
  [ -f "$CASE_DIR/group-term" ] ||
    { echo 'FAIL: group teardown skipped TERM grace period' >&2; exit 1; }
  printf '%s\n' "PASS $1: cgroup TERM/KILL leaves no emulator or helper"
}
exit_group_case wrapper-killed ORPHAN '' 137
exit_group_case wrapper-killed-shell ORPHAN '; :' 0
exit_group_case normal-save NORMAL '' 0
exit_group_case cooperative-exit COOPERATIVE '' 137

# An empty group must request no grace-period sleeps, including after a normal
# nonzero game exit. Also measure the handoff rather than session shutdown.
run_case empty-group 'game runemu.sh EMPTY
END'
awk -v ended="$(cat "$CASE_DIR/empty-group-end")" \
    -v returned="$(cat "$CASE_DIR/empty-group-return")" 'BEGIN {
  elapsed = (returned - ended) / 1000000
  printf "PASS empty game group: zero sleeps, %.1f ms to launcher\n", elapsed
  exit !(elapsed < 1000)
}'

grep -q '^stop archr-game-' "$CASE_DIR/scope-calls"
export ARCHR_SESSION_STOP_SLEEP=/bin/sleep

# A SIGKILL cannot run traps. Keep the fake cgroup registry across a fresh
# supervisor invocation, as systemd does, and require recovery before any UI.
setup_case session-restart 'game runemu.sh LONG'
timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/first-output" 2>&1 &
test_runner=$!
i=0
while [ ! -f "$CASE_DIR/game-ready" ] && [ "$i" -lt 200 ]; do
  /bin/sleep 0.01
  i=$((i+1))
done
[ -f "$CASE_DIR/game-ready" ]
kill -KILL "$(cat "$CASE_DIR/supervisor-pid")"
status=0
wait "$test_runner" 2>/dev/null || status=$?
test_runner=
[ "$status" -eq 137 ]
kill -0 "$(cat "$CASE_DIR/wrapper-pid")"
old_unit=$(cut -d ' ' -f2 "$CASE_DIR/scope-runs")
printf '%s\n' "${old_unit#--unit=}" >"$CASE_DIR/recovered-scope"
rm -f "$CASE_DIR/count"
run_case session-restart 'game runemu.sh demo
END'
[ "$(cut -d ' ' -f2 "$CASE_DIR/scope-runs" | sort -u | wc -l)" -eq 2 ]
[ ! -f "$CASE_DIR/cpu-stale-on-recovery" ] ||
  { echo 'FAIL: recovered scope left the CPU capped before the launcher' >&2; exit 1; }
while IFS= read -r pid; do
  i=0
  while kill -0 "$pid" 2>/dev/null && [ "$i" -lt 100 ]; do
    /bin/sleep 0.01
    i=$((i+1))
  done
  ! kill -0 "$pid" 2>/dev/null
done <"$CASE_DIR/game-pids"
printf '%s\n' 'PASS supervisor SIGKILL: next session clears the scope and never reuses its name'

# Wait until the game's processes are gone (init reaps the orphans).
all_gone() {
  for list in "$CASE_DIR"/scopes/*/members; do
    [ -f "$list" ] || continue
    while IFS= read -r pid; do
      i=0
      while kill -0 "$pid" 2>/dev/null && [ "$i" -lt 100 ]; do
        /bin/sleep 0.01
        i=$((i + 1))
      done
      if kill -0 "$pid" 2>/dev/null; then
        echo "FAIL: process $pid survived $name" >&2
        exit 1
      fi
    done <"$list"
  done
}

wait_for() {
  i=0
  while [ ! -e "$1" ] && [ "$i" -lt 300 ]; do
    /bin/sleep 0.01
    i=$((i + 1))
  done
  [ -e "$1" ] || { echo "FAIL: $name: $1 never appeared" >&2; exit 1; }
}

# The gate process must leave on its own, without the game ever running.
gate_closed() {
  waiter=$(sed -n 1p "$CASE_DIR/scopes/$(ls "$CASE_DIR/scopes")/members")
  i=0
  while kill -0 "$waiter" 2>/dev/null && [ "$i" -lt 200 ]; do
    /bin/sleep 0.01
    i=$((i + 1))
  done
  ! kill -0 "$waiter" 2>/dev/null ||
    { echo "FAIL: $name: gate still waiting" >&2; kill -KILL "$waiter"; exit 1; }
  [ ! -f "$CASE_DIR/runemu-started" ] ||
    { echo "FAIL: $name: the game ran behind a closed gate" >&2; exit 1; }
}

export ARCHR_SESSION_STOP_SLEEP=/bin/sleep
# Registration far later than any deadline, after a TERM: the session leaves,
# then systemd registers the scope and runs the gate, which gets EOF.
setup_case late-register 'game runemu.sh LONG
END'
name=late-register
: >"$CASE_DIR/register-barrier"
timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 &
test_runner=$!
wait_for "$CASE_DIR/register-waiting"
kill -TERM "$(cat "$CASE_DIR/supervisor-pid")"
status=0
wait "$test_runner" 2>/dev/null || status=$?
test_runner=
[ "$status" -eq 0 ]
/bin/sleep 1
rm -f "$CASE_DIR/register-barrier"
wait_for "$CASE_DIR/scopes/archr-game-1.scope/members"
gate_closed
[ ! -f "$CASE_DIR/save-exit-calls" ]
printf '%s\n' 'PASS registration after the session left: the gate never opens'

# Late registration with no stop: the game starts once the scope is seen.
setup_case late-register-run 'game runemu.sh demo
END'
name=late-register-run
: >"$CASE_DIR/register-barrier"
timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 &
test_runner=$!
wait_for "$CASE_DIR/register-waiting"
/bin/sleep 0.5
rm -f "$CASE_DIR/register-barrier"
status=0
wait "$test_runner" 2>/dev/null || status=$?
test_runner=
[ "$status" -eq 0 ] && [ -f "$CASE_DIR/finished" ]
[ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = 37 ]
[ ! -f "$CASE_DIR/go-before-active" ] ||
  { echo 'FAIL: the gate opened before the scope was seen' >&2; exit 1; }
[ ! -f "$CASE_DIR/gate-fd-leak" ]
printf '%s\n' 'PASS late registration: go only after is-active, the game holds no gate'

# A gate of an earlier session registers late, while the next session holds
# the "go" of its own game ready: it must never take that "go" nor run.
setup_case gate-generations 'game runemu.sh LONG'
name=gate-generations
: >"$CASE_DIR/register-barrier-archr-game-1.scope"
timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/first-output" 2>&1 &
test_runner=$!
wait_for "$CASE_DIR/register-waiting"
kill -KILL "$(cat "$CASE_DIR/supervisor-pid")"
wait "$test_runner" 2>/dev/null || :
rm -f "$CASE_DIR/count"
printf '%s\n' 'game runemu.sh demo' END >"$CASE_DIR/plan"
: >"$CASE_DIR/isactive-barrier"
timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 &
test_runner=$!
wait_for "$CASE_DIR/isactive-waiting"
# The new gate is open and waiting; now the old launch registers.
rm -f "$CASE_DIR/register-barrier-archr-game-1.scope"
wait_for "$CASE_DIR/scopes/archr-game-1.scope/members"
old_waiter=$(sed -n 1p "$CASE_DIR/scopes/archr-game-1.scope/members")
i=0
while kill -0 "$old_waiter" 2>/dev/null && [ "$i" -lt 200 ]; do
  /bin/sleep 0.01
  i=$((i + 1))
done
! kill -0 "$old_waiter" 2>/dev/null ||
  { echo 'FAIL: the old gate is waiting on a FIFO' >&2; kill -KILL "$old_waiter"; exit 1; }
rm -f "$CASE_DIR/isactive-barrier"
status=0
wait "$test_runner" 2>/dev/null || status=$?
test_runner=
[ "$status" -eq 0 ] && [ -f "$CASE_DIR/finished" ]
[ "$(cat "$CASE_DIR/runemu-runs")" = archr-game-2.scope ] ||
  { echo "FAIL: games run: $(tr '\n' ' ' <"$CASE_DIR/runemu-runs")" >&2; exit 1; }
[ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = 37 ]
! ls "$ARCHR_SESSION_RUN_DIR" | grep -q game-gate ||
  { echo 'FAIL: a gate FIFO outlived its launch' >&2; exit 1; }
printf '%s\n' 'PASS late gate of an earlier session: no FIFO, no go taken, no game'

# A registration that never comes: the session gives up, does not wait on
# systemd-run, and is back at the launcher in bounded time.
printf '#!/bin/sh\nexec /bin/sleep 0.01\n' >"$root/bin/fast-sleep"
chmod +x "$root/bin/fast-sleep"
export ARCHR_SESSION_STOP_SLEEP="$root/bin/fast-sleep"
setup_case never-registered 'game runemu.sh LONG
END'
name=never-registered
: >"$CASE_DIR/register-barrier"
started=$(date +%s%N)
status=0
timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 || status=$?
ended=$(date +%s%N)
[ "$status" -eq 0 ] || [ "$status" -eq 143 ]
[ -f "$CASE_DIR/finished" ] ||
  { echo 'FAIL: the session never came back to the launcher' >&2; exit 1; }
[ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = 125 ]
grep -q 'not registered' "$ARCHR_SESSION_LOG"
awk -v started="$started" -v ended="$ended" 'BEGIN {
  elapsed = (ended - started) / 1000000
  printf "PASS registration never comes: back at the launcher in %.0f ms\n", elapsed
  exit !(elapsed < 5000)
}'
# Let the stuck client finish: its gate is gone, so no game runs.
rm -f "$CASE_DIR/register-barrier"
wait_for "$CASE_DIR/scopes/archr-game-1.scope/members"
gate_closed
export ARCHR_SESSION_STOP_SLEEP=/bin/sleep

# SIGKILL with the gate process already reading: EOF ends it. It holds the
# gate only read-only (read's stdin is a copy): it cannot keep it open.
setup_case gate-eof 'game runemu.sh LONG
END'
name=gate-eof
: >"$CASE_DIR/isactive-barrier"
timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 &
test_runner=$!
wait_for "$CASE_DIR/isactive-waiting"
waiter=$(sed -n 1p "$CASE_DIR/scopes/$(ls "$CASE_DIR/scopes")/members")
i=0
while [ "$(ls -l /proc/"$waiter"/fd 2>/dev/null | grep -c game-gate)" -eq 0 ] &&
      [ "$i" -lt 200 ]; do
  /bin/sleep 0.01
  i=$((i + 1))
done
gate_fds=0
for fd in /proc/"$waiter"/fd/*; do
  case "$(readlink "$fd")" in
    */game-gate-*)
      gate_fds=$((gate_fds + 1))
      flags=$(awk '/^flags:/ { print $2 }' "/proc/$waiter/fdinfo/${fd##*/}")
      [ $((0$flags & 3)) -eq 0 ] ||
        { echo "FAIL: gate process can write to its gate ($flags)" >&2; exit 1; }
      ;;
  esac
done
[ "$gate_fds" -ge 1 ] || { echo 'FAIL: gate process never opened its gate' >&2; exit 1; }
kill -KILL "$(cat "$CASE_DIR/supervisor-pid")"
status=0
wait "$test_runner" 2>/dev/null || status=$?
test_runner=
[ "$status" -eq 137 ]
rm -f "$CASE_DIR/isactive-barrier"
gate_closed
printf '%s\n' 'PASS session SIGKILL before go: gate EOF, read-only, the game never runs'

# systemd-run fails without a scope: nothing to save, nothing left over.
setup_case register-fail 'game runemu.sh LONG
END'
name=register-fail
: >"$CASE_DIR/register-barrier"
: >"$CASE_DIR/register-fail"
timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 &
test_runner=$!
i=0
while [ ! -f "$CASE_DIR/register-waiting" ] && [ "$i" -lt 200 ]; do
  /bin/sleep 0.01
  i=$((i + 1))
done
kill -TERM "$(cat "$CASE_DIR/supervisor-pid")"
/bin/sleep 0.1
rm -f "$CASE_DIR/register-barrier"
status=0
wait "$test_runner" 2>/dev/null || status=$?
test_runner=
[ "$status" -eq 0 ]
[ ! -f "$CASE_DIR/save-exit-calls" ]
printf '%s\n' 'PASS failed scope registration: stop needs no save and no cleanup'

# A TERM while the scope of a finished game is being stopped: no save-exit.
setup_case term-in-cleanup 'game runemu.sh NORMAL
END'
name=term-in-cleanup
: >"$CASE_DIR/stop-barrier"
timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 &
test_runner=$!
i=0
while { [ ! -f "$CASE_DIR/game-ready" ] || [ ! -f "$CASE_DIR/term-ready" ] ||
        [ ! -f "$CASE_DIR/stubborn-ready" ]; } && [ "$i" -lt 200 ]; do
  /bin/sleep 0.01
  i=$((i + 1))
done
kill -TERM "$(cat "$CASE_DIR/emulator-pid")"
i=0
while [ ! -f "$CASE_DIR/stop-waiting" ] && [ "$i" -lt 200 ]; do
  /bin/sleep 0.01
  i=$((i + 1))
done
[ -f "$CASE_DIR/stop-waiting" ]
kill -TERM "$(cat "$CASE_DIR/supervisor-pid")"
/bin/sleep 0.2
rm -f "$CASE_DIR/stop-barrier"
status=0
wait "$test_runner" 2>/dev/null || status=$?
test_runner=
[ "$status" -eq 0 ]
[ ! -f "$CASE_DIR/save-exit-calls" ] ||
  { echo 'FAIL: TERM during cleanup asked a finished game to save' >&2; exit 1; }
[ "$(grep -c '^stop archr-game-' "$CASE_DIR/scope-calls")" -eq 1 ]
[ "$(cat "$ARCHR_SESSION_CPUFREQ/policy0/scaling_max_freq")" = 1416000 ]
all_gone
printf '%s\n' 'PASS TERM during normal game cleanup: no second save, one scope stop'

# A manager that never answers, with save-exit hung too: still under 8 s.
for hang in stop is-active; do
  setup_case "systemctl-hang-$hang" 'game runemu.sh LONG
END'
  name=systemctl-hang-$hang
  : >"$CASE_DIR/save-exit-hang"
  timeout 14 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 &
  test_runner=$!
  i=0
  while [ ! -f "$CASE_DIR/game-ready" ] && [ "$i" -lt 200 ]; do
    /bin/sleep 0.01
    i=$((i + 1))
  done
  [ -f "$CASE_DIR/game-ready" ]
  : >"$CASE_DIR/systemctl-hang-$hang"
  started=$(date +%s%N)
  kill -TERM "$(cat "$CASE_DIR/supervisor-pid")"
  status=0
  wait "$test_runner" 2>/dev/null || status=$?
  test_runner=
  ended=$(date +%s%N)
  [ "$status" -eq 0 ]
  awk -v started="$started" -v ended="$ended" -v hang="$hang" 'BEGIN {
    elapsed = (ended - started) / 1000000
    printf "PASS systemctl %s hung (save-exit hung too): stop took %.0f ms\n", hang, elapsed
    exit !(elapsed < 8000)
  }'
  # A dead manager stops nothing; the next session would. Clean up here.
  rm -f "$CASE_DIR/systemctl-hang-$hang"
  for unit in "$CASE_DIR"/scopes/*; do systemctl stop "${unit##*/}" || :; done
done

run_case removed-actions 'action suspend
action shutdown
action reboot
action settings
END'
[ ! -f "$CASE_DIR/actions" ]
printf '%s\n' 'PASS removed actions cannot execute in supervisor'
export ARCHR_SESSION_STOP_SLEEP="$root/bin/stop-sleep"
stop_case launcher-stop HOLD launcher-ready
[ "$(cat "$CASE_DIR/stop-sleeps")" = 0.2 ]
! grep -q 'game cleanup' "$ARCHR_SESSION_LOG"
[ ! -e "$ARCHR_SESSION_RUN_DIR/next" ]
[ "$(cat "$CASE_DIR/count")" = 1 ]
printf '%s\n' 'PASS launcher-only stop has no game cleanup window'
echo 'All session tests passed.'
