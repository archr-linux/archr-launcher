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
# The next instruction must be consumed before starting any game.
[ ! -e "$ARCHR_SESSION_RUN_DIR/next" ]
# Do not target the wrapper: it must survive to restore services/governors.
[ ! -e "$ARCHR_SESSION_KILL_FILE" ]
guardian=$(ps -o pgid= -p "$$" | tr -d ' ')
printf '%s\n' "$guardian" >"$CASE_DIR/guardian-pid"
stat -c %i "/proc/$guardian" >"$CASE_DIR/guardian-inode"
# Same contract as the real runemu: save-exit reads its $$, not the guardian.
printf '%s\n' demo.zip "$$" nes retroarch core >"$CASE_DIR/current-game"
if [ "${1-}" = LONG ] || [ "${1-}" = ORPHAN ] ||
   [ "${1-}" = NORMAL ] || [ "${1-}" = COOPERATIVE ]; then
  # Like the real runemu.sh: NO TERM trap. Cleanup is normal control flow
  # after its emulator exits, including a nonzero/signaled emulator status.
  printf '%s\n' "$$" >"$CASE_DIR/game-pids"
  printf '%s\n' "$$" >"$CASE_DIR/wrapper-pid"
  if [ "$1" != LONG ]; then
    if [ "$1" != COOPERATIVE ]; then
      "$ARCHR_TEST_EMULATOR" STUBBORN &
    fi
    "$ARCHR_TEST_EMULATOR" TERM &
  fi
  "$ARCHR_TEST_EMULATOR" &
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
# This process name contains ')' and a space to exercise /proc/stat parsing.
case "${1-}" in
  STUBBORN) trap '' TERM ;;
  # A cooperative helper needs time to flush a save on TERM (not just exit).
  TERM) trap '/bin/sleep 0.3; : >"$CASE_DIR/group-term"; exit 0' TERM ;;
esac
/bin/sleep 30 &
child=$!
printf '%s\n' "$$" "$child" >>"$CASE_DIR/game-pids"
case "${1-}" in
  STUBBORN) : >"$CASE_DIR/stubborn-ready" ;;
  TERM) : >"$CASE_DIR/term-ready" ;;
  *) : >"$CASE_DIR/game-ready" ;;
esac
wait "$child"
EOF
cat >"$root/bin/systemctl" <<'EOF'
#!/bin/sh
case "$1" in
  import-environment) printf '%s\n' "$*" >>"$CASE_DIR/imports" ;;
  *) printf '%s\n' "$*" >>"$CASE_DIR/actions" ;;
esac
EOF
cat >"$root/bin/systemd-run" <<'EOF'
#!/bin/sh
# Like the real one: return at once and run the unit command elsewhere.
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
cat >"$root/bin/stop-sleep" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CASE_DIR/stop-sleeps"
check_guardian() {
  [ -f "$CASE_DIR/guardian-pid" ] || return 0
  guardian=$(cat "$CASE_DIR/guardian-pid")
  # /proc inode identity must stay pinned for the WHOLE grace interval, not
  # just at a snapshot before TERM. The same live leader reserves PGID/SID.
  if ! kill -0 "$guardian" 2>/dev/null ||
     [ "$(stat -c %i "/proc/$guardian" 2>/dev/null)" != "$(cat "$CASE_DIR/guardian-inode")" ]; then
    : >"$CASE_DIR/guardian-lost"
    return 1
  fi
  : >"$CASE_DIR/guardian-reserved"
}
check_guardian || exit 1
/bin/sleep "$@"
check_guardian
EOF
cat >"$root/bin/swaymsg" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CASE_DIR/sway-calls"
printf '%s\n' '[{"active": true}]'
EOF
cat >"$root/bin/sleep" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CASE_DIR/sleeps"
EOF
chmod +x "$root/bin/"*

setup_case() {
  name=$1
  export CASE_DIR="$root/$name"
  mkdir -p "$CASE_DIR"
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
  if [ "$name" = stale ]; then
    mkdir -p "$ARCHR_SESSION_RUN_DIR"
    printf '%s\n' 'action reboot' >"$ARCHR_SESSION_RUN_DIR/next"
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
  [ ! -e "$ARCHR_SESSION_RUN_DIR/game-exit" ]
  [ ! -e "$ARCHR_SESSION_RUN_DIR/game-release" ]
}

run_case game 'game runemu.sh /roms/demo.zip -Pnes --core=auto --emulator=retroarch --controllers=
END'
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
printf '%s\n' 'PASS the CPU ceiling comes back after every game'

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

run_case crash 'CRASH
game runemu.sh demo
END'
[ "$(cat "$CASE_DIR/sleeps")" = 1 ]
[ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = 37 ]
printf '%s\n' 'PASS crash backoff and recovery'

