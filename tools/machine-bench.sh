#!/bin/sh
set -eu
umask 077

name=vps
dest=vps.invalid
second=there.invalid
agents="claude codex gemini opencode grok cursor copilot vibe"
repo=$(cd "$(dirname "$0")/.." && pwd)
me=$(basename "$0")
folder=
side=
window_bin=
agent=0
log=0
here_src=
there_src=
here_ver=
there_ver=
stage=
finished=1

say() {
  printf 'machine-bench.sh: %s\n' "$*" >&2
}

refuse() {
  say "$*"
  exit 2
}

fail() {
  say "$*"
  exit 1
}

step() {
  stage=$*
  printf '%s\n' "$*" >&2
}

usage() {
  cat <<'EOF'
usage: tools/machine-bench.sh up FOLDER [--here BIN] [--there BIN] [--agent] [--log]
       tools/machine-bench.sh status FOLDER
       tools/machine-bench.sh down FOLDER
       tools/machine-bench.sh -h | --help

FOLDER is a folder under /tmp: up makes it, so it must not exist yet.
--here and --there are cornercase releases, copied into the bench; --there defaults to --here, and --here to cornercase on the PATH.
Exit 0 done, 1 failed on the way, 2 wrong usage or refused, nothing done.
See design/manual-check.md, Another machine.
EOF
}

usage_fail() {
  say "$*"
  usage >&2
  exit 2
}

need() {
  command -v "$1" > /dev/null 2>&1 || fail "$1 is needed"
}

valid_path() {
  [ -n "$1" ] || refuse "the folder is empty"
  folder=${1%/}
  [ -n "$folder" ] || refuse "the root is not a folder for the bench"
  case $folder in /*) ;; *) refuse "$folder is not an absolute path" ;; esac
  case $folder in *[!abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._/-]*) refuse "$folder has a character outside A-Z a-z 0-9 . _ / -" ;; esac
  case $folder in
    *//* | */./* | */../* | */. | */..) refuse "$folder has an empty, . or .. part" ;;
  esac
  case "${HOME-}/" in
    "$folder"/*) refuse "$folder is, or holds, the home folder" ;;
  esac
  case $folder in
    /tmp/?* | /private/tmp/?*) ;;
    *) refuse "$folder is not a folder under /tmp" ;;
  esac
}

marked() {
  mark=$folder/.machine-bench
  [ -d "$folder" ] && [ ! -L "$folder" ] && [ -f "$mark" ] && [ ! -L "$mark" ] ||
    refuse "$folder was not made by up"
  content=$(cat "$mark")
  [ "$content" = "machine-bench 1 $folder" ] && return 0
  case $content in
    "machine-bench 1 "*) refuse "$folder was made for another folder" ;;
    *) refuse "$folder was not made by up" ;;
  esac
}

resolve() {
  case $1 in
    */*) printf '%s/%s' "$(cd "$(dirname "$1")" 2> /dev/null && pwd)" "$(basename "$1")" ;;
    *) command -v "$1" || true ;;
  esac
}

release() {
  path=$(resolve "$1")
  [ -n "$path" ] && [ -f "$path" ] && [ -x "$path" ] || refuse "$1 is not an executable file"
  out=$(env -i PATH="$PATH" HOME="${HOME-}" "$path" --version 2> /dev/null) || refuse "$path --version failed"
  case $out in
    cornercase\ [0-9]*) ;;
    *) refuse "$path --version printed '$out', not 'cornercase <version>'" ;;
  esac
  release_path=$path
  release_ver=${out#cornercase }
  release_ver=${release_ver%% *}
}

