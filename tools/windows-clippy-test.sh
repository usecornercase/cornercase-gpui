#!/bin/sh
set -u

repo=$(cd "$(dirname "$0")/.." && pwd -P)
script=$repo/tools/windows-clippy.sh
sh_path=$(command -v "${SH:-sh}")
total=0
failed=0

t=$(mktemp -d /tmp/wct.XXXXXX) || exit 1
t=$(cd "$t" && pwd -P)
mkdir -p "$t/bin" "$t/sys" "$t/control" "$t/start"
: > "$t/calls"

cleanup() {
  case $t in
    /tmp/wct.* | /private/tmp/wct.*) rm -rf "$t" ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' INT TERM HUP

for tool in dirname sed cat; do
  ln -s "$(command -v "$tool")" "$t/sys/$tool"
done
unset LLVM_BIN

make_llvm() {
  mkdir -p "$1/bin"
  for program in clang-cl llvm-lib llvm-rc; do
    printf '#!/bin/sh\nexit 0\n' > "$1/bin/$program"
    chmod 755 "$1/bin/$program"
  done
}

cat > "$t/bin/cargo" <<'STUB'
#!/bin/sh
t=@T@
printf 'pwd=%s|path=%s|args=%s\n' "$(pwd -P)" "$PATH" "$*" >> "$t/calls"
case ${1-} in
  xwin)
    case ${2-} in
      --version)
        if [ -e "$t/control/no-xwin" ]; then
          echo 'error: no such command: `xwin`' >&2
          exit 101
        fi
        echo 'cargo-xwin 0.23.1'
        exit 0 ;;
      clippy)
        status=0
        [ -e "$t/control/status" ] && status=$(cat "$t/control/status")
        exit "$status" ;;
    esac ;;
esac
exit 101
STUB
cat > "$t/bin/rustc" <<'STUB'
#!/bin/sh
echo "rustc $*" >> @T@/calls
[ -e @T@/control/no-rustc-answer ] && exit 1
echo @T@/rustlib/x86_64-pc-windows-msvc/lib
STUB
cat > "$t/bin/brew" <<'STUB'
#!/bin/sh
t=@T@
echo "brew $*" >> "$t/calls"
if [ "${1-}" = --prefix ] && [ -e "$t/control/$2" ]; then
  cat "$t/control/$2"
  exit 0
fi
echo "Error: No available formula with the name \"$2\"" >&2
exit 1
STUB
for stub in cargo rustc brew; do
  sed "s|@T@|$t|g" "$t/bin/$stub" > "$t/bin/$stub.new"
  mv "$t/bin/$stub.new" "$t/bin/$stub"
  chmod 755 "$t/bin/$stub"
done
mkdir -p "$t/rustlib/x86_64-pc-windows-msvc/lib"
make_llvm "$t/brew20"
make_llvm "$t/brewllvm"
make_llvm "$t/mine"
make_llvm "$t/onpath"

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
  reset
  if "$1"; then
    printf 'ok - %s\n' "$1"
  else
    failed=$((failed + 1))
    printf 'not ok - %s: %s\n' "$1" "$(cat "$t/why" 2> /dev/null)"
  fi
}

reset() {
  : > "$t/calls"
  rm -rf "$t/control"
  mkdir -p "$t/control"
  rm -rf "$t/lacking" "$t/mine"
  make_llvm "$t/mine"
  mkdir -p "$t/rustlib/x86_64-pc-windows-msvc/lib"
  unset LLVM_BIN
  extra_path=
}

runw() {
  (cd "$t/start" && PATH="$t/bin:$t/sys${extra_path:+:$extra_path}" "$sh_path" "$script" "$@") > "$t/out" 2> "$t/err"
  rc=$?
}

has() {
  grep -F -e "$2" "$1" > /dev/null
}

hasnt() {
  ! has "$1" "$2"
}

hasline() {
  grep -F -x -e "$2" "$1" > /dev/null
}

nohasline() {
  ! hasline "$1" "$2"
}

