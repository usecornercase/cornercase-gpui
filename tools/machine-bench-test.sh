#!/bin/sh
set -u

repo=$(cd "$(dirname "$0")/.." && pwd)
script=$repo/tools/machine-bench.sh
sh_cmd=${SH:-sh}
total=0
failed=0

t=$(mktemp -d /tmp/mbt.XXXXXX) || exit 1
mkdir -p "$t/home/.ssh" "$t/home/.claude" "$t/home/.cornercase/worktrees/x" "$t/home/.local/state/cornercase" \
  "$t/home/.config/cornercase" "$t/control" "$t/bin" "$t/new" "$t/old"
for f in .ssh/config .claude/settings.json .cornercase/worktrees/x/f .local/state/cornercase/session.json \
  .config/cornercase/config.json; do
  printf 'sentinel\n' > "$t/home/$f"
done
: > "$t/calls"
: > "$t/pids"

cleanup() {
  while read -r p; do
    kill "$p" 2> /dev/null
  done < "$t/pids"
  case $t in
    /tmp/mbt.*) rm -rf "$t" ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' INT TERM HUP

HOME=$t/home
PATH=$t/bin:$PATH
export HOME PATH
unset CORNERCASE_SOCKET CORNERCASE_REMOTE CORNERCASE_PANE CORNERCASE_SERVER CORNERCASE_BIN CORNERCASE_RUN_DIR

make_stub() {
  cat > "$1/cornercase" <<'EOF'
#!/bin/sh
t=@T@
sock=${CORNERCASE_SOCKET-unset}
printf 'bin=%s|socket=%s|remote=%s|pane=%s|server=%s|home=%s|pwd=%s|path1=%s|claude=%s|args=%s\n' \
  "$0" "$sock" "${CORNERCASE_REMOTE-unset}" "${CORNERCASE_PANE-unset}" "${CORNERCASE_SERVER-unset}" \
  "${HOME-unset}" "$(pwd -P)" "${PATH%%:*}" "${CLAUDE_CONFIG_DIR-unset}" "$*" >> "$t/calls"
cmd=${1-}
state=$sock.stub
[ "$sock" = unset ] && state=$t/control/unset
needs_up() {
  [ -e "$sock" ] && return 0
  echo 'no cornercase server is running' >&2
  exit 1
}
case $cmd in
  --version) echo "cornercase @V@" ;;
  start-server) : > "$sock" ;;
  kill-server)
    needs_up
    held=no
    [ -e "$(dirname "$sock")/../ssh/vps.invalid.held" ] && held=yes
    held_there=no
    [ -e "$(dirname "$sock")/../ssh/there.invalid.held" ] && held_there=yes
    printf '%s held=%s\n' "$sock" "$held,$held_there" >> "$t/killed"
    rm -f "$sock" "$state"-agent "$state"-child ;;
  status)
    needs_up
    if [ "${2-}" = --panes ]; then
      printf 'pane\ttab\tagent\tparent\n'
      [ -e "$state-agent" ] && printf '3\t1\tclaude\t-\n'
      [ -e "$state-child" ] && printf '4\t1\tclaude\t3\n'
    fi
    exit 0 ;;
  open) echo 1 ;;
  start) : > "$state-agent"; echo 3 ;;
  send)
    : > "$state-child"
    echo 'not confirmed: the prompt may not have been submitted' >&2
    exit 1 ;;
  machines)
    if [ "${2-}" = add ] && [ -e "$t/control/fail-machines" ]; then
      exit 1
    fi
    s=linked
    e=null
    [ -f "$t/control/state" ] && s=$(cat "$t/control/state")
    [ -f "$t/control/error" ] && e=\"$(cat "$t/control/error")\"
    printf '{"here":{"icon":"laptop","version":"0.13.32"},"machines":[{"name":"vps","destination":"vps.invalid","state":"%s","version":"0.13.32","error":%s}]}\n' "$s" "$e" ;;
esac
exit 0
EOF
  sed -e "s|@T@|$t|g" -e "s|@V@|$2|g" "$1/cornercase" > "$1/cornercase.new"
  mv "$1/cornercase.new" "$1/cornercase"
  chmod 755 "$1/cornercase"
}

make_stub "$t/new" 0.13.32
make_stub "$t/old" 0.13.21
cp "$t/new/cornercase" "$t/bin/cornercase"

check() {
  msg=$1
  shift
  "$@" || {
    printf '%s\n' "$msg" > "$t/why"
    return 1
  }
}

run_test() {
  total=$((total + 1))
  rm -f "$t/why"
  if "$1"; then
    printf 'ok - %s\n' "$1"
  else
    failed=$((failed + 1))
    printf 'not ok - %s: %s\n' "$1" "$(cat "$t/why" 2> /dev/null)"
  fi
}

runb() {
  "$sh_cmd" "$script" "$@" > "$t/out" 2> "$t/err"
  rc=$?
}

runb_env() {
  (
    while :; do
      case ${1-} in
        *=*) export "${1%%=*}=${1#*=}"; shift ;;
        *) break ;;
      esac
    done
    "$sh_cmd" "$script" "$@" > "$t/out" 2> "$t/err"
    printf '%s\n' "$?" > "$t/rc"
  )
  rc=$(cat "$t/rc")
}

calls() {
  sed 's|=/private/tmp/|=/tmp/|g' "$t/calls" > "$t/calls.norm"
}