fresh_folder() {
  need git
  need cp
  if [ -e "$folder" ] || [ -L "$folder" ]; then
    refuse "$folder exists already"
  fi
  parent=${folder%/*}
  [ -d "$parent" ] || refuse "$parent does not exist"
  real=$(cd "$parent" && pwd -P)
  case $real in
    /tmp | /tmp/* | /private/tmp | /private/tmp/*) ;;
    *) refuse "$parent is a link to $real, outside /tmp" ;;
  esac
  [ "$(git -C "$parent" rev-parse --is-inside-work-tree 2> /dev/null || true)" != true ] ||
    refuse "$folder would be inside a git repository"
  socket=$folder/there/server.sock
  [ "${#socket}" -le 103 ] || refuse "$socket is too long for a socket (103 bytes at most)"
  case ${CORNERCASE_SOCKET-} in
    "$folder"/*) refuse "CORNERCASE_SOCKET of the caller is inside $folder" ;;
  esac
}

emit() {
  sed -e "s|@F@|$folder|g" -e "s|@S@|$side|g" -e "s|@B@|$window_bin|g" -e "s|@D@|$dest|g" -e "s|@E@|$second|g" > "$1"
  chmod "$2" "$1"
}

write_env() {
  window_bin=$folder/here/bin/cornercase
  if [ "$log" = 1 ]; then
    window_bin=$folder/here/cornercase-logged
  fi
  emit "$folder/env" 600 <<'EOF'
export PATH="@F@/bin:@F@/here/bin:$PATH"
export CORNERCASE_SOCKET=@F@/here/server.sock
export CLAUDE_CONFIG_DIR=@F@/here/claude
export CORNERCASE_BIN=@B@
export CORNERCASE_GPUI_SETTINGS=@F@/window/settings.json
export CORNERCASE_GPUI_BROWSER_PROFILE=@F@/window/browser
unset CORNERCASE_PANE CORNERCASE_SERVER CORNERCASE_REMOTE
EOF
}

write_commands() {
  emit "$folder/bin/here" 700 <<'EOF'
#!/bin/sh
. @F@/env
unset CORNERCASE_RUN_DIR
cd @F@/here/here-proj || exit 1
exec @F@/here/bin/cornercase "$@"
EOF
  emit "$folder/there/run" 700 <<'EOF'
#!/bin/sh
cd @F@/there/home/there-proj || exit 255
exec env -i HOME=@F@/there/home USER="${USER-}" LOGNAME="${LOGNAME-}" SHELL="${SHELL:-/bin/sh}" \
  LANG="${LANG-}" TERM="${TERM-}" PATH=@F@/there/bin:/usr/bin:/bin:/usr/sbin:/sbin \
  CORNERCASE_SOCKET=@F@/there/server.sock CLAUDE_CONFIG_DIR=@F@/there/claude "$@"
EOF
  emit "$folder/bin/there" 700 <<'EOF'
#!/bin/sh
exec @F@/there/run @F@/there/bin/cornercase "$@"
EOF
  ln -s ../../bin/ssh "$folder/there/bin/ssh"
}

write_forward() {
  emit "$folder/bin/forward" 600 <<'EOF'
import socket
import sys
import threading


def pipe(source, target):
    try:
        while True:
            data = source.recv(65536)
            if not data:
                break
            target.sendall(data)
    except OSError:
        pass
    try:
        target.shutdown(socket.SHUT_WR)
    except OSError:
        pass


def relay(client, far):
    ends = [threading.Thread(target=pipe, args=pair, daemon=True) for pair in ((client, far), (far, client))]
    for end in ends:
        end.start()
    for end in ends:
        end.join()
    client.close()
    far.close()


def serve(listener, host, port):
    while True:
        try:
            client, _ = listener.accept()
        except OSError:
            return
        try:
            far = socket.create_connection((host, port), timeout=5)
        except OSError as error:
            sys.stderr.write("channel 2: open failed: connect failed: %s\n" % (error.strerror or error))
            sys.stderr.flush()
            client.close()
            continue
        far.settimeout(None)
        threading.Thread(target=relay, args=(client, far), daemon=True).start()


def loopback(name):
    return name in ("127.0.0.1", "localhost", "::1")


def parse(kind, spec):
    parts = spec.split(":")
    if len(parts) == 3:
        parts.insert(0, "127.0.0.1")
    if len(parts) != 4 or not (parts[1].isdigit() and parts[3].isdigit()):
        sys.stderr.write("Bad %s forwarding specification '%s'\n" % (kind, spec))
        sys.exit(255)
    bind, listen, host, port = parts[0], int(parts[1]), parts[2], int(parts[3])
    if not (loopback(bind) and loopback(host)):
        sys.stderr.write("machine-bench ssh: %s: the bench forwards loopback only\n" % spec)
        sys.exit(255)
    return bind, listen, host, port


def report(kind, bind, bound, host, port, asked, level):
    if kind != "-R":
        return
    if asked == 0 and level in ("INFO", "VERBOSE", "DEBUG", "DEBUG1", "DEBUG2", "DEBUG3"):
        sys.stderr.write("Allocated port %d for remote forward to %s:%d\n" % (bound, host, port))
    elif asked != 0 and level.startswith("DEBUG"):
        sys.stderr.write("debug1: remote forward success for: listen %s:%d, connect %s:%d\n" % (bind, bound, host, port))
    sys.stderr.flush()


def listen_on(kind, bind, listen):
    family = socket.AF_INET6 if ":" in bind else socket.AF_INET
    listener = socket.socket(family, socket.SOCK_STREAM)
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        listener.bind((bind, listen))
        listener.listen(64)
    except OSError:
        sys.stderr.write("bind [%s]:%d: Address already in use\n" % (bind, listen))
        if kind == "-L":
            sys.stderr.write("channel_setup_fwd_listener_tcpip: cannot listen to port: %d\n" % listen)
            sys.stderr.write("Could not request local forwarding.\n")
        else:
            sys.stderr.write("Error: remote port forwarding failed for listen port %d\n" % listen)
        sys.exit(255)
    return listener


def main():
    wanted = []
    for found in sys.argv[1].split(","):
        kind, spec = found.split(":", 1)
        wanted.append((kind, parse(kind, spec)))
    level = sys.argv[2].upper() if len(sys.argv) > 2 else "INFO"
    listeners = []
    for kind, (bind, listen, host, port) in wanted:
        listener = listen_on(kind, bind, listen)
        report(kind, bind, listener.getsockname()[1], host, port, listen, level)
        listeners.append((listener, host, port))
    threads = [threading.Thread(target=serve, args=found, daemon=True) for found in listeners]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()


try:
    main()
except KeyboardInterrupt:
    pass
EOF
}

write_ssh() {
  emit "$folder/bin/ssh" 700 <<'EOF'
#!/bin/sh
dir=@F@/ssh
printf '%s %s\n' "$(date +%H:%M:%S)" "$*" >> "$dir/calls.log"
config=0
level=INFO
net=0
forwards=
refuse_flag() {
  printf 'machine-bench ssh: %s: the bench keeps no master and no agent forwarding; it forwards only -N with -L or -R\n' "$1" >&2
  exit 255
}
add_forward() {
  [ -n "$2" ] || exit 255
  forwards="${forwards:+$forwards,}$1:$2"
}
while [ $# -gt 0 ]; do
  case $1 in
    --) shift; break ;;
    -G) config=1; shift ;;
    -N) net=1; shift ;;
    -L | -R) [ $# -ge 2 ] || exit 255; add_forward "$1" "$2"; shift 2 ;;
    -L?*) add_forward -L "${1#-L}"; shift ;;
    -R?*) add_forward -R "${1#-R}"; shift ;;
    -f | -D* | -W* | -O*) refuse_flag "$1" ;;
    -o) [ $# -ge 2 ] || exit 255
      case $2 in [Ll][Oo][Gg][Ll][Ee][Vv][Ee][Ll]=*) level=${2#*=} ;; esac
      shift 2 ;;
    -[BbcEeFIiJlmPpQSw]) [ $# -ge 2 ] || exit 255; shift 2 ;;
    -*) shift ;;
    *) break ;;
  esac
done
dest=${1-}
if [ -z "$dest" ]; then
  printf 'usage: ssh [options] destination [command]\n' >&2
  exit 255
fi
shift
if [ "$config" = 1 ]; then
  printf 'hostname %s\n' "$dest"
  exit 0
fi
if [ "$net" = 1 ]; then
  if [ $# -ne 0 ] || [ -z "$forwards" ]; then
    printf 'machine-bench ssh: %s: -N needs forwards and no command\n' "$dest" >&2
    exit 255
  fi
elif [ -n "$forwards" ]; then
  printf 'machine-bench ssh: %s: a forward needs -N and no command\n' "$dest" >&2
  exit 255
elif [ $# -eq 0 ]; then
  printf 'machine-bench ssh: %s: no command; the bench opens no shell\n' "$dest" >&2
  exit 255
fi
case $dest in
  @D@ | @E@) ;;
  *)
    printf 'ssh: Could not resolve hostname %s: Name or service not known\n' "$dest" >&2
    exit 255
    ;;
esac
held() {
  [ -e "$dir/$dest.held" ] || return 0
  printf 'ssh: connect to host %s port 22: Connection refused\n' "$dest" >&2
  exit 255
}
held
if [ -e "$dir/$dest.slow" ]; then
  n=$(cat "$dir/$dest.slow")
  case $n in '' | *[!0-9]*) n=8 ;; esac
  sleep "$n"
  held
fi
if [ "$net" = 1 ]; then
  command -v python3 > /dev/null 2>&1 || {
    printf 'machine-bench ssh: forwarding needs python3\n' >&2
    exit 255
  }
  exec python3 -I @F@/bin/forward "$forwards" "$level"
fi
if [ -e "$dir/$dest.missing" ]; then
  printf 'sh: 1: cornercase: not found\n' >&2
  exit 127
fi
exec @F@/there/run /bin/sh -c "$*"
EOF
}

write_logged() {
  emit "$folder/here/cornercase-logged" 700 <<'EOF'
#!/bin/sh
quote() {
  case $1 in
    '' | *[!A-Za-z0-9_./:=@%+,-]*) ;;
    *) printf '%s' "$1"; return ;;
  esac
  rest=$1
  out=
  while :; do
    case $rest in
      *\'*) out=$out${rest%%\'*}"'\\''"; rest=${rest#*\'} ;;
      *) out=$out$rest; break ;;
    esac
  done
  printf "'%s'" "$out"
}
line=$(date +%H:%M:%S)
for arg in "$@"; do
  line="$line $(quote "$arg")"
done
printf '%s\n' "$line" >> @F@/commands.log
exec @F@/here/bin/cornercase "$@"
EOF
}

write_agent() {
  side=$1
  mkdir "$folder/$side/agent"
  emit "$folder/$side/agent/claude" 700 <<'EOF'
#!/bin/sh
case ${1-} in
  -p | --print | auth | mcp) printf 'fake claude: %s is not faked\n' "$*" >&2; exit 1 ;;
  --version) echo '0.0.0 (fake claude)'; exit 0 ;;
esac
report() { printf '\033]7501;state=%s\033\134' "$1"; }
printf 'fake claude on %s: type idle, working, blocked, done, error or spawn\n' @S@
report idle
while IFS= read -r line; do
  case $line in
    idle | working | done | error) report "$line" ;;
    blocked) report blocked:kind=permission ;;
    spawn) @F@/@S@/bin/cornercase start claude > /dev/null 2>&1 || echo 'fake claude: spawn failed' ;;
    *) printf 'fake claude: %s\n' "$line" ;;
  esac
done
EOF
}

write_config() {
  side=$1
  claude=/nonexistent/claude
  if [ "$agent" = 1 ]; then
    claude=$folder/$side/agent/claude
  fi
  {
    printf '{\n  "worktrees_dir": "%s/%s/worktrees",\n' "$folder" "$side"
    if [ "$side" = here ] && [ "$agent" = 1 ]; then
      printf '  "agents_section": true,\n'
    fi
    printf '  "agent_commands": {\n    "claude": "%s"' "$claude"
    for name_of in $agents; do
      [ "$name_of" = claude ] || printf ',\n    "%s": "/nonexistent/%s"' "$name_of" "$name_of"
    done
    printf '\n  }\n}\n'
  } > "$folder/$side/config.json"
}

make_project() {
  git_clean() {
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git "$@"
  }
  git_clean init -q -b main "$1"
  git_clean -C "$1" -c user.name=machine-bench -c user.email=machine-bench@localhost commit -q --allow-empty -m bench
}

make_folders() {
  mkdir "$folder/bin" "$folder/ssh" "$folder/window" "$folder/here" "$folder/there"
  : > "$folder/ssh/calls.log"
  mkdir "$folder/here/bin" "$folder/here/claude" "$folder/here/worktrees"
  mkdir "$folder/there/bin" "$folder/there/claude" "$folder/there/worktrees" "$folder/there/home"
  cp -- "$here_src" "$folder/here/bin/cornercase"
  cp -- "$there_src" "$folder/there/bin/cornercase"
  chmod 700 "$folder/here/bin/cornercase" "$folder/there/bin/cornercase"
}

make_files() {
  make_project "$folder/here/here-proj"
  make_project "$folder/there/home/there-proj"
  printf '[user]\n\tname = machine-bench\n\temail = machine-bench@localhost\n[init]\n\tdefaultBranch = main\n' \
    > "$folder/there/home/.gitconfig"
  write_config here
  write_config there
  write_commands
  write_env
  write_ssh
  write_forward
  if [ "$log" = 1 ]; then
    write_logged
  fi
  if [ "$agent" = 1 ]; then
    write_agent here
    write_agent there
  fi
  chmod 600 "$folder/here/config.json" "$folder/there/config.json"
}

run_start() {
  "$folder/bin/$1" start-server > "$folder/$1/start.log" 2>&1 < /dev/null || {
    cat "$folder/$1/start.log" >&2
    fail "cornercase start-server failed on $1"
  }
}

open_project() {
  id=$("$folder/bin/$1" open "$2") || fail "cornercase open failed on $1"
  case $id in
    '' | *[!0-9]*) fail "cornercase open printed '$id', not a project id" ;;
  esac
}

pane_with() {
  awk -F '\t' -v want="$1" '
    NR == 1 { for (i = 1; i <= NF; i++) col[$i] = i; next }
    $(col["agent"]) == "claude" && $(col["parent"]) == want { print $(col["pane"]); exit }'
}

wait_pane() {
  n=0
  while [ "$n" -lt 30 ]; do
    found=$("$folder/bin/there" status --panes 2> /dev/null | pane_with "$1")
    if [ -n "$found" ]; then
      printf '%s' "$found"
      return 0
    fi
    n=$((n + 1))
    sleep 1
  done
  return 1
}

start_agent() {
  step "starting its agent"
  "$folder/bin/there" start claude --project "$id" > /dev/null || fail "cornercase start claude failed on there"
  parent_pane=$(wait_pane -) || fail "no claude pane with no parent showed up on there in 30 s"
  "$folder/bin/there" send --pane "$parent_pane" --enter spawn > /dev/null 2>&1 || true
  child_pane=$(wait_pane "$parent_pane") || fail "no child of pane $parent_pane showed up on there in 30 s"
}

machine_field() {
  sed -n "s/.*\"$1\": *\"\\([^\"]*\\)\".*/\\1/p" | head -n 1
}

link_there() {
  step "linking $name"
  "$folder/bin/here" machines add "$name" "$dest" --icon server --json > /dev/null ||
    fail "cornercase machines add failed on here"
  n=0
  while [ "$n" -lt 30 ]; do
    json=$("$folder/bin/here" machines --json 2> /dev/null || true)
    state=$(printf '%s\n' "$json" | machine_field state)
    case $state in
      linked | older) return 0 ;;
      unreachable) fail "$name is unreachable: $(printf '%s\n' "$json" | machine_field error)" ;;
    esac
    n=$((n + 1))
    sleep 1
  done
  fail "$name was still '$state' after 30 s"
}