run_case stale 'CRASH
END'
[ ! -f "$CASE_DIR/actions" ]
[ "$(cat "$CASE_DIR/sleeps")" = 1 ]
printf '%s\n' 'PASS stale request discarded before launcher crash'

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
  wait "$test_runner" || status=$?
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
  if kill -0 "$(cat "$CASE_DIR/guardian-pid")" 2>/dev/null; then
    echo 'FAIL: service stop did not reap the guardian' >&2
    exit 1
  fi
  [ ! -e "$ARCHR_SESSION_RUN_DIR/game-exit" ]
  [ ! -e "$ARCHR_SESSION_RUN_DIR/game-release" ]
  if grep -q 'cleanup timed out' "$ARCHR_SESSION_LOG"; then
    echo 'FAIL: normal wrapper cleanup needed forced group termination' >&2
    exit 1
  fi
  printf '%s\n' "PASS $1 preserves untrapped wrapper cleanup and leaves no processes"
}
run_stop_case service-stop 'game runemu.sh LONG'
run_stop_case service-stop-shell 'game runemu.sh LONG; :'

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
  group=$(ps -o pgid= -p "$wrapper" | tr -d ' ')
  [ -n "$group" ]
  [ "$wrapper" != "$group" ] ||
    { echo 'FAIL: runemu is still the group leader; no guardian reserves PGID' >&2; exit 1; }
  [ "$(sed -n '2p' "$CASE_DIR/current-game")" = "$wrapper" ]
  [ ! -f "$CASE_DIR/group-term" ]
  if [ "$2" != NORMAL ]; then
    kill -KILL "$wrapper"
  else
    # Normal emulator exit (Select+Start/save-and-quit): runemu must finish its
    # cleanup before the remaining helpers receive any group signal.
    kill -TERM "$(cat "$CASE_DIR/emulator-pid")"
  fi
  status=0
  wait "$test_runner" || status=$?
  test_runner=
  [ "$status" -eq 0 ] && [ -f "$CASE_DIR/finished" ]
  [ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = "$4" ]
  [ "$(cat "$ARCHR_SESSION_CPUFREQ/policy0/scaling_max_freq")" = 1416000 ]
  [ -f "$CASE_DIR/cpu-restored-before-launcher" ]
  [ -f "$CASE_DIR/guardian-reserved" ] && [ ! -f "$CASE_DIR/guardian-lost" ] ||
    { echo 'FAIL: launch leader did not reserve PGID throughout teardown' >&2; exit 1; }
  if kill -0 "$(cat "$CASE_DIR/guardian-pid")" 2>/dev/null; then
    echo 'FAIL: guardian survived game teardown instead of being reaped' >&2
    exit 1
  fi
  [ "$(sort -u "$CASE_DIR/stop-sleeps")" = 0.1 ]
  [ "$(wc -l <"$CASE_DIR/stop-sleeps")" -le 10 ]
  if [ "$2" = COOPERATIVE ]; then
    [ "$(wc -l <"$CASE_DIR/stop-sleeps")" -lt 10 ] ||
      { echo 'FAIL: cooperative group waited the full deadline' >&2; exit 1; }
  fi
  if [ "$2" = NORMAL ]; then
    [ -f "$CASE_DIR/cleanup" ] && [ ! -f "$CASE_DIR/premature-term" ] ||
      { echo 'FAIL: group termination interrupted normal save/cleanup' >&2; exit 1; }
  fi
  # Allow init to reap killed orphans: a zombie is already dead but can briefly
  # keep kill -0 on the group successful. No running descendant may survive.
  i=0
  while kill -0 -- "-$group" 2>/dev/null && [ "$i" -lt 100 ]; do
    /bin/sleep 0.01
    i=$((i + 1))
  done
  if kill -0 -- "-$group" 2>/dev/null; then
    echo "FAIL: game group $group survived $1" >&2
    exit 1
  fi
  [ -f "$CASE_DIR/group-term" ] ||
    { echo 'FAIL: group teardown skipped TERM grace period' >&2; exit 1; }
  printf '%s\n' "PASS $1: no game group; guardian pinned through teardown and reaped"
}
exit_group_case wrapper-killed ORPHAN '' 137
exit_group_case wrapper-killed-shell ORPHAN '; :' 0
exit_group_case normal-save NORMAL '' 0
exit_group_case cooperative-exit COOPERATIVE '' 137

# An empty group must request no grace-period sleeps, including after a normal
# nonzero game exit. Also measure the handoff rather than session shutdown.
run_case empty-group 'game runemu.sh EMPTY
END'
[ ! -f "$CASE_DIR/stop-sleeps" ]
awk -v ended="$(cat "$CASE_DIR/empty-group-end")" \
    -v returned="$(cat "$CASE_DIR/empty-group-return")" 'BEGIN {
  elapsed = (returned - ended) / 1000000
  printf "PASS empty game group: zero sleeps, %.1f ms to launcher\n", elapsed
  exit !(elapsed < 1000)
}'

# The empty-group fast path still releases and reaps its guardian.
if kill -0 "$(cat "$CASE_DIR/guardian-pid")" 2>/dev/null; then
  echo 'FAIL: empty group left its guardian alive' >&2
  exit 1
fi
[ ! -e "$ARCHR_SESSION_RUN_DIR/game-exit" ]
[ ! -e "$ARCHR_SESSION_RUN_DIR/game-release" ]
export ARCHR_SESSION_STOP_SLEEP=/bin/sleep

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
! grep -q '4 s' "$ARCHR_SESSION_LOG"
[ ! -e "$ARCHR_SESSION_RUN_DIR/next" ]
[ "$(cat "$CASE_DIR/count")" = 1 ]
printf '%s\n' 'PASS launcher-only stop has no game cleanup window'
echo 'All session tests passed.'