home_state() {
  { find "$HOME" | sort; find "$HOME" -type f -exec cksum {} + | sort; } 2> /dev/null
}

snap() {
  calls0=$(wc -l < "$t/calls")
  home0=$(home_state)
}

new_calls() {
  tail -n +"$((calls0 + 1))" "$t/calls" | grep -v 'args=--version$'
}

no_new_calls() {
  [ -z "$(new_calls)" ]
}

home_same() {
  [ "$(home_state)" = "$home0" ]
}

contains() {
  grep -q -F -- "$2" "$1"
}

has_call() {
  calls
  grep -F -- "$1" "$t/calls.norm" | grep -q -F -- "$2"
}

nothing_made() {
  check "the folder was made: $1" [ ! -e "$1" ] || return 1
  check "a cornercase ran" no_new_calls || return 1
  check "the home folder changed" home_same
}

refused() {
  target=$1
  shift
  snap
  runb "$@"
  check "exit $rc, not 2 ($*)" [ "$rc" = 2 ] || return 1
  [ -z "$target" ] || nothing_made "$target"
}

up_needs_a_folder() {
  refused "" up && refused "" up --agent
}

up_refuses_unknown_words_and_options() {
  refused "" frobnicate && refused "$t/b" up "$t/b" --nope && refused "$t/b" up "$t/b" --here &&
    refused "$t/c" up "$t/b" "$t/c"
}

up_refuses_a_relative_folder() {
  refused "cc119" up cc119
}

up_refuses_root() {
  refused "" up /
}

up_refuses_the_home_folder_and_what_holds_it() {
  refused "" up "$HOME" || return 1
  check "no word about the home folder" contains "$t/err" "home folder" || return 1
  refused "" up "$t"
}

up_refuses_a_folder_outside_tmp() {
  refused "/var/tmp/mbt-never-$$" up "/var/tmp/mbt-never-$$"
}

up_refuses_other_characters_and_dot_parts() {
  n=mbt-never-$$
  refused "/tmp/a b" up "/tmp/a b" && refused "/tmp/a\$b" up "/tmp/a\$b" && refused "/tmp/../tmp/$n" up "/tmp/../tmp/$n" &&
    refused "/tmp/./$n" up "/tmp/./$n" && refused "/tmp//$n" up "/tmp//$n"
}

up_refuses_a_folder_that_exists() {
  mkdir "$t/e1"
  : > "$t/e2"
  ln -s "$t/e1" "$t/e3"
  refused "" up "$t/e1" && refused "" up "$t/e2" && refused "" up "$t/e3" || return 1
  check "an existing thing was removed" [ -d "$t/e1" ] && [ -f "$t/e2" ] && [ -L "$t/e3" ]
}

up_refuses_a_folder_inside_a_git_repository() {
  GIT_CONFIG_GLOBAL=/dev/null git init -q "$t/repo" || return 1
  refused "$t/repo/bench" up "$t/repo/bench"
}

up_refuses_a_folder_whose_socket_would_be_too_long() {
  a=
  i=0
  while [ "$i" -lt 90 ]; do
    a=${a}a
    i=$((i + 1))
  done
  refused "/tmp/$a" up "/tmp/$a" || return 1
  check "no word about the length" contains "$t/err" "too long"
}

up_refuses_a_release_that_is_not_cornercase() {
  refused "$t/r1" up "$t/r1" --here /bin/echo && refused "$t/r2" up "$t/r2" --there "$t/missing"
}

up_refuses_the_callers_socket_inside_the_folder() {
  snap
  runb_env "CORNERCASE_SOCKET=$t/b2/here/server.sock" up "$t/b2"
  check "exit $rc, not 2" [ "$rc" = 2 ] || return 1
  nothing_made "$t/b2"
}

b=$t/b

make_main_bench() {
  runb_env "CORNERCASE_SOCKET=$t/caller.sock" CORNERCASE_REMOTE=devbox CORNERCASE_PANE=7 CORNERCASE_SERVER=x up "$b" --here "$t/new/cornercase" --agent --log
  [ "$rc" = 0 ] || { cat "$t/err" >&2; return 1; }
  calls
}

up_runs_every_cornercase_on_a_socket_of_the_bench() {
  bad=
  while IFS= read -r line; do
    case $line in
      "bin=$b/"*)
        case $line in
          *"|socket=$b/"*"|remote=unset|pane=unset|server=unset|"*) ;;
          *) bad=$line ;;
        esac ;;
      "bin="*)
        case $line in
          *"args=--version") ;;
          *) bad=$line ;;
        esac ;;
    esac
  done < "$t/calls.norm"
  check "a call without its socket: $bad" [ -z "$bad" ] || return 1
  check "the caller's socket or remote reached a call" test ! "$(grep -c -e caller.sock -e devbox "$t/calls.norm")" -gt 0
}

up_starts_each_server_from_its_own_project() {
  has_call "args=start-server" "socket=$b/here/server.sock|remote=unset|pane=unset|server=unset|home=$HOME|pwd=$b/here/here-proj|" &&
    has_call "args=start-server" "socket=$b/there/server.sock|remote=unset|pane=unset|server=unset|home=$b/there/home|pwd=$b/there/home/there-proj|"
}