socket_holders() {
  command -v lsof > /dev/null 2>&1 || return 0
  for s in here there; do
    [ -e "$folder/$s/server.sock" ] || continue
    lsof -t "$folder/$s/server.sock" 2> /dev/null | grep -vx "$$" | tr '\n' ' ' || true
  done
}

stop_servers() {
  for s in here there; do
    if [ -e "$folder/$s/server.sock" ]; then
      "$folder/bin/$s" kill-server --yes > /dev/null 2>&1 || say "kill-server failed on $s"
    fi
  done
}

up_exit() {
  trap - EXIT
  if [ "$finished" = 0 ]; then
    stop_servers
    say "up failed at $stage; its servers are stopped and $folder is kept for its logs ($folder/here/server.log, $folder/there/server.log, $folder/<side>/start.log): $repo/tools/$me down $folder"
  fi
}

summary() {
  printf 'bench %s\n' "$folder"
  printf '  here   cornercase %s  %s/here/server.sock  project %s/here/here-proj\n' "$here_ver" "$folder" "$folder"
  printf '  there  cornercase %s  %s/there/server.sock  machine %s (%s): %s · %s\n' \
    "$there_ver" "$folder" "$name" "$dest" "$state" "$there_ver"
  if [ "$agent" = 1 ]; then
    printf '  agent  claude on there: pane %s, its child pane %s\n' "$parent_pane" "$child_pane"
  fi
  if [ "$log" = 1 ]; then
    printf '  log    %s/commands.log\n' "$folder"
  fi
  printf 'every command      . %s/env   then here … and there …\n' "$folder"
  printf 'the window         . %s/env; cd %s/here/here-proj; "%s/target/debug/cornercase-gpui" > %s/window.log 2>&1 & echo $! > %s/window.pid\n' \
    "$folder" "$folder" "$repo" "$folder" "$folder"
  printf 'forward a port     the stand-in forwards -N -L and -R to this computer'"'"'s loopback (python3): a port that is taken here lands on another local port, and a -R on port 0 says which port it got, as ssh does\n'
  printf 'cut the link       touch %s/ssh/%s.held; here machines retry %s   (rm it and retry to bring it back; on Linux the open link drops only when its cornercase proxy ends, by pid; .slow holds each ssh N s; each name, %s and %s, has its own files)\n' \
    "$folder" "$dest" "$name" "$dest" "$second"
  printf 'the window there   ⇧⌘K, type %s (target/manual-check text PID %s, then key PID return), or start it with --remote %s; This Mac in that dialog brings it back\n' \
    "$second" "$second" "$second"
  printf 'vps elsewhere      here machines edit %s --destination %s points it at the same server by its other name (--destination %s puts it back)\n' \
    "$name" "$second" "$dest"
  printf 'take it down       kill "%s(cat %s/window.pid)"; %s/tools/%s down %s\n' '$' "$folder" "$repo" "$me" "$folder"
}