no_blank_line() {
  ! grep -x '' "$t/err" > /dev/null
}

clippy_calls() {
  grep -c 'args=xwin clippy' "$t/calls"
}

brew_answers() {
  printf '%s' "$2" > "$t/control/$1"
}

want_path='x86_64-pc-windows-msvc --all-targets --all-features --locked --target-dir target/win'

llvm_bin_with_the_three_is_used_and_brew_is_not_asked() {
  LLVM_BIN=$t/mine/bin
  export LLVM_BIN
  brew_answers llvm@20 "$t/brew20"
  runw
  check "status $rc" [ "$rc" = 0 ] &&
    check "brew was asked" hasnt "$t/calls" 'brew ' &&
    check "folder not first" has "$t/calls" "path=$t/mine/bin:$t/bin:$t/sys|"
}

a_wrong_llvm_bin_stops_with_3_and_names_what_it_lacks() {
  rm "$t/mine/bin/llvm-rc"
  LLVM_BIN=$t/mine/bin
  export LLVM_BIN
  brew_answers llvm@20 "$t/brew20"
  runw
  check "status $rc" [ "$rc" = 3 ] &&
    check "no name" has "$t/err" 'LLVM_BIN' &&
    check "no lack" has "$t/err" 'has no llvm-rc' &&
    check "brew asked" hasnt "$t/calls" 'brew ' &&
    check "clippy ran" [ "$(clippy_calls)" = 0 ]
}

a_missing_llvm_bin_folder_stops_with_3() {
  LLVM_BIN=$t/nowhere
  export LLVM_BIN
  brew_answers llvm@20 "$t/brew20"
  runw
  check "status $rc" [ "$rc" = 3 ] &&
    check "no answer" has "$t/err" "$t/nowhere does not exist" &&
    check "clippy ran" [ "$(clippy_calls)" = 0 ]
}

a_relative_llvm_bin_is_read_from_the_callers_folder() {
  mkdir -p "$t/start/rel"
  make_llvm "$t/start/rel/llvm"
  LLVM_BIN=rel/llvm/bin
  export LLVM_BIN
  runw
  check "status $rc: $(cat "$t/err")" [ "$rc" = 0 ] &&
    check "folder not first" has "$t/calls" "path=$t/start/rel/llvm/bin:$t/bin:$t/sys|"
  r=$?
  rm -rf "$t/start/rel"
  return $r
}

brew_llvm20_comes_before_brew_llvm_and_the_path() {
  brew_answers llvm@20 "$t/brew20"
  brew_answers llvm "$t/brewllvm"
  extra_path=$t/onpath/bin
  runw
  check "status $rc" [ "$rc" = 0 ] &&
    check "wrong folder" has "$t/calls" "path=$t/brew20/bin:" &&
    check "llvm asked" nohasline "$t/calls" 'brew --prefix llvm'
}

brew_llvm_comes_before_the_path() {
  brew_answers llvm@20 "$t/missing20"
  brew_answers llvm "$t/brewllvm"
  extra_path=$t/onpath/bin
  runw
  check "status $rc" [ "$rc" = 0 ] &&
    check "wrong folder" has "$t/calls" "path=$t/brewllvm/bin:"
}

a_brew_answer_whose_folder_lacks_a_program_is_not_a_find() {
  make_llvm "$t/lacking"
  rm "$t/lacking/bin/llvm-lib"
  brew_answers llvm@20 "$t/lacking"
  brew_answers llvm "$t/lacking"
  runw
  check "status $rc" [ "$rc" = 3 ] &&
    check "no lack named" has "$t/err" "$t/lacking/bin has no llvm-lib" &&
    check "clippy ran" [ "$(clippy_calls)" = 0 ]
}

a_brew_answer_whose_folder_does_not_exist_is_not_a_find() {
  brew_answers llvm@20 "$t/missing20"
  brew_answers llvm "$t/missing"
  runw
  check "status $rc" [ "$rc" = 3 ] &&
    check "no answer" has "$t/err" "$t/missing/bin does not exist" &&
    check "clippy ran" [ "$(clippy_calls)" = 0 ]
}