up_gives_there_a_home_and_environment_of_its_own() {
  grep -F "socket=$b/there/" "$t/calls.norm" > "$t/there.calls"
  bad=$(grep -v -F -e "home=$b/there/home|" "$t/there.calls" | head -1)
  check "a there call with another home: $bad" [ -z "$bad" ] || return 1
  bad=$(grep -v -F -e "|path1=$b/there/bin|claude=$b/there/claude|" "$t/there.calls" | head -1)
  check "a there call with another PATH or CLAUDE_CONFIG_DIR: $bad" [ -z "$bad" ]
}

up_touches_nothing_outside_its_folder() {
  check "the home folder changed" home_same
}

up_makes_its_folders_private() {
  for d in "$b" "$b/here" "$b/there"; do
    case $(ls -ld "$d") in
      drwx------*) ;;
      *) check "$d is not private" false; return 1 ;;
    esac
  done
}

up_copies_each_release() {
  snap
  runb up "$t/b2" --here "$t/new/cornercase" --there "$t/old/cornercase"
  check "exit $rc: $(cat "$t/err")" [ "$rc" = 0 ] || return 1
  check "here is not a copy of the release" cmp -s "$t/new/cornercase" "$t/b2/here/bin/cornercase" || return 1
  check "there is not a copy of the release" cmp -s "$t/old/cornercase" "$t/b2/there/bin/cornercase" || return 1
  check "the summary misses a version" contains "$t/out" "0.13.21" || return 1
  contains "$t/out" "0.13.32"
}

config_pins_ok() {
  for s in here there; do
    c=$b/$s/config.json
    contains "$c" "\"worktrees_dir\": \"$b/$s/worktrees\"" || return 1
    contains "$c" "\"claude\": \"$b/$s/agent/claude\"" || return 1
    for a in codex gemini opencode grok cursor copilot vibe; do
      contains "$c" "\"$a\": \"/nonexistent/$a\"" || return 1
    done
  done
}

up_pins_every_agent_and_keeps_worktrees_inside() {
  check "a pin or the worktrees folder is wrong" config_pins_ok
}

up_turns_on_the_agents_section_on_here_only() {
  check "here lacks the agents section" contains "$b/here/config.json" '"agents_section": true' || return 1
  check "there has the agents section" test ! "$(grep -c agents_section "$b/there/config.json")" -gt 0
}

up_without_agent_pins_claude_and_leaves_the_section_off() {
  runb up "$t/b3" --here "$t/new/cornercase"
  check "a plain up failed: $(cat "$t/err")" [ "$rc" = 0 ] || return 1
  check "claude is not pinned without --agent" contains "$t/b3/here/config.json" '"claude": "/nonexistent/claude"' || return 1
  check "the agents section is on without --agent" test ! "$(grep -c agents_section "$t/b3/here/config.json")" -gt 0 || return 1
  contains "$t/b3/env" "export CORNERCASE_BIN=$t/b3/here/bin/cornercase"
}

up_refuses_an_empty_release() {
  refused "$t/e9" up "$t/e9" --here "" && refused "$t/e9" up "$t/e9" --there ""
}

up_refuses_a_folder_whose_parent_is_a_link_out_of_tmp() {
  outside=$(mktemp -d /var/tmp/mbt.XXXXXX) || return 1
  ln -s "$outside" "$t/lnk"
  refused "$outside/b" up "$t/lnk/b"
  r=$?
  rm -rf "$outside"
  return "$r"
}

up_links_there_as_vps() {
  check "no link on here's socket" has_call "args=machines add vps vps.invalid --icon server --json" "socket=$b/here/server.sock|"
}

up_starts_an_agent_on_there_with_a_child() {
  has_call "args=start claude --project 1" "socket=$b/there/server.sock|" || return 1
  has_call "args=send --pane 3 --enter spawn" "socket=$b/there/server.sock|" || return 1
  contains "$t/b.out" "pane 3, its child pane 4"
}

up_says_how_to_send_the_window_there() {
  contains "$t/b.out" "there.invalid" || return 1
  contains "$t/b.out" "machines edit vps --destination there.invalid"
}

up_accepts_an_older_there() {
  printf older > "$t/control/state"
  runb up "$t/b4" --here "$t/new/cornercase" --there "$t/old/cornercase"
  rm -f "$t/control/state"
  check "exit $rc: $(cat "$t/err")" [ "$rc" = 0 ] || return 1
  contains "$t/out" older
}

up_stops_its_servers_when_it_fails_and_keeps_its_folder() {
  : > "$t/killed"
  : > "$t/control/fail-machines"
  runb up "$t/f1" --here "$t/new/cornercase"
  rm -f "$t/control/fail-machines"
  check "exit $rc, not 1" [ "$rc" = 1 ] || return 1
  check "here was not stopped" contains "$t/killed" "$t/f1/here/server.sock" || return 1
  check "there was not stopped" contains "$t/killed" "$t/f1/there/server.sock" || return 1
  check "the folder or its mark is gone" [ -f "$t/f1/.machine-bench" ]
}

up_says_why_an_unreachable_machine_failed() {
  printf unreachable > "$t/control/state"
  printf 'ssh: connect to host vps.invalid port 22: Connection refused' > "$t/control/error"
  runb up "$t/f2" --here "$t/new/cornercase"
  rm -f "$t/control/state" "$t/control/error"
  check "exit $rc, not 1" [ "$rc" = 1 ] || return 1
  check "the error is not in the message" contains "$t/err" "Connection refused"
}