up() {
  valid_path "$folder_arg"
  release "${here_arg:-cornercase}"
  here_src=$release_path
  here_ver=$release_ver
  release "${there_arg:-$here_src}"
  there_src=$release_path
  there_ver=$release_ver
  fresh_folder
  step "making $folder"
  mkdir "$folder"
  printf 'machine-bench 1 %s\n' "$folder" > "$folder/.machine-bench"
  finished=0
  trap up_exit EXIT
  trap 'exit 1' INT TERM HUP
  make_folders
  make_files
  step "starting there"
  run_start there
  open_project there "$folder/there/home/there-proj"
  if [ "$agent" = 1 ]; then
    start_agent
  fi
  step "starting here"
  run_start here
  open_project here "$folder/here/here-proj"
  link_there
  finished=1
  summary
}

parse_up() {
  while [ $# -gt 0 ]; do
    case $1 in
      --here | --there)
        [ $# -ge 2 ] && [ -n "$2" ] || usage_fail "$1 needs a value"
        if [ "$1" = --here ]; then here_arg=$2; else there_arg=$2; fi
        shift 2 ;;
      --agent) agent=1; shift ;;
      --log) log=1; shift ;;
      -*) usage_fail "unknown option $1" ;;
      *)
        [ -z "$folder_arg" ] || usage_fail "a second folder: $1"
        folder_arg=$1
        shift ;;
    esac
  done
  [ -n "$folder_arg" ] || usage_fail "up needs a folder"
}

