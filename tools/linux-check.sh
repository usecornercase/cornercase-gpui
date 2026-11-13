#!/bin/sh
set -eu

name=linux-check.sh
image=cornercase-gpui-linux
volume=cornercase-gpui-linux-target

say() {
  printf '%s: %s\n' "$name" "$*" >&2
}

usage_text() {
  cat <<'USAGE'
usage: tools/linux-check.sh [-j N]

Runs cargo clippy --all-targets --all-features --locked -- -D warnings and cargo test --locked
for Linux, in a container of its own made from the image cornercase-gpui-linux, with this checkout
mounted at its own path and the build in the volume cornercase-gpui-linux-target.
It looks for docker on the PATH, a daemon that answers and the image, and builds nothing.
Exit status: 0 clean; 2 a wrong word; 3 something it needs is not there and nothing was run;
anything else is the container's (cargo's 101 when clippy or a test fails, docker's 125 to 127).
See design/linux.md.
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
    [ $# -eq 2 ] || refuse "-j takes one whole number"
    case $2 in
      '' | *[!0-9]* | 0 | 0[0-9]*) refuse "-j takes a whole number of at least 1, not '$2'" ;;
    esac
    jobs=" -j $2"
    ;;
  *) refuse "unknown word '$1'" ;;
esac
[ -z "$jobs" ] && [ $# -gt 0 ] && refuse "unknown word '$1'"

missing() {
  say "nothing was run: the Linux check needs $1"
  exit 3
}

repo=$(cd "$(dirname "$0")/.." && pwd -P)

command -v docker > /dev/null 2>&1 || missing "docker, and it is not on the PATH"

if ! version=$(docker version --format '{{.Server.Version}}' 2>&1) || [ -z "$version" ]; then
  missing "a Docker daemon that answers, and 'docker version' said: $(printf '%s' "$version" | sed -n '/./{p;q;}')"
fi

if ! created=$(docker image inspect --format '{{.Created}}' "$image" 2>&1); then
  missing "the image $image, and 'docker image inspect' said: $(printf '%s' "$created" | sed -n '/./{p;q;}'); design/linux.md says how to build it"
fi

say "image $image made $created; the build goes to the volume $volume, which every checkout shares: a run already in it makes this one wait"

exec docker run --rm --init --pull never --name "cornercase-gpui-linux-check-$$" \
  -v "$repo:$repo" -w "$repo" \
  -v "$volume:/target" -e CARGO_TARGET_DIR=/target \
  "$image" bash -lc "cargo clippy --all-targets --all-features --locked$jobs -- -D warnings && cargo test --locked$jobs"
