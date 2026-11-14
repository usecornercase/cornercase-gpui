#!/bin/sh
set -eu

version_pattern='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'

fail() { echo "::error::$*" >&2; exit 1; }

note() { echo "$*" >&2; }

warn() { echo "::warning::$*" >&2; }

cd "$(dirname "$0")/../.."

version() {
  value=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
  printf '%s\n' "$value" | grep -Eq "$version_pattern" \
    || fail "Cargo.toml's version is not a semantic version: $value"
  printf '%s\n' "$value"
}

latest() {
  git -c versionsort.suffix=- tag --list 'v*' --sort=-v:refname | head -n 1 | sed 's/^v//'
}

ordered() {
  tr - '~' | sort -V "$@" | tr '~' -
}

highest() {
  ordered | tail -n 1
}

newer() {
  [ "$1" != "$2" ] && [ "$(printf '%s\n%s\n' "$1" "$2" | highest)" = "$1" ]
}

needs_repo() {
  [ -n "${GH_REPO:-}" ] || fail "GH_REPO is not set: name the repository, such as usecornercase/cornercase-gpui"
}

published() {
  tags=$(gh api --paginate "repos/$GH_REPO/releases?per_page=100" \
    --jq '.[] | select((.draft or .prerelease) | not) | .tag_name') || return 1
  printf '%s\n' "$tags" | sed -n 's/^v//p' | grep -E "$version_pattern" || :
}

latest_flag() {
  wanted=$1
  printf '%s\n' "$wanted" | grep -Eq "$version_pattern" \
    || fail "latest-flag takes a version such as 0.1.91, not '$wanted'"
  case $wanted in
    *-*)
      note "$wanted is a pre-release, which is never the Latest release: --latest=false"
      echo --latest=false
      return
      ;;
  esac
  needs_repo
  versions=$(published) \
    || fail "nothing was published: listing the releases of $GH_REPO failed (gh's error is the line before this one in the release step's log); re-run the failed jobs once GitHub answers"
  top=$(printf '%s\n' "$versions" | highest)
  if [ -z "$top" ]; then
    note "no release is published yet, so $wanted becomes the Latest release: --latest"
    echo --latest
  elif newer "$top" "$wanted"; then
    note "$top is published and above $wanted, so $wanted does not become the Latest release: --latest=false"
    echo --latest=false
  elif [ "$top" = "$wanted" ]; then
    note "$wanted is published already and nothing above it is, so it is the Latest release: --latest"
    echo --latest
  else
    note "$wanted is above every published release (the highest is $top), so it becomes the Latest release: --latest"
    echo --latest
  fi
}

app_changed() {
  range=$1
  set -- src assets build.rs Cargo.toml Cargo.lock rust-toolchain.toml .cargo tools/macos .github/release \
    .github/workflows/release.yml LICENSE-MIT LICENSE-APACHE LICENSE-GPUI
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
  [ -n "$(section "$current")" ] \
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
    if ! gh release view "$name" > /dev/null 2>&1 \
      && [ "$(gh run list --workflow release.yml --branch "$name" --limit 1 --json databaseId --jq length)" = 0 ]; then
      gh workflow run release.yml --ref "$name"
      echo "$name was tagged but never released: releasing it"
      return
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
  [ -n "$(section "$current")" ] \
    || fail "CHANGELOG.md has no '## $current' section with text: add what changed, written for users"
}

section() {
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
}

below() {
  while read -r found; do
    if newer "$1" "$found"; then printf '%s\n' "$found"; fi
  done | highest
}

carried() {
  sed -n 's/^## //p' CHANGELOG.md | grep -E "$version_pattern" | ordered -r | while read -r found; do
    newer "$found" "$2" || break
    if newer "$1" "$found" && [ -n "$(section "$found")" ]; then printf '%s\n' "$found"; fi
  done
}

carried_versions() {
  if ! listed=$(published); then
    warn "listing the releases of $GH_REPO failed (gh's error is the line before this one), so the notes are $1's section alone: the sections of versions not published since the previous release are left out"
    return
  fi
  previous=$(printf '%s\n' "$listed" | below "$1")
  if [ -z "$previous" ]; then
    note "no release is published below $1, so the notes are its section alone"
    return
  fi
  unreleased=$(carried "$1" "$previous" | tr '\n' ' ')
  if [ -z "$unreleased" ]; then
    note "$previous is the highest release below $1, so the notes are $1's section alone"
  else
    note "$previous is the highest release below $1, so the notes hold $1's section and those of $(printf "%s" "${unreleased% }" | sed "s/ /, /g"), none of which is published"
  fi
  printf '%s\n' "$unreleased"
}

notes() {
  printf '%s\n' "$1" | grep -Eq "$version_pattern" \
    || fail "notes takes a version such as 0.1.91, not '$1'"
  [ -n "$(section "$1")" ] \
    || fail "CHANGELOG.md has no '## $1' section with text, so there are no notes to publish"
  needs_repo
  extra=$(carried_versions "$1")
  section "$1"
  for older in $extra; do
    printf '\n### %s\n\n' "$older"
    section "$older"
  done
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
  latest-flag) [ "$#" -eq 1 ] || fail "usage: release.sh latest-flag VERSION"; latest_flag "$1" ;;
  tag) tag ;;
  *) fail "usage: release.sh version|check|notes|licences|notices|sums|pull-request|latest-flag|tag" ;;
esac
