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
export ARCHR_SESSION_FAKE_SUSPEND="$root/bin/archr-fake-suspend"
export ARCHR_SESSION_SWAY_ATTEMPTS=0
export ARCHR_SESSION_SLEEP="$root/bin/sleep"
export ARCHR_SESSION_STOP_SLEEP=/bin/sleep
export ARCHR_SESSION_PROFILE="$root/profile"
export ARCHR_SESSION_SHELL=${SESSION_TEST_SHELL:-sh}
export ARCHR_TEST_EMULATOR="$root/bin/fake) emulator"
cat >"$ARCHR_SESSION_PROFILE" <<'EOF'
# Deliberately Bash-only, like ArchR's real /etc/profile.d/001-functions.
function get_setting() {
  case "$1" in
    system.language) printf '%s\n' pt_BR ;;
    system.suspendmode) printf '%s\n' "${ARCHR_TEST_SUSPEND_MODE-}" ;;
  esac
}
export MALI_WAYLAND_AFBC=0
export SDL_VIDEODRIVER=wayland
EOF

cat >"$root/bin/archr-launcher" <<'EOF'
#!/bin/sh
set -eu
[ "$LANG" = pt_BR.UTF-8 ]
[ "$LOCPATH" = /storage/.config/emulationstation/locale ]
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
if [ "${1-}" = LONG ]; then
  # Like the real runemu.sh: NO TERM trap. Cleanup is normal control flow
  # after its emulator exits, including a nonzero/signaled emulator status.
  printf '%s\n' "$$" >"$CASE_DIR/game-pids"
  "$ARCHR_TEST_EMULATOR" &
  child=$!
  wait "$child" || :
  /bin/sleep 0.15
  : >"$CASE_DIR/cleanup"
  exit 0
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
/bin/sleep 30 &
child=$!
printf '%s\n' "$$" "$child" >>"$CASE_DIR/game-pids"
: >"$CASE_DIR/game-ready"
wait "$child"
EOF
cat >"$root/bin/systemctl" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CASE_DIR/actions"
EOF
cat >"$root/bin/archr-fake-suspend" <<'EOF'
#!/bin/sh
printf '%s\n' "fake-suspend $*" >>"$CASE_DIR/actions"
EOF
cat >"$root/bin/stop-sleep" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CASE_DIR/stop-sleeps"
exec /bin/sleep "$@"
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

run_case() {
  name=$1
  export CASE_DIR="$root/$name"
  mkdir -p "$CASE_DIR"
  export ARCHR_SESSION_RUN_DIR="$CASE_DIR/run"
  export ARCHR_SESSION_LOG="$CASE_DIR/session.log"
  export ARCHR_SESSION_KILL_FILE="$CASE_DIR/kill-data"
  printf '%s\n' "$2" >"$CASE_DIR/plan"
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
}

run_case game 'game runemu.sh /roms/demo.zip -Pnes --core=auto --emulator=retroarch --controllers=
END'
[ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = 37 ]
[ ! -f "$CASE_DIR/sleeps" ]
printf '%s\n' 'PASS game round-trip and kill-target reset'

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
printf '%s
' 'PASS clean exit returns to launcher'

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
run_stop_case() {
  export CASE_DIR="$root/$1"
  mkdir -p "$CASE_DIR"
  export ARCHR_SESSION_RUN_DIR="$CASE_DIR/run"
  export ARCHR_SESSION_LOG="$CASE_DIR/session.log"
  export ARCHR_SESSION_KILL_FILE="$CASE_DIR/kill-data"
  ARCHR_SESSION_SWAY_ATTEMPTS=0
  export ARCHR_SESSION_SWAY_ATTEMPTS
  printf '%s\n' "$2" END >"$CASE_DIR/plan"
  timeout 12 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 &
  test_runner=$!
  i=0
  while [ ! -f "$CASE_DIR/game-ready" ] && [ "$i" -lt 200 ]; do
    /bin/sleep 0.01
    i=$((i + 1))
  done
  [ -f "$CASE_DIR/game-ready" ]
  kill -TERM "$(cat "$CASE_DIR/supervisor-pid")"
  status=0
  wait "$test_runner" || status=$?
  test_runner=
  [ -f "$CASE_DIR/cleanup" ] || { echo 'FAIL: service stop skipped game cleanup' >&2; exit 1; }
  [ "$status" -eq 0 ]
  while IFS= read -r pid; do
    if kill -0 "$pid" 2>/dev/null; then
      echo "FAIL: fake game process $pid survived service stop" >&2
      exit 1
    fi
  done <"$CASE_DIR/game-pids"
  if grep -q 'cleanup timed out' "$ARCHR_SESSION_LOG"; then
    echo 'FAIL: normal wrapper cleanup needed forced group termination' >&2
    exit 1
  fi
  printf '%s\n' "PASS $1 preserves untrapped wrapper cleanup and leaves no processes"
}
run_stop_case service-stop 'game runemu.sh LONG'
run_stop_case service-stop-shell 'game runemu.sh LONG; :'

run_case removed-actions 'action shutdown
action reboot
action settings
END'
[ ! -f "$CASE_DIR/actions" ]
printf '%s\n' 'PASS removed actions cannot execute in supervisor'
for mode in absent off mem; do
  ARCHR_TEST_SUSPEND_MODE=$mode
  [ "$mode" != absent ] || ARCHR_TEST_SUSPEND_MODE=
  export ARCHR_TEST_SUSPEND_MODE
  run_case "suspend-$mode" 'action suspend
END'
  expected='fake-suspend power'
  [ "$mode" != mem ] || expected=suspend
  [ "$(cat "$CASE_DIR/actions")" = "$expected" ]
  [ "$(cat "$CASE_DIR/count")" = 2 ]
done
printf '%s\n' 'PASS suspend policy and launcher return'

export CASE_DIR="$root/launcher-stop"
mkdir -p "$CASE_DIR"
export ARCHR_SESSION_RUN_DIR="$CASE_DIR/run"
export ARCHR_SESSION_LOG="$CASE_DIR/session.log"
export ARCHR_SESSION_KILL_FILE="$CASE_DIR/kill-data"
export ARCHR_SESSION_STOP_SLEEP="$root/bin/stop-sleep"
export ARCHR_SESSION_SWAY_ATTEMPTS=0
printf '%s\n' HOLD >"$CASE_DIR/plan"
timeout 8 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 &
test_runner=$!
i=0
while [ ! -f "$CASE_DIR/launcher-ready" ] && [ "$i" -lt 200 ]; do
  /bin/sleep 0.01
  i=$((i + 1))
done
[ -f "$CASE_DIR/launcher-ready" ]
kill -TERM "$(cat "$CASE_DIR/supervisor-pid")"
wait "$test_runner"
test_runner=
[ "$(cat "$CASE_DIR/stop-sleeps")" = 0.2 ]
! grep -q '4 s' "$ARCHR_SESSION_LOG"
[ ! -e "$ARCHR_SESSION_RUN_DIR/next" ]
[ "$(cat "$CASE_DIR/count")" = 1 ]
printf '%s\n' 'PASS launcher-only stop has no game cleanup window'
echo 'All session tests passed.'