down_removes_the_folders_a_failed_up_kept() {
  runb down "$t/f1"
  check "down of f1: $(cat "$t/err")" [ "$rc" = 0 ] || return 1
  runb down "$t/f2"
  check "down of f2: $(cat "$t/err")" [ "$rc" = 0 ]
}

shell_env() {
  cat > "$t/readenv.sh" <<'EOF'
. "$1"
printf '%s\n' "$CORNERCASE_SOCKET" "${CORNERCASE_PANE-unset}${CORNERCASE_SERVER-unset}${CORNERCASE_REMOTE-unset}" \
  "$PATH" "$CORNERCASE_BIN" "$CORNERCASE_GPUI_SETTINGS" "$CORNERCASE_GPUI_BROWSER_PROFILE"
EOF
  env CORNERCASE_PANE=3 CORNERCASE_SERVER=x CORNERCASE_REMOTE=devbox CORNERCASE_SOCKET=elsewhere "$1" "$t/readenv.sh" "$b/env"
}

env_reads_right_in() {
  out=$(shell_env "$1") || return 1
  want="$b/here/server.sock
unsetunsetunset
$b/bin:$b/here/bin:$PATH
$b/here/cornercase-logged
$b/window/settings.json
$b/window/browser"
  [ "$out" = "$want" ] || { printf 'under %s: %s\n' "$1" "$out" > "$t/why"; return 1; }
}

the_env_aims_the_shell_at_here() {
  env_reads_right_in sh && env_reads_right_in dash || return 1
  if command -v zsh > /dev/null 2>&1; then
    env_reads_right_in zsh
  fi
}

there_cannot_reach_here() {
  env CORNERCASE_SOCKET="$b/here/server.sock" CORNERCASE_REMOTE=devbox "$sh_cmd" "$b/bin/there" status > /dev/null 2>&1
  has_call "args=status" "socket=$b/there/server.sock|remote=unset|" || return 1
  check "there did not run from its project" has_call "args=status" "pwd=$b/there/home/there-proj|" || return 1
  env CORNERCASE_REMOTE=devbox "$sh_cmd" "$b/bin/here" status > /dev/null 2>&1
  check "here did not carry its own socket and no remote" has_call "args=status" "socket=$b/here/server.sock|remote=unset|"
}

stand_in() {
  "$sh_cmd" "$b/bin/ssh" "$@" > "$t/out" 2> "$t/err"
  rc=$?
}

runs_a_link_against_there_as() {
  snap
  lines=$(wc -l < "$b/ssh/calls.log")
  stand_in -T -o ServerAliveInterval=5 -o ServerAliveCountMax=3 -o ConnectTimeout=10 -o BatchMode=yes -a -x \
    -o ClearAllForwardings=yes -- "$1" "sh -c 'exec cornercase status'"
  check "exit $rc: $(cat "$t/err")" [ "$rc" = 0 ] || return 1
  calls
  line=$(grep -F 'args=status' "$t/calls.norm" | tail -1)
  case $line in
    *"socket=$b/there/server.sock|"*"home=$b/there/home|pwd=$b/there/home/there-proj|path1=$b/there/bin|"*) ;;
    *) check "the stub saw: $line" false; return 1 ;;
  esac
  check "the stand-in logged nothing" [ "$(wc -l < "$b/ssh/calls.log")" -gt "$lines" ]
}

the_stand_in_runs_a_link_against_there() {
  runs_a_link_against_there_as vps.invalid
}

the_stand_in_runs_a_command_on_there_by_its_second_name() {
  runs_a_link_against_there_as there.invalid
}

the_stand_in_holds_each_name_on_its_own() {
  snap
  : > "$b/ssh/vps.invalid.held"
  stand_in -- there.invalid "sh -c 'exec cornercase status'"
  check "there.invalid while only vps.invalid is held: exit $rc: $(cat "$t/err")" [ "$rc" = 0 ] || { rm -f "$b/ssh/vps.invalid.held"; return 1; }
  stand_in -- vps.invalid "sh -c 'exec cornercase status'"
  rm -f "$b/ssh/vps.invalid.held"
  check "vps.invalid held: exit $rc" [ "$rc" = 255 ] || return 1
  : > "$b/ssh/there.invalid.held"
  stand_in -- vps.invalid "sh -c 'exec cornercase status'"
  check "vps.invalid while only there.invalid is held: exit $rc" [ "$rc" = 0 ] || { rm -f "$b/ssh/there.invalid.held"; return 1; }
  stand_in -- there.invalid "sh -c 'exec cornercase status'"
  rm -f "$b/ssh/there.invalid.held"
  check "there.invalid held: exit $rc" [ "$rc" = 255 ] || return 1
  check "there.invalid held: no Connection refused" contains "$t/err" "Connection refused"
}

the_stand_in_refuses_while_held() {
  snap
  : > "$b/ssh/vps.invalid.held"
  stand_in -- vps.invalid "sh -c 'exec cornercase status'"
  rm -f "$b/ssh/vps.invalid.held"
  check "held: exit $rc" [ "$rc" = 255 ] || return 1
  check "held: no Connection refused" contains "$t/err" "Connection refused" || return 1
  check "held: a command ran" no_new_calls
}