scan() {
  ps -A -o pid= -o ppid= -o args= | MB_MODE=$1 MB_FOLDER=$folder MB_PIDS=${2-} awk '
    function benchword(s) { return length(s) > 0 && substr(s, 1, length(F)) == F }
    function base(s,   n, parts) { n = split(s, parts, "/"); return parts[n] }
    BEGIN { F = ENVIRON["MB_FOLDER"] "/"; mode = ENVIRON["MB_MODE"] }
    {
      pid[NR] = $1; ppid[NR] = $2
      line = $0
      sub(/^[ \t]*[0-9]+[ \t]+[0-9]+[ \t]+/, "", line)
      args[NR] = line
      if (benchword($3) || benchword($4) || benchword($5)) { direct[$1] = 1; mark[$1] = 1 }
      if (base($3) == "cornercase-gpui") window[$1] = 1
    }
    END {
      changed = 1
      while (changed) {
        changed = 0
        for (i = 1; i <= NR; i++)
          if (!(pid[i] in mark) && (ppid[i] in mark)) { mark[pid[i]] = 1; changed = 1 }
      }
      n = split(ENVIRON["MB_PIDS"], old, " ")
      for (k = 1; k <= n; k++) keep[old[k]] = 1
      for (i = 1; i <= NR; i++) {
        if (mode == "pids" && (pid[i] in mark)) print pid[i]
        if (mode == "left" && ((pid[i] in mark) || (pid[i] in keep))) print pid[i], args[i]
        if (mode == "windows" && (pid[i] in direct) && (ppid[i] in window)) print ppid[i]
      }
    }'
}

