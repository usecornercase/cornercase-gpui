#!/bin/sh
set -u

repo=$(cd "$(dirname "$0")/.." && pwd -P)
script=$repo/tools/linux-check.sh
sh_path=$(command -v "${SH:-sh}")
total=0
failed=0

t=$(mktemp -d /tmp/lct.XXXXXX) || exit 1
t=$(cd "$t" && pwd -P)
mkdir -p "$t/bin" "$t/sys" "$t/control" "$t/a/tools" "$t/b/tools"
: > "$t/calls"

cleanup() {
  case $t in
    /tmp/lct.* | /private/tmp/lct.*) rm -rf "$t" ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' INT TERM HUP

for tool in dirname sed cat; do
  ln -s "$(command -v "$tool")" "$t/sys/$tool"
done
cp "$script" "$t/a/tools/linux-check.sh"
cp "$script" "$t/b/tools/linux-check.sh"

cat > "$t/bin/docker" <<'STUB'
#!/bin/sh
t=@T@
printf '%s\n' "$*" >> "$t/calls"
case ${1-} in
  version)
    if [ -e "$t/control/no-daemon" ]; then
      echo 'Cannot connect to the Docker daemon at unix:///x/docker.sock. Is the docker daemon running?' >&2
      exit 1
    fi
    echo 29.1.2 ;;
  image)
    if [ -e "$t/control/no-image" ]; then
      echo 'Error response from daemon: No such image: cornercase-gpui-linux:latest' >&2
      exit 1
    fi
    echo 2026-11-04T10:29:00Z ;;
  run)
    status=0
    [ -e "$t/control/status" ] && status=$(cat "$t/control/status")
    exit "$status" ;;
esac
STUB
sed "s|@T@|$t|g" "$t/bin/docker" > "$t/bin/docker.new"
mv "$t/bin/docker.new" "$t/bin/docker"
chmod 755 "$t/bin/docker"

check() {
  msg=$1
  shift
  "$@" || {
    printf '%s\n' "$msg" > "$t/why"
    return 1
  }
}

reset() {
  : > "$t/calls"
  rm -rf "$t/control"
  mkdir -p "$t/control"
}

run_test() {
  total=$((total + 1))
  rm -f "$t/why"
  reset
  if "$1"; then
    printf 'ok - %s\n' "$1"
  else
    failed=$((failed + 1))
    printf 'not ok - %s: %s\n' "$1" "$(cat "$t/why" 2> /dev/null)"
  fi
}

runl() {
  "$sh_path" "${checkout:-$script}" "$@" > "$t/out" 2> "$t/err"
  rc=$?
}

with_path() {
  p=$1
  shift
  PATH=$p "$@"
}

has() {
  grep -F -e "$2" "$1" > /dev/null
}

nohas() {
  ! has "$1" "$2"
}

run_line() {
  grep '^run ' "$t/calls"
}

without_docker_on_the_path_it_exits_3_and_names_it() {
  PATH=$t/sys runl
  check "status $rc" [ "$rc" = 3 ] &&
    check "no name" has "$t/err" 'docker, and it is not on the PATH' &&
    check "something ran" [ ! -s "$t/calls" ]
}

a_daemon_that_does_not_answer_exits_3_and_asks_nothing_more() {
  : > "$t/control/no-daemon"
  PATH=$t/bin:$t/sys runl
  check "status $rc" [ "$rc" = 3 ] &&
    check "no quote" has "$t/err" 'Cannot connect to the Docker daemon' &&
    check "image asked" nohas "$t/calls" 'image' &&
    check "run called" nohas "$t/calls" 'run '
}

a_missing_image_exits_3_names_it_and_points_to_design_linux() {
  : > "$t/control/no-image"
  PATH=$t/bin:$t/sys runl
  check "status $rc" [ "$rc" = 3 ] &&
    check "no image" has "$t/err" 'the image cornercase-gpui-linux' &&
    check "no pointer" has "$t/err" 'design/linux.md says how to build it' &&
    check "run called" nohas "$t/calls" 'run '
}