the_stand_in_runs_the_command_while_slow_is_zero() {
  printf 0 > "$b/ssh/vps.invalid.slow"
  stand_in -- vps.invalid "sh -c 'exec cornercase status'"
  rm -f "$b/ssh/vps.invalid.slow"
  check "slow 0: exit $rc" [ "$rc" = 0 ]
}

the_stand_in_answers_127_while_missing() {
  : > "$b/ssh/vps.invalid.missing"
  stand_in -- vps.invalid "sh -c 'exec cornercase status'"
  rm -f "$b/ssh/vps.invalid.missing"
  check "missing: exit $rc, not 127" [ "$rc" = 127 ]
}

refused_by_stand_in() {
  snap
  stand_in "$@"
  check "exit $rc, not 255 ($*)" [ "$rc" = 255 ] || return 1
  check "a command ran ($*)" no_new_calls
}

the_stand_in_refuses_other_hosts_masters_and_shells() {
  spec=127.0.0.1:61234:localhost:3000
  refused_by_stand_in -- devbox cmd &&
    refused_by_stand_in -N -T -L "$spec" -- devbox &&
    refused_by_stand_in -O check vps.invalid &&
    refused_by_stand_in -- vps.invalid &&
    refused_by_stand_in -f -N -L "$spec" -- vps.invalid &&
    refused_by_stand_in -N -D 1080 -- vps.invalid &&
    refused_by_stand_in -W localhost:3000 -- vps.invalid &&
    refused_by_stand_in -N -- vps.invalid &&
    refused_by_stand_in -N -L "$spec" -- vps.invalid "sh -c true"
}

free_port() {
  python3 -I -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])'
}

other_free_port() {
  other=$(free_port)
  while [ "$other" = "$1" ]; do
    other=$(free_port)
  done
  printf '%s\n' "$other"
}

echo_server() {
  python3 -I -c '
import socket, sys, threading
def serve(c):
    while True:
        d = c.recv(100)
        if not d:
            break
        c.sendall(d)
l = socket.socket()
l.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
l.bind(("127.0.0.1", int(sys.argv[1])))
l.listen(5)
while True:
    c, _ = l.accept()
    threading.Thread(target=serve, args=(c,), daemon=True).start()
' "$1" > /dev/null 2>&1 &
  track "$!"
}

late_server() {
  python3 -I -c '
import socket, sys, threading
def serve(c):
    while c.recv(100):
        pass
    c.sendall(b"pong")
    c.close()
l = socket.socket()
l.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
l.bind(("127.0.0.1", int(sys.argv[1])))
l.listen(5)
while True:
    c, _ = l.accept()
    threading.Thread(target=serve, args=(c,), daemon=True).start()
' "$1" > /dev/null 2>&1 &
  track "$!"
}

half_close_trip() {
  python3 -I -c '
import socket, sys
s = socket.create_connection(("127.0.0.1", int(sys.argv[1])), timeout=5)
s.sendall(b"ping")
s.shutdown(socket.SHUT_WR)
print(s.recv(10).decode())
' "$1" 2> /dev/null
}

round_trip() {
  python3 -I -c '
import socket, sys
s = socket.create_connection(("127.0.0.1", int(sys.argv[1])), timeout=5)
s.sendall(b"ping")
print(s.recv(10).decode())
' "$1" 2> /dev/null
}

until_answers() {
  n=0
  while [ "$n" -lt 50 ]; do
    [ "$(round_trip "$1")" = ping ] && return 0
    n=$((n + 1))
    sleep 0.1
  done
  return 1
}

until_gone() {
  n=0
  while [ "$n" -lt 50 ]; do
    kill -0 "$1" 2> /dev/null || return 0
    n=$((n + 1))
    sleep 0.1
  done
  return 1
}

forward_in() {
  "$sh_cmd" "$1/bin/ssh" -N -T -o ExitOnForwardFailure=yes -o BatchMode=yes "${4:--L}" "127.0.0.1:$2:localhost:$3" -- vps.invalid \
    > "$t/fout" 2> "$t/ferr" &
  forwarded=$!
  track "$forwarded"
}

the_stand_in_forwards_a_port_to_there() {
  far=$(free_port)
  near=$(other_free_port "$far")
  echo_server "$far"
  until_answers "$far" || { check "the echo server did not start" false; return 1; }
  forward_in "$b" "$near" "$far"
  until_answers "$near" || { check "nothing answered through the forward: $(cat "$t/ferr")" false; return 1; }
  check "the forward is not the stand-in's own process" contains_args "$forwarded" "$b/bin/forward" || return 1
  kill "$forwarded"
  check "SIGTERM did not end the forward" until_gone "$forwarded"
}

contains_args() {
  ps -o args= -p "$1" 2> /dev/null | grep -q -F -- "$2"
}

the_stand_in_forwards_a_reverse_forward_too() {
  far=$(free_port)
  near=$(other_free_port "$far")
  echo_server "$far"
  until_answers "$far" || { check "the echo server did not start" false; return 1; }
  forward_in "$b" "$near" "$far" -R
  until_answers "$near" || { check "nothing answered through the -R forward: $(cat "$t/ferr")" false; return 1; }
  kill "$forwarded"
  check "SIGTERM did not end the -R forward" until_gone "$forwarded"
}

