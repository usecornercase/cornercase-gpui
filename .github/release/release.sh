#!/bin/sh
set -eu

fail() { echo "::error::$*"; exit 1; }

cd "$(dirname "$0")/../.."

version() {
  value=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
  printf '%s\n' "$value" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' \
    || fail "Cargo.toml's version is not a semantic version: $value"
  printf '%s\n' "$value"
}

latest() {
  git tag --list 'v*' --sort=-v:refname | head -n 1 | sed 's/^v//'
}

newer() {
  [ "$1" != "$2" ] && [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -n 1)" = "$1" ]
}

app_changed() {
  range=$1
  set -- src assets build.rs Cargo.toml Cargo.lock rust-toolchain.toml .cargo tools/macos .github/release .github/workflows/release.yml
  [ -n "$(git diff --name-only "$range" -- "$@")" ]
}

above_latest() {
  last=$(latest)
  [ -z "$last" ] || newer "$1" "$last" \
    || fail "this changes the app, so it releases a new version: set Cargo.toml's version above $last, the last release, and add its section to CHANGELOG.md"
}

pull_request() {
  if ! app_changed "$1...HEAD"; then
    echo "the app does not change: no new version needed"
    return
  fi
  current=$(version)
  above_latest "$current"
  grep -Fqx "## $current" CHANGELOG.md \
    || fail "add a '## $current' section to CHANGELOG.md with what changed, written for users"
  echo "this pull request releases $current"
}

tag() {
  current=$(version)
  name=v$current
  if git rev-parse -q --verify "refs/tags/$name" > /dev/null; then
    if app_changed "$name..HEAD"; then
      fail "$name is already released, but the app changed since: bump the version in a new pull request"
    fi
    echo "$name is already released"
    return
  fi
  above_latest "$current"
  check "$name"
  gh api "repos/$GH_REPO/git/refs" -f ref="refs/tags/$name" -f sha="$(git rev-parse HEAD)" > /dev/null
  gh workflow run release.yml --ref "$name"
  echo "releasing $name"
}

check() {
  tag=$1
  current=$(version)
  [ "$tag" = "v$current" ] || fail "the tag $tag does not match Cargo.toml's version $current: tag v$current, or change the version"
  git merge-base --is-ancestor HEAD origin/main \
    || fail "the tagged commit is not on main: merge it first, then tag"
  grep -Fqx "## $current" CHANGELOG.md \
    || fail "CHANGELOG.md has no '## $current' section: add it with what changed, written for users"
}

notes() {
  wanted=$1
  awk -v heading="## $wanted" '
    $0 == heading { inside = 1; next }
    /^## / { inside = 0 }
    inside { lines[++count] = $0 }
    END {
      first = 1
      while (first <= count && lines[first] == "") first++
      last = count
      while (last >= first && lines[last] == "") last--
      for (i = first; i <= last; i++) print lines[i]
    }
  ' CHANGELOG.md
  printf '\nDownloads, how to open an unsigned build and how to check the checksums: https://github.com/usecornercase/cornercase-gpui#install\n'
}

licences() {
  dest=$1
  notices=$2
  mkdir -p "$dest"
  cp LICENSE-MIT LICENSE-APACHE LICENSE-GPUI "$dest/"
  cp assets/icons/LICENSE "$dest/LICENSE-LUCIDE"
  cp assets/file-icons/LICENSE "$dest/LICENSE-FILE-ICONS"
  cp "$notices" "$dest/THIRD-PARTY-NOTICES.txt"
}

notices() {
  cargo about generate --locked --fail -c .github/release/about.toml -o "$1" .github/release/about.hbs
}

sums() {
  dir=$1
  (
    cd "$dir"
    for file in *; do
      case "$file" in *.sha256 | sha256.sum) continue ;; esac
      sha256sum "$file" > "$file.sha256"
    done
    cat ./*.sha256 > sha256.sum
  )
}

command=${1:-}
[ "$#" -gt 0 ] && shift
case "$command" in
  version) version ;;
  check) [ "$#" -eq 1 ] || fail "usage: release.sh check TAG"; check "$1" ;;
  notes) [ "$#" -eq 1 ] || fail "usage: release.sh notes VERSION"; notes "$1" ;;
  licences) [ "$#" -eq 2 ] || fail "usage: release.sh licences DEST NOTICES"; licences "$1" "$2" ;;
  notices) [ "$#" -eq 1 ] || fail "usage: release.sh notices OUT"; notices "$1" ;;
  sums) [ "$#" -eq 1 ] || fail "usage: release.sh sums DIR"; sums "$1" ;;
  pull-request) [ "$#" -eq 1 ] || fail "usage: release.sh pull-request BASE"; pull_request "$1" ;;
  tag) tag ;;
  *) fail "usage: release.sh version|check|notes|licences|notices|sums|pull-request|tag" ;;
esac
