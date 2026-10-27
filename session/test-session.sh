#!/bin/sh
# Hermetic supervisor contract tests. SESSION_TEST_SHELL selects the child sh.
set -eu
session=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/archr-launcher-session
root=$(mktemp -d "${TMPDIR:-/tmp}/archr-session-test.XXXXXX")
trap 'rm -rf "$root"' EXIT HUP INT TERM
mkdir -p "$root/bin"
export PATH="$root/bin:$PATH"
export ARCHR_SESSION_TEST=1
export ARCHR_SESSION_LAUNCHER=archr-launcher
export ARCHR_SESSION_SETTINGS="$root/bin/archr-settings"
export ARCHR_SESSION_SWAY_ATTEMPTS=0
export ARCHR_SESSION_SLEEP="$root/bin/sleep"
export ARCHR_SESSION_PROFILE="$root/profile"
export ARCHR_SESSION_SHELL=${SESSION_TEST_SHELL:-sh}
cat >"$ARCHR_SESSION_PROFILE" <<'EOF'
# Deliberately Bash-only, like ArchR's real /etc/profile.d/001-functions.
function get_setting() { printf '%s\n' pt_BR; }
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
  CRASH) exit 9 ;;
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
cat >"$root/bin/systemctl" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CASE_DIR/actions"
EOF
cat >"$root/bin/archr-settings" <<'EOF'
#!/bin/sh
printf '%s\n' settings >>"$CASE_DIR/actions"
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
  # END terminates the supervisor; a timeout or an earlier failure is not success.
  status=0
  timeout 15 "${SESSION_TEST_SHELL:-sh}" "$session" >"$CASE_DIR/output" 2>&1 || status=$?
  if [ "$status" -ne 143 ] || [ ! -f "$CASE_DIR/finished" ]; then
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

run_case reboot 'action reboot
END'
[ "$(cat "$CASE_DIR/actions")" = reboot ]
printf '%s\n' 'PASS reboot action'

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

run_case clean 'CLEAN
action shutdown
action suspend
action settings
END'
printf '%s\n' poweroff suspend settings >"$CASE_DIR/expected"
cmp "$CASE_DIR/expected" "$CASE_DIR/actions"
[ ! -f "$CASE_DIR/sleeps" ]
printf '%s\n' 'PASS clean exit and remaining actions'

ARCHR_SESSION_SETTINGS="$root/missing-settings"
export ARCHR_SESSION_SETTINGS
run_case missing-settings 'action settings
game runemu.sh demo
END'
[ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = 37 ]
grep -q 'settings.*unavailable' "$ARCHR_SESSION_LOG"
printf '%s\n' 'PASS unavailable settings returns to launcher'

# A missing compositor consumes exactly the bounded 100 polls, then continues.
ARCHR_SESSION_SWAY_ATTEMPTS=100
export ARCHR_SESSION_SWAY_ATTEMPTS
XDG_RUNTIME_DIR="$root/no-compositor"
export XDG_RUNTIME_DIR
run_case sway-timeout 'action reboot
END'
[ "$(wc -l <"$CASE_DIR/sleeps" | tr -d ' ')" = 100 ]
[ "$(sort -u "$CASE_DIR/sleeps")" = 0.1 ]
[ "$(cat "$CASE_DIR/actions")" = reboot ]
printf '%s\n' 'PASS bounded sway wait'

ARCHR_SESSION_SWAY_ATTEMPTS=0
export ARCHR_SESSION_SWAY_ATTEMPTS
run_case log-cap 'LOUD
game runemu.sh LOGCHECK
action reboot
END'
[ "$(wc -c <"$ARCHR_SESSION_LOG")" -lt 262144 ]
[ "$(cat "$ARCHR_SESSION_RUN_DIR/last-exit")" = 37 ]
[ "$(cat "$CASE_DIR/actions")" = reboot ]
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
  run_case sway-ready 'action reboot
END'
  [ ! -f "$CASE_DIR/sleeps" ]
  [ "$(cat "$CASE_DIR/sway-calls")" = "-s $XDG_RUNTIME_DIR/sway-ipc.0.sock -t get_outputs" ]
  printf '%s\n' 'PASS ready compositor starts without delay'
fi

echo 'All session tests passed.'