the_stand_in_keeps_the_answer_after_a_client_half_closes() {
  far=$(free_port)
  near=$(other_free_port "$far")
  late_server "$far"
  n=0
  while [ "$n" -lt 50 ] && ! python3 -I -c 'import socket,sys;socket.create_connection(("127.0.0.1",int(sys.argv[1])),timeout=1)' "$far" 2> /dev/null; do
    n=$((n + 1))
    sleep 0.1
  done
  forward_in "$b" "$near" "$far"
  n=0
  answer=
  while [ "$n" -lt 50 ] && [ "$answer" != pong ]; do
    answer=$(half_close_trip "$near")
    n=$((n + 1))
    [ "$answer" = pong ] || sleep 0.1
  done
  kill "$forwarded"
  check "the answer after the client's half-close was '$answer', not pong" [ "$answer" = pong ]
}

the_stand_in_says_a_taken_port_as_ssh_does() {
  taken=$(free_port)
  far=$(free_port)
  echo_server "$taken"
  until_answers "$taken" || { check "the holder did not start" false; return 1; }
  stand_in -N -T -L "127.0.0.1:$taken:localhost:$far" -- vps.invalid
  check "exit $rc, not 255" [ "$rc" = 255 ] || return 1
  check "no ssh line: $(cat "$t/err")" contains "$t/err" "Could not request local forwarding." || return 1
  check "no bind line" contains "$t/err" "Address already in use"
}

the_stand_in_refuses_a_forward_while_held() {
  : > "$b/ssh/vps.invalid.held"
  stand_in -N -T -L "127.0.0.1:$(free_port):localhost:3000" -- vps.invalid
  rm -f "$b/ssh/vps.invalid.held"
  check "held: exit $rc" [ "$rc" = 255 ] || return 1
  check "held: no Connection refused" contains "$t/err" "Connection refused"
}

the_stand_in_forwards_only_loopback() {
  stand_in -N -T -L "0.0.0.0:$(free_port):localhost:3000" -- vps.invalid
  check "a wildcard bind: exit $rc" [ "$rc" = 255 ] || return 1
  stand_in -N -T -L "127.0.0.1:$(free_port):example.com:80" -- vps.invalid
  check "another host: exit $rc" [ "$rc" = 255 ] || return 1
  check "no word about loopback: $(cat "$t/err")" contains "$t/err" "loopback only"
}

the_stand_in_says_a_refused_connection_and_keeps_running() {
  far=$(free_port)
  near=$(other_free_port "$far")
  forward_in "$b" "$near" "$far"
  n=0
  while [ "$n" -lt 50 ] && [ -z "$(round_trip "$near")" ]; do
    n=$((n + 1))
    sleep 0.1
  done
  check "the forward ended" kill -0 "$forwarded" || return 1
  kill "$forwarded"
  until_gone "$forwarded"
  check "no open failed line: $(cat "$t/ferr")" contains "$t/ferr" "open failed: connect failed"
}

the_stand_in_answers_a_host_lookup_without_any_config() {
  stand_in -G -- github.com
  check "exit $rc" [ "$rc" = 0 ] || return 1
  check "the answer is '$(cat "$t/out")'" [ "$(cat "$t/out")" = "hostname github.com" ]
}

the_log_keeps_each_command_line_quoted() {
  cat > "$t/logcall.sh" <<'EOF'
"$1" send --pane 1 --enter "it's a b"
EOF
  "$sh_cmd" "$t/logcall.sh" "$b/here/cornercase-logged" > /dev/null 2>&1
  last=$(tail -1 "$b/commands.log")
  case $last in
    *"send --pane 1 --enter 'it'\\''s a b'") ;;
    *) check "the log line is: $last" false; return 1 ;;
  esac
  calls
  last=$(tail -1 "$t/calls.norm")
  case $last in
    *"args=send --pane 1 --enter it's a b") ;;
    *) check "the stub got: $last" false ;;
  esac
}

spawned_by_there() {
  new_calls | grep -F "bin=$b/there/bin/cornercase|" | grep -q '|args=start claude$'
}

the_fake_agent_reports_states_and_spawns_a_child() {
  snap
  printf 'working\nspawn\nhello\n' | env CORNERCASE_SOCKET="$b/there/server.sock" "$sh_cmd" "$b/there/agent/claude" > "$t/out" 2>&1
  esc=$(printf '\033')
  contains "$t/out" "${esc}]7501;state=idle${esc}\\" || { check "no idle report" false; return 1; }
  contains "$t/out" "${esc}]7501;state=working${esc}\\" || { check "no working report" false; return 1; }
  check "the typed line was not echoed" contains "$t/out" "fake claude: hello" || return 1
  check "no child was started by there's own copy" spawned_by_there || return 1
  "$sh_cmd" "$b/there/agent/claude" -p x > /dev/null 2>&1
  check "claude -p did not fail" [ "$?" = 1 ]
}

refused_for_status_and_down() {
  snap
  for word in status down; do
    runb "$word" "$1"
    check "$word $1: exit $rc, not 2" [ "$rc" = 2 ] || return 1
  done
  check "a cornercase ran" no_new_calls
}

