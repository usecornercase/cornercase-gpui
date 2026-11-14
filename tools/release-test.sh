#!/bin/sh
set -u

repo=$(cd "$(dirname "$0")/.." && pwd -P)
script=$repo/.github/release/release.sh
sh_path=$(command -v "${SH:-sh}")
jq_path=$(command -v jq) || {
  echo "release-test.sh: these tests need jq, to answer gh's --jq as gh does"
  exit 1
}
total=0
failed=0
slug=usecornercase/cornercase-gpui

t=$(mktemp -d /tmp/release-test.XXXXXX) || exit 1
t=$(cd "$t" && pwd -P)
mkdir -p "$t/bin" "$t/sys" "$t/control" "$t/api"
: > "$t/calls"

cleanup() {
  case $t in
    /tmp/release-test.* | /private/tmp/release-test.*) rm -rf "$t" ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' INT TERM HUP

for tool in dirname sed grep tr sort tail head cat; do
  ln -s "$(command -v "$tool")" "$t/sys/$tool"
done

cat > "$t/bin/gh" <<'STUB'
#!/bin/sh
t=@T@
printf '%s\n' "$*" >> "$t/calls"
[ "${1-}" = api ] || { echo "stub gh: unexpected call: $*" >&2; exit 64; }
shift
paginate=false
[ "${1-}" = --paginate ] && { paginate=true; shift; }
path=$1
expr=
[ "${2-}" = --jq ] && expr=$3
[ "$path" = "repos/usecornercase/cornercase-gpui/releases?per_page=100" ] || { echo "stub gh: unexpected call: api $path" >&2; exit 64; }
if [ -e "$t/control/fail" ]; then
  echo "gh: HTTP 502: Bad Gateway (https://api.github.com/$path)" >&2
  exit 1
fi
n=1
while [ -e "$t/api/releases-$n.json" ]; do
  @JQ@ -r "$expr" "$t/api/releases-$n.json"
  $paginate || break
  n=$((n + 1))
done
exit 0
STUB
sed -e "s|@T@|$t|g" -e "s|@JQ@|$jq_path|g" "$t/bin/gh" > "$t/bin/gh.new"
mv "$t/bin/gh.new" "$t/bin/gh"
chmod 755 "$t/bin/gh"

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
  rm -rf "$t/control" "$t/api"
  mkdir -p "$t/control" "$t/api"
  REPO=$slug
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

runr() {
  (
    if [ "$REPO" = none ]; then unset GH_REPO; else GH_REPO=$REPO; export GH_REPO; fi
    PATH=$t/bin:$t/sys
    export PATH
    "$sh_path" "$script" latest-flag "$@"
  ) > "$t/out" 2> "$t/err"
  rc=$?
}

has() {
  grep -F -e "$2" "$1" > /dev/null
}

page() {
  n=$1
  shift
  list=
  for entry in "$@"; do
    draft=false
    pre=false
    case $entry in
      draft:*)
        draft=true
        entry=${entry#draft:}
        ;;
      pre:*)
        pre=true
        entry=${entry#pre:}
        ;;
    esac
    list="$list${list:+,}{\"tag_name\":\"$entry\",\"draft\":$draft,\"prerelease\":$pre}"
  done
  printf '[%s]\n' "$list" > "$t/api/releases-$n.json"
}

flag_is() {
  [ "$(cat "$t/out")" = "$1" ]
}

no_call() {
  [ ! -s "$t/calls" ]
}

no_release_yet_becomes_the_latest() {
  page 1
  runr 0.1.0
  check "rc $rc" [ "$rc" = 0 ] &&
    check "flag" flag_is --latest &&
    check "reason" has "$t/err" "no release is published yet"
}

a_version_above_every_published_release_becomes_the_latest() {
  page 1 v0.1.90 v0.1.89
  runr 0.1.91
  check "flag" flag_is --latest &&
    check "reason" has "$t/err" "the highest is 0.1.90"
}