a_failing_brew_answer_is_not_a_find() {
  extra_path=$t/onpath/bin
  runw
  check "status $rc: $(cat "$t/err")" [ "$rc" = 0 ] &&
    check "no brew call" has "$t/calls" 'brew --prefix llvm@20' &&
    check "path not used as is" has "$t/calls" "path=$t/bin:$t/sys:$t/onpath/bin|"
}

without_brew_the_path_is_used_as_it_is() {
  mv "$t/bin/brew" "$t/brew.off"
  extra_path=$t/onpath/bin
  runw
  mv "$t/brew.off" "$t/bin/brew"
  check "status $rc" [ "$rc" = 0 ] &&
    check "path changed" has "$t/calls" "path=$t/bin:$t/sys:$t/onpath/bin|args=xwin clippy" &&
    check "not said" has "$t/err" 'from the PATH'
}

a_program_that_is_not_executable_is_not_a_find() {
  make_llvm "$t/lacking"
  chmod 644 "$t/lacking/bin/llvm-rc"
  brew_answers llvm@20 "$t/lacking"
  runw
  check "status $rc" [ "$rc" = 3 ] &&
    check "no lack named" has "$t/err" "$t/lacking/bin has no llvm-rc"
}

nothing_found_names_the_three_programs_and_every_place_with_3() {
  runw
  check "status $rc" [ "$rc" = 3 ] &&
    check "no clang-cl" has "$t/err" 'clang-cl, llvm-lib and llvm-rc' &&
    check "no LLVM_BIN" has "$t/err" 'LLVM_BIN: not set' &&
    check "no llvm@20" has "$t/err" 'brew --prefix llvm@20: brew has no answer' &&
    check "no llvm" has "$t/err" 'brew --prefix llvm: brew has no answer' &&
    check "no PATH" has "$t/err" 'the PATH: no clang-cl, llvm-lib, llvm-rc' &&
    check "clippy ran" [ "$(clippy_calls)" = 0 ]
}

a_missing_cargo_xwin_is_3_not_cargos_101() {
  : > "$t/control/no-xwin"
  extra_path=$t/onpath/bin
  runw
  check "status $rc" [ "$rc" = 3 ] &&
    check "no xwin line" has "$t/err" 'cargo xwin: cargo has no xwin command' &&
    check "blank line" no_blank_line &&
    check "clippy ran" [ "$(clippy_calls)" = 0 ]
}

a_missing_target_is_3() {
  rm -rf "$t/rustlib"
  extra_path=$t/onpath/bin
  runw
  mkdir -p "$t/rustlib/x86_64-pc-windows-msvc/lib"
  check "status $rc" [ "$rc" = 3 ] &&
    check "no target line" has "$t/err" 'the rust target x86_64-pc-windows-msvc: not installed for this checkout'"'"'s toolchain (no '"$t"'/rustlib/x86_64-pc-windows-msvc/lib)' &&
    check "blank line" no_blank_line &&
    check "clippy ran" [ "$(clippy_calls)" = 0 ]
}

everything_missing_is_said_in_one_run() {
  : > "$t/control/no-xwin"
  rm -rf "$t/rustlib"
  runw
  mkdir -p "$t/rustlib/x86_64-pc-windows-msvc/lib"
  check "status $rc" [ "$rc" = 3 ] &&
    check "no LLVM" has "$t/err" 'LLVM_BIN: not set' &&
    check "no xwin" has "$t/err" 'cargo xwin:' &&
    check "no target" has "$t/err" 'the rust target'
}

cargo_runs_the_clippy_line_from_the_checkouts_root() {
  LLVM_BIN=$t/mine/bin
  export LLVM_BIN
  runw
  check "status $rc" [ "$rc" = 0 ] &&
    check "wrong line or folder" has "$t/calls" "pwd=$repo|path=$t/mine/bin:$t/bin:$t/sys|args=xwin clippy --target $want_path -- -D warnings"
}