status_and_down_refuse_a_folder_up_did_not_make() {
  mkdir "$t/plain" "$t/files"
  : > "$t/files/a"
  cp -R "$b" "$t/bcopy"
  refused_for_status_and_down "$t/plain" && refused_for_status_and_down "$t/files" && refused_for_status_and_down / || return 1
  printf 'machine-bench 1 %s\n' "$HOME" > "$HOME/.machine-bench"
  refused_for_status_and_down "$HOME"
  r=$?
  rm -f "$HOME/.machine-bench"
  [ "$r" = 0 ] || return 1
  refused_for_status_and_down "$t/bcopy" || return 1
  check "an existing folder was removed" [ -d "$t/plain" ] && [ -d "$t/files" ] && [ -d "$t/bcopy" ] || return 1
  check "kill-server ran" test ! "$(grep -c "$t/bcopy" "$t/killed")" -gt 0
}

down_refuses_from_inside_the_bench() {
  lines=$(wc -l < "$t/killed")
  runb_env CORNERCASE_PANE=3 "CORNERCASE_SOCKET=$b/there/server.sock" down "$b"
  check "exit $rc, not 2" [ "$rc" = 2 ] || return 1
  check "kill-server ran" [ "$(wc -l < "$t/killed")" = "$lines" ]
}

lingers() {
  mkdir -p "$1/here/bin"
  printf '#!/bin/sh\necho $$ > %s/lingers.pid\nwhile :; do sleep 1; done\n' "$1" > "$1/here/bin/lingers"
  chmod 755 "$1/here/bin/lingers"
}

track() {
  printf '%s\n' "$1" >> "$t/pids"
}

down_refuses_while_a_window_runs() {
  lines=$(wc -l < "$t/killed")
  mkdir "$t/w" "$t/w2"
  ln -s /bin/sleep "$t/w/cornercase-gpui"
  "$t/w/cornercase-gpui" 30 > /dev/null 2>&1 &
  wpid=$!
  track "$wpid"
  printf '%s\n' "$wpid" > "$b/window.pid"
  runb down "$b"
  check "pidfile window: exit $rc, not 2" [ "$rc" = 2 ] || return 1
  check "its pid is not named" contains "$t/err" "$wpid" || return 1
  rm -f "$b/window.pid"
  kill "$wpid" 2> /dev/null
  lingers "$b"
  ln -s "$(command -v xargs)" "$t/w2/cornercase-gpui"
  printf 'x\n' | "$t/w2/cornercase-gpui" "$b/here/bin/lingers" > /dev/null 2>&1 &
  xpid=$!
  track "$xpid"
  sleep 1
  runb down "$b"
  rc2=$rc
  kill "$xpid" 2> /dev/null
  [ ! -f "$b/lingers.pid" ] || kill "$(cat "$b/lingers.pid")" 2> /dev/null
  check "parent window: exit $rc2, not 2" [ "$rc2" = 2 ] || return 1
  check "kill-server ran" [ "$(wc -l < "$t/killed")" = "$lines" ]
}

down_stops_here_then_there_and_removes_the_folder() {
  sleep 1
  : > "$t/killed"
  snap
  runb down "$b"
  check "exit $rc: $(cat "$t/err")" [ "$rc" = 0 ] || return 1
  first=$(sed -n 1p "$t/killed")
  second=$(sed -n 2p "$t/killed")
  check "here was not first: $first" test "$first" = "$b/here/server.sock held=yes,yes" || return 1
  check "there was not second: $second" test "$second" = "$b/there/server.sock held=yes,yes" || return 1
  check "the folder is still there" [ ! -e "$b" ] || return 1
  grep -F 'args=kill-server --yes' "$t/calls" | grep -q 'socket=unset' && { check "a kill-server had no socket" false; return 1; }
  check "the home folder changed" home_same
}

down_keeps_the_folder_while_bench_processes_remain() {
  runb up "$t/b5" --here "$t/new/cornercase"
  check "up: $(cat "$t/err")" [ "$rc" = 0 ] || return 1
  mkdir "$t/cornercase-gpui"
  lingers "$t/b5"
  cat > "$t/cornercase-gpui/start.sh" <<'EOF'
#!/bin/sh
"$1" > /dev/null 2>&1
EOF
  chmod 755 "$t/cornercase-gpui/start.sh"
  "$t/cornercase-gpui/start.sh" "$t/b5/here/bin/lingers" > /dev/null 2>&1 &
  spid=$!
  track "$spid"
  sleep 1
  runb down "$t/b5"
  rc1=$rc
  lpid=$(cat "$t/b5/lingers.pid" 2> /dev/null)
  [ -z "$lpid" ] || track "$lpid"
  kill "$lpid" "$spid" 2> /dev/null
  check "taken for a window or stopped: exit $rc1" [ "$rc1" = 1 ] || return 1
  check "its pid is not listed" contains "$t/err" "$lpid" || return 1
  check "the folder was removed" [ -d "$t/b5" ] || return 1
  sleep 1
  runb down "$t/b5"
  check "second down: exit $rc: $(cat "$t/err")" [ "$rc" = 0 ]
}

