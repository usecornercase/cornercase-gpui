#!/bin/sh
set -eu
export LC_ALL=C

fail() {
  echo "cornercase-cli.sh: $*" >&2
  exit 1
}

bin=${CORNERCASE_BIN:-cornercase}
case $bin in
  */*) bin=$(cd "$(dirname "$bin")" && pwd)/$(basename "$bin") ;;
esac

version=$("$bin" --version) || fail "could not run $bin --version"
case $version in
  cornercase\ [0-9]*) ;;
  *) fail "$bin --version printed '$version', not 'cornercase <version>'" ;;
esac

commands() {
  help=$("$bin" "$@" --help) || fail "could not run $bin $* --help"
  printf '%s\n' "$help" | sed -n '/^Commands:$/,/^$/s/^  \([a-z][a-z-]*\) .*/\1/p' | grep -vx help || true
}

section() {
  printf '================================================================ cornercase %s\n' "$*"
  "$bin" "$@" --help
  echo
}

walk() {
  list=$(commands "$@") || exit 1
  for command in $list; do
    section "$@" "$command"
    walk "$@" "$command"
  done
}

[ -n "$(commands)" ] || fail "$bin --help lists no commands"

cd "$(dirname "$0")/.."
out=docs/cornercase-cli.txt
new=$out.new
trap 'rm -f "$new"' EXIT
{
  printf "%s: the help of every command and subcommand, in the order of \`cornercase --help\`.\n" "$version"
  printf 'Made by tools/cornercase-cli.sh, which runs CORNERCASE_BIN, else cornercase on the PATH.\n\n'
  walk
} > "$new"
mv "$new" "$out"