a_version_below_a_published_release_is_not_the_latest() {
  page 1 v0.1.92 v0.1.90
  runr 0.1.91
  check "flag" flag_is --latest=false &&
    check "reason" has "$t/err" "0.1.92 is published"
}

a_higher_draft_does_not_count() {
  page 1 draft:v0.1.92 v0.1.90
  runr 0.1.91
  check "flag" flag_is --latest
}

a_pre_release_is_never_the_latest_and_lists_nothing() {
  page 1 v0.1.90
  : > "$t/control/fail"
  runr 0.2.0-rc.1
  check "rc $rc" [ "$rc" = 0 ] &&
    check "flag" flag_is --latest=false &&
    check "calls" no_call
}

a_published_pre_release_does_not_count() {
  page 1 pre:v0.2.0-rc.1 v0.1.90
  runr 0.1.91
  check "flag" flag_is --latest
}

the_same_version_published_again_stays_the_latest() {
  page 1 v0.1.91 v0.1.90
  runr 0.1.91
  check "flag" flag_is --latest &&
    check "reason" has "$t/err" "published already"
}

a_failed_listing_prints_no_flag_and_fails() {
  page 1 v0.1.90
  : > "$t/control/fail"
  runr 0.1.91
  check "rc $rc" [ "$rc" = 1 ] &&
    check "stdout" [ ! -s "$t/out" ] &&
    check "gh's error" has "$t/err" "Bad Gateway" &&
    check "annotation" has "$t/err" "::error::nothing was published"
}

versions_compare_as_numbers() {
  page 1 v0.1.9
  runr 0.1.10
  check "0.1.10 over 0.1.9" flag_is --latest || return 1
  page 1 v0.1.10
  runr 0.1.9
  check "0.1.9 under 0.1.10" flag_is --latest=false
}

a_release_is_above_its_pre_release() {
  page 1 v0.2.0-rc.1
  runr 0.2.0
  check "flag" flag_is --latest
}

a_tag_that_is_no_version_does_not_count() {
  page 1 nightly v1.0 vnext v0.1.90
  runr 0.1.91
  check "flag" flag_is --latest &&
    check "reason" has "$t/err" "the highest is 0.1.90"
}

a_higher_release_on_a_later_page_counts() {
  page 1 v0.1.90
  page 2 v0.1.92
  runr 0.1.91
  check "flag" flag_is --latest=false
}

a_wrong_argument_fails_and_asks_nothing() {
  page 1 v0.1.90
  for args in "" "0.1.91 0.1.92" v0.1.91 soon; do
    # shellcheck disable=SC2086
    runr $args
    check "rc for '$args': $rc" [ "$rc" = 1 ] || return 1
    check "stdout for '$args'" [ ! -s "$t/out" ] || return 1
    check "annotation for '$args'" has "$t/err" "::error::" || return 1
  done
  check "calls" no_call
}

a_missing_gh_repo_fails_and_asks_nothing() {
  page 1 v0.1.90
  REPO=none
  runr 0.1.91
  check "rc $rc" [ "$rc" = 1 ] &&
    check "names GH_REPO" has "$t/err" "::error::GH_REPO is not set" &&
    check "calls" no_call
}

run_test no_release_yet_becomes_the_latest
run_test a_version_above_every_published_release_becomes_the_latest
run_test a_version_below_a_published_release_is_not_the_latest
run_test a_higher_draft_does_not_count
run_test a_pre_release_is_never_the_latest_and_lists_nothing
run_test a_published_pre_release_does_not_count
run_test the_same_version_published_again_stays_the_latest
run_test a_failed_listing_prints_no_flag_and_fails
run_test versions_compare_as_numbers
run_test a_release_is_above_its_pre_release
run_test a_tag_that_is_no_version_does_not_count
run_test a_higher_release_on_a_later_page_counts
run_test a_wrong_argument_fails_and_asks_nothing
run_test a_missing_gh_repo_fails_and_asks_nothing

printf '%s tests, %s failed\n' "$total" "$failed"
[ "$failed" = 0 ]