down_lists_a_forward_left_running() {
  runb up "$t/b6" --here "$t/new/cornercase"
  check "up: $(cat "$t/err")" [ "$rc" = 0 ] || return 1
  forward_in "$t/b6" "$(free_port)" "$(free_port)"
  n=0
  while [ "$n" -lt 50 ] && ! contains_args "$forwarded" "$t/b6/bin/forward"; do
    n=$((n + 1))
    sleep 0.1
  done
  runb status "$t/b6"
  check "status names no forward: $(cat "$t/out")" contains "$t/out" "forward pid $forwarded" || { kill "$forwarded"; return 1; }
  runb down "$t/b6"
  rc1=$rc
  kill "$forwarded" 2> /dev/null
  until_gone "$forwarded"
  check "down with a forward left: exit $rc1" [ "$rc1" = 1 ] || return 1
  check "its pid is not listed" contains "$t/err" "$forwarded" || return 1
  check "the folder was removed" [ -d "$t/b6" ] || return 1
  sleep 1
  runb down "$t/b6"
  check "second down: exit $rc: $(cat "$t/err")" [ "$rc" = 0 ]
}

status_names_the_files_of_both_names() {
  : > "$b/ssh/there.invalid.slow"
  : > "$b/ssh/vps.invalid.held"
  runb status "$b"
  rm -f "$b/ssh/there.invalid.slow" "$b/ssh/vps.invalid.held"
  check "there.invalid.slow is not named: $(cat "$t/out")" contains "$t/out" "ssh/there.invalid.slow is there" || return 1
  check "vps.invalid.held is not named" contains "$t/out" "ssh/vps.invalid.held is there"
}

status_names_both_remembered_lists() {
  runb status "$b"
  check "status: exit $rc" [ "$rc" = 0 ] || return 1
  check "the lists are named: $(cat "$t/out")" contains "$t/out" "the window's own list: remembered-tunnels.json is not there" || return 1
  mkdir -p "$b/window"
  : > "$b/window/remembered-tunnels.json"
  : > "$b/there/tunnels.json"
  runb status "$b"
  rm -f "$b/window/remembered-tunnels.json" "$b/there/tunnels.json"
  check "the window's list is not there" contains "$t/out" "the window's own list: remembered-tunnels.json is there" || return 1
  check "there's list is not there" contains "$t/out" "cornercase's list on there: tunnels.json is there" || return 1
  check "here's list is there" contains "$t/out" "cornercase's list on here: tunnels.json is not there"
}

run_test up_needs_a_folder
run_test up_refuses_unknown_words_and_options
run_test up_refuses_a_relative_folder
run_test up_refuses_root
run_test up_refuses_the_home_folder_and_what_holds_it
run_test up_refuses_a_folder_outside_tmp
run_test up_refuses_other_characters_and_dot_parts
run_test up_refuses_a_folder_that_exists
run_test up_refuses_a_folder_inside_a_git_repository
run_test up_refuses_a_folder_whose_socket_would_be_too_long
run_test up_refuses_a_release_that_is_not_cornercase
run_test up_refuses_the_callers_socket_inside_the_folder

snap
make_main_bench || {
  printf 'not ok - the main bench could not be made\n'
  exit 1
}
cp "$t/out" "$t/b.out"
run_test up_runs_every_cornercase_on_a_socket_of_the_bench
run_test up_starts_each_server_from_its_own_project
run_test up_gives_there_a_home_and_environment_of_its_own
run_test up_touches_nothing_outside_its_folder
run_test up_makes_its_folders_private
run_test up_copies_each_release
run_test up_pins_every_agent_and_keeps_worktrees_inside
run_test up_turns_on_the_agents_section_on_here_only
run_test up_without_agent_pins_claude_and_leaves_the_section_off
run_test up_refuses_an_empty_release
run_test up_refuses_a_folder_whose_parent_is_a_link_out_of_tmp
run_test up_links_there_as_vps
run_test up_starts_an_agent_on_there_with_a_child
run_test up_says_how_to_send_the_window_there
run_test up_accepts_an_older_there
run_test up_stops_its_servers_when_it_fails_and_keeps_its_folder
run_test up_says_why_an_unreachable_machine_failed
run_test down_removes_the_folders_a_failed_up_kept
run_test the_env_aims_the_shell_at_here
run_test there_cannot_reach_here
run_test the_stand_in_runs_a_link_against_there
run_test the_stand_in_refuses_while_held
run_test the_stand_in_runs_a_command_on_there_by_its_second_name
run_test the_stand_in_holds_each_name_on_its_own
run_test the_stand_in_runs_the_command_while_slow_is_zero
run_test the_stand_in_answers_127_while_missing
run_test the_stand_in_refuses_other_hosts_masters_and_shells
run_test the_stand_in_forwards_a_port_to_there
run_test the_stand_in_forwards_a_reverse_forward_too
run_test the_stand_in_keeps_the_answer_after_a_client_half_closes
run_test the_stand_in_says_a_taken_port_as_ssh_does
run_test the_stand_in_refuses_a_forward_while_held
run_test the_stand_in_forwards_only_loopback
run_test the_stand_in_says_a_refused_connection_and_keeps_running
run_test the_stand_in_answers_a_host_lookup_without_any_config
run_test the_log_keeps_each_command_line_quoted
run_test the_fake_agent_reports_states_and_spawns_a_child
run_test status_names_the_files_of_both_names
run_test status_names_both_remembered_lists
run_test status_and_down_refuse_a_folder_up_did_not_make
run_test down_refuses_from_inside_the_bench
run_test down_refuses_while_a_window_runs
run_test down_stops_here_then_there_and_removes_the_folder
run_test down_keeps_the_folder_while_bench_processes_remain
run_test down_lists_a_forward_left_running

printf '%s tests, %s failed\n' "$total" "$failed"
[ "$failed" = 0 ]