window_pid() {
  pidfile=$folder/window.pid
  [ -f "$pidfile" ] || return 0
  wp=$(cat "$pidfile")
  case $wp in '' | *[!0-9]*) return 0 ;; esac
  kill -0 "$wp" 2> /dev/null || return 0
  first=$(ps -o args= -p "$wp" 2> /dev/null | awk '{ print $1 }')
  case ${first##*/} in
    cornercase-gpui) printf '%s\n' "$wp" ;;
  esac
}

windows_on_bench() {
  { window_pid; scan windows; } | sort -u | tr '\n' ' ' | sed 's/ *$//'
}

refuse_inside() {
  [ -n "${CORNERCASE_PANE-}" ] || return 0
  case ${CORNERCASE_SOCKET-} in
    "$folder"/*) refuse "this shell runs inside the bench's server: run down from outside it" ;;
  esac
}

refuse_window() {
  found=$(windows_on_bench)
  [ -z "$found" ] || refuse "a window is still on the bench (pid $found): kill $found first, then run down again"
}

wait_gone() {
  n=0
  while [ "$n" -lt 10 ]; do
    [ -n "$(scan left "$1")" ] || return 0
    n=$((n + 1))
    sleep 1
  done
  [ -z "$(scan left "$1")" ]
}

down() {
  valid_path "$folder_arg"
  marked
  refuse_inside
  refuse_window
  for d in "$dest" "$second"; do
    : > "$folder/ssh/$d.held"
    rm -f "$folder/ssh/$d.slow"
  done
  snapshot=$(scan pids | tr '\n' ' ')$(socket_holders)
  stop_servers
  if ! wait_gone "$snapshot"; then
    printf 'still running:\n' >&2
    scan left "$snapshot" >&2
    fail "end them by pid, then run down again; $folder is kept"
  fi
  rm -rf -- "$folder"
  printf 'removed %s\n' "$folder"
}

side_report() {
  version=$("$folder/bin/$1" --version 2> /dev/null || printf unknown)
  running=stopped
  if [ -e "$folder/$1/server.sock" ] && "$folder/bin/$1" status > /dev/null 2>&1; then
    running=running
  fi
  printf '  %-6s %s, server %s\n' "$1" "$version" "$running"
}

machine_line() {
  error=$(printf '%s\n' "$json" | machine_field error)
  printf '  %s %s%s\n' "$name" "$(printf '%s\n' "$json" | machine_field state)" "${error:+: $error}"
}

forwarders() {
  ps -A -o pid= -o args= | MB_FORWARD="$folder/bin/forward" awk '
    { at = index($0, ENVIRON["MB_FORWARD"]) }
    at > 0 { print $1, substr($0, at + length(ENVIRON["MB_FORWARD"]) + 1) }'
}

forwarders_report() {
  found=$(forwarders)
  if [ -z "$found" ]; then
    printf '  forwards none\n'
    return 0
  fi
  printf '%s\n' "$found" | while read -r fpid fspecs; do
    printf '  forward pid %s %s\n' "$fpid" "$fspecs"
  done
}

list_file() {
  if [ -e "$2" ]; then
    printf '  %s %s is there\n' "$1" "$3"
  else
    printf '  %s %s is not there\n' "$1" "$3"
  fi
}

lists_report() {
  list_file "cornercase's list on here:" "$folder/here/tunnels.json" tunnels.json
  list_file "cornercase's list on there:" "$folder/there/tunnels.json" tunnels.json
  list_file "the window's own list:" "$folder/window/remembered-tunnels.json" remembered-tunnels.json
}

status() {
  valid_path "$folder_arg"
  marked
  printf 'bench %s\n' "$folder"
  side_report here
  side_report there
  if [ -e "$folder/here/server.sock" ] && "$folder/bin/here" status > /dev/null 2>&1; then
    json=$("$folder/bin/here" machines --json 2> /dev/null || true)
    machine_line
  fi
  for d in "$dest" "$second"; do
    for f in held slow missing; do
      if [ -e "$folder/ssh/$d.$f" ]; then
        printf '  ssh/%s.%s is there\n' "$d" "$f"
      fi
    done
  done
  printf '  window %s\n' "$(windows_on_bench | grep . || printf none)"
  forwarders_report
  lists_report
  printf 'processes:\n'
  scan left | grep . || printf '  none\n'
}

folder_arg=
here_arg=
there_arg=

case $(uname -s) in
  MINGW* | MSYS* | CYGWIN*) refuse "the bench runs on macOS and Linux (#70)" ;;
esac

[ $# -ge 1 ] || usage_fail "a word is needed"
word=$1
shift
case $word in
  -h | --help) usage; exit 0 ;;
  up) parse_up "$@"; up ;;
  status | down)
    [ $# -eq 1 ] || usage_fail "$word takes one folder"
    case $1 in -*) usage_fail "unknown option $1" ;; esac
    folder_arg=$1
    "$word" ;;
  *) usage_fail "unknown word $word" ;;
esac
