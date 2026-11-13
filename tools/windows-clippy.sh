#!/bin/sh
set -eu

name=windows-clippy.sh
target=x86_64-pc-windows-msvc

say() {
  printf '%s: %s\n' "$name" "$*" >&2
}

usage_text() {
  cat <<'USAGE'
usage: tools/windows-clippy.sh [-j N]

Runs cargo xwin clippy on the Windows code (target/win, all targets and features, -D warnings).
It looks for LLVM's clang-cl, llvm-lib and llvm-rc in one folder, in this order:
  LLVM_BIN (when set, and then nowhere else), brew --prefix llvm@20, brew --prefix llvm, the PATH.
It also needs cargo xwin and the rust target x86_64-pc-windows-msvc.
Exit status: 0 clean; 2 a wrong word; 3 something it needs is not there and nothing was run;
anything else is cargo's (101 when clippy reports something or the build fails).
See design/windows.md.
USAGE
}

refuse() {
  say "$1"
  usage_text >&2
  exit 2
}

jobs=
case ${1-} in
  -h | --help)
    usage_text
    exit 0
    ;;
  '') ;;
  -j)
    [ $# -ge 2 ] || refuse "-j takes one whole number"
    case $2 in
      '' | *[!0-9]* | 0 | 0[0-9]*) refuse "-j takes a whole number of at least 1, not '$2'" ;;
    esac
    [ $# -le 2 ] || refuse "unknown word '$3'"
    jobs=$2
    ;;
  *) refuse "unknown word '$1'" ;;
esac
[ -z "$jobs" ] && [ $# -gt 0 ] && refuse "unknown word '$1'"

llvm_bin=${LLVM_BIN-}
case $llvm_bin in
  '' | /*) ;;
  *) llvm_bin=$(pwd -P)/$llvm_bin ;;
esac

cd "$(dirname "$0")/.."

lacks() {
  missing=
  if [ ! -d "$1" ]; then
    missing=nodir
    return 0
  fi
  for program in clang-cl llvm-lib llvm-rc; do
    if [ ! -f "$1/$program" ] || [ ! -x "$1/$program" ]; then
      missing="$missing $program"
    fi
  done
  missing=${missing# }
}

describe_lack() {
  if [ "$missing" = nodir ]; then
    printf '%s does not exist' "$1"
  else
    printf '%s has no %s' "$1" "$(printf '%s' "$missing" | sed 's/ /, /g')"
  fi
}

found=
how=
report=
problems=

try_brew() {
  formula=$1
  if ! command -v brew > /dev/null 2>&1; then
    report="$report
    brew --prefix $formula: brew is not on the PATH"
    return 0
  fi
  answer=$(brew --prefix "$formula" 2> /dev/null) || answer=
  if [ -z "$answer" ]; then
    report="$report
    brew --prefix $formula: brew has no answer"
    return 0
  fi
  lacks "$answer/bin"
  if [ -z "$missing" ]; then
    found=$answer/bin
    how="brew --prefix $formula"
  else
    report="$report
    brew --prefix $formula: $(describe_lack "$answer/bin")"
  fi
}

try_path() {
  absent=
  for program in clang-cl llvm-lib llvm-rc; do
    command -v "$program" > /dev/null 2>&1 || absent="$absent $program"
  done
  if [ -z "$absent" ]; then
    found=path
    how="the PATH"
  else
    report="$report
    the PATH: no$(printf '%s' "$absent" | sed 's/ /, /g; s/^,//')"
  fi
}

search() {
  if [ -n "$llvm_bin" ]; then
    lacks "$llvm_bin"
    if [ -z "$missing" ]; then
      found=$llvm_bin
      how=LLVM_BIN
    else
      report="
    LLVM_BIN: $(describe_lack "$llvm_bin"); it is set, so nowhere else was looked at"
    fi
    return 0
  fi
  report="
    LLVM_BIN: not set"
  try_brew llvm@20
  [ -n "$found" ] || try_brew llvm
  [ -n "$found" ] || try_path
}

search

add_problem() {
  problems="${problems:+$problems
}$1"
}

if [ -z "$found" ]; then
  add_problem "  LLVM's clang-cl, llvm-lib and llvm-rc in one folder, looked for in:$report"
fi

xwin_problem=
if ! command -v cargo > /dev/null 2>&1; then
  xwin_problem="cargo is not on the PATH"
elif ! cargo xwin --version > /dev/null 2>&1; then
  xwin_problem="cargo has no xwin command"
fi
if [ -n "$xwin_problem" ]; then
  add_problem "  cargo xwin: $xwin_problem"
fi

target_problem=
if ! command -v rustc > /dev/null 2>&1; then
  target_problem="rustc is not on the PATH"
else
  libdir=$(rustc --print target-libdir --target "$target" 2> /dev/null) || libdir=
  if [ -z "$libdir" ]; then
    target_problem="rustc gave no folder for it"
  elif [ ! -d "$libdir" ]; then
    target_problem="not installed for this checkout's toolchain (no $libdir)"
  fi
fi
if [ -n "$target_problem" ]; then
  add_problem "  the rust target $target: $target_problem"
fi

if [ -n "$problems" ]; then
  say "nothing was run: the Windows clippy needs what is missing below"
  printf '%s\n' "$problems" >&2
  exit 3
fi

if [ "$found" = path ]; then
  say "clang-cl, llvm-lib and llvm-rc from the PATH"
else
  say "clang-cl, llvm-lib and llvm-rc from $found ($how)"
  PATH=$found:$PATH
  export PATH
fi

set --
if [ -n "$jobs" ]; then
  set -- -j "$jobs"
fi
exec cargo xwin clippy --target "$target" --all-targets --all-features --locked --target-dir target/win "$@" \
  -- -D warnings