it_runs_clippy_and_tests_in_a_container_of_its_own() {
  PATH=$t/bin:$t/sys runl
  line=$(run_line)
  check "status $rc" [ "$rc" = 0 ] &&
    check "no --rm" has "$t/calls" 'run --rm --init --pull never --name cornercase-gpui-linux-check-' &&
    check "wrong command: $line" has "$t/calls" 'cornercase-gpui-linux bash -lc cargo clippy --all-targets --all-features --locked -- -D warnings && cargo test --locked'
}

it_mounts_this_checkout_at_its_own_path_and_works_there() {
  checkout=$t/a/tools/linux-check.sh
  PATH=$t/bin:$t/sys runl
  unset checkout
  check "status $rc" [ "$rc" = 0 ] &&
    check "mount: $(run_line)" has "$t/calls" "-v $t/a:$t/a -w $t/a "
}

two_checkouts_get_two_paths() {
  checkout=$t/a/tools/linux-check.sh
  PATH=$t/bin:$t/sys runl
  checkout=$t/b/tools/linux-check.sh
  PATH=$t/bin:$t/sys runl
  unset checkout
  check "a: $(run_line)" has "$t/calls" "-v $t/a:$t/a" &&
    check "b: $(run_line)" has "$t/calls" "-v $t/b:$t/b" &&
    check "a mounted for b" nohas "$t/calls" "-v $t/b:$t/a"
}

the_build_goes_to_the_shared_volume_with_cargo_target_dir() {
  PATH=$t/bin:$t/sys runl
  check "volume: $(run_line)" has "$t/calls" '-v cornercase-gpui-linux-target:/target -e CARGO_TARGET_DIR=/target'
}

minus_j_reaches_both_cargo_commands() {
  PATH=$t/bin:$t/sys runl -j 4
  check "status $rc" [ "$rc" = 0 ] &&
    check "line: $(run_line)" has "$t/calls" 'cargo clippy --all-targets --all-features --locked -j 4 -- -D warnings && cargo test --locked -j 4'
}

a_wrong_word_exits_2_and_runs_nothing() {
  for words in --nope -j "-j x" "-j 0" "-j 4 x" "-j 04" x; do
    # shellcheck disable=SC2086
    PATH=$t/bin:$t/sys runl $words
    check "'$words' gave $rc" [ "$rc" = 2 ] || return 1
    check "'$words' printed no usage" has "$t/err" 'usage: tools/linux-check.sh' || return 1
    check "'$words' ran something" [ ! -s "$t/calls" ] || return 1
  done
}

help_prints_the_usage_and_exits_0() {
  PATH=$t/bin:$t/sys runl --help
  check "status $rc" [ "$rc" = 0 ] &&
    check "no usage" has "$t/out" 'usage: tools/linux-check.sh [-j N]' &&
    check "no statuses" has "$t/out" '3 something it needs is not there' &&
    check "ran something" [ ! -s "$t/calls" ]
}

the_containers_status_comes_back() {
  for status in 0 101 125; do
    printf '%s' "$status" > "$t/control/status"
    PATH=$t/bin:$t/sys runl
    check "status $rc, not $status" [ "$rc" = "$status" ] || return 1
  done
}

it_asks_docker_only_its_version_the_image_and_one_run() {
  PATH=$t/bin:$t/sys runl
  words=$(sed 's/ .*//' "$t/calls" | tr '\n' ' ')
  check "calls: $(cat "$t/calls")" [ "$words" = 'version image run ' ]
}

run_test without_docker_on_the_path_it_exits_3_and_names_it
run_test a_daemon_that_does_not_answer_exits_3_and_asks_nothing_more
run_test a_missing_image_exits_3_names_it_and_points_to_design_linux
run_test it_runs_clippy_and_tests_in_a_container_of_its_own
run_test it_mounts_this_checkout_at_its_own_path_and_works_there
run_test two_checkouts_get_two_paths
run_test the_build_goes_to_the_shared_volume_with_cargo_target_dir
run_test minus_j_reaches_both_cargo_commands
run_test a_wrong_word_exits_2_and_runs_nothing
run_test help_prints_the_usage_and_exits_0
run_test the_containers_status_comes_back
run_test it_asks_docker_only_its_version_the_image_and_one_run

printf '%s tests, %s failed\n' "$total" "$failed"
[ "$failed" = 0 ]