the_found_folder_is_first_on_cargos_path() {
  brew_answers llvm@20 "$t/brew20"
  runw
  check "status $rc" [ "$rc" = 0 ] &&
    check "path" has "$t/calls" "args=xwin clippy"  &&
    check "folder not first" has "$t/calls" "pwd=$repo|path=$t/brew20/bin:$t/bin:$t/sys|args=xwin clippy"
}

minus_j_reaches_cargo_before_the_double_dash() {
  brew_answers llvm@20 "$t/brew20"
  runw -j 4
  check "status $rc" [ "$rc" = 0 ] &&
    check "no -j" has "$t/calls" "$want_path -j 4 -- -D warnings"
}

a_wrong_word_or_job_count_exits_2_and_runs_nothing() {
  brew_answers llvm@20 "$t/brew20"
  for words in --nope -j "-j x" "-j 0" "-j 4 x" "-j 04" "x" "-j -1"; do
    # shellcheck disable=SC2086
    runw $words
    check "'$words' gave $rc" [ "$rc" = 2 ] || return 1
    check "'$words' printed no usage" has "$t/err" 'usage: tools/windows-clippy.sh' || return 1
    check "'$words' ran something" [ ! -s "$t/calls" ] || return 1
  done
  runw -j 4 stray
  check "stray word not named" has "$t/err" "unknown word 'stray'"
}

help_prints_the_usage_and_exits_0() {
  runw --help
  check "status $rc" [ "$rc" = 0 ] &&
    check "no usage" has "$t/out" 'usage: tools/windows-clippy.sh [-j N]' &&
    check "no statuses" has "$t/out" '3 something it needs is not there' &&
    check "ran something" [ ! -s "$t/calls" ]
}

cargos_status_comes_back() {
  brew_answers llvm@20 "$t/brew20"
  for status in 0 101 1; do
    printf '%s' "$status" > "$t/control/status"
    runw
    check "status $rc, not $status" [ "$rc" = "$status" ] || return 1
  done
}

the_found_line_names_the_folder_and_how_it_was_found() {
  brew_answers llvm@20 "$t/brew20"
  runw
  check "status $rc" [ "$rc" = 0 ] &&
    check "line" has "$t/err" "clang-cl, llvm-lib and llvm-rc from $t/brew20/bin (brew --prefix llvm@20)"
}

run_test llvm_bin_with_the_three_is_used_and_brew_is_not_asked
run_test a_wrong_llvm_bin_stops_with_3_and_names_what_it_lacks
run_test a_missing_llvm_bin_folder_stops_with_3
run_test a_relative_llvm_bin_is_read_from_the_callers_folder
run_test brew_llvm20_comes_before_brew_llvm_and_the_path
run_test brew_llvm_comes_before_the_path
run_test a_brew_answer_whose_folder_lacks_a_program_is_not_a_find
run_test a_brew_answer_whose_folder_does_not_exist_is_not_a_find
run_test a_failing_brew_answer_is_not_a_find
run_test without_brew_the_path_is_used_as_it_is
run_test a_program_that_is_not_executable_is_not_a_find
run_test nothing_found_names_the_three_programs_and_every_place_with_3
run_test a_missing_cargo_xwin_is_3_not_cargos_101
run_test a_missing_target_is_3
run_test everything_missing_is_said_in_one_run
run_test cargo_runs_the_clippy_line_from_the_checkouts_root
run_test the_found_folder_is_first_on_cargos_path
run_test minus_j_reaches_cargo_before_the_double_dash
run_test a_wrong_word_or_job_count_exits_2_and_runs_nothing
run_test help_prints_the_usage_and_exits_0
run_test cargos_status_comes_back
run_test the_found_line_names_the_folder_and_how_it_was_found

printf '%s tests, %s failed\n' "$total" "$failed"
[ "$failed" = 0 ]
