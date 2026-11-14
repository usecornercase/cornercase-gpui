#!/bin/sh
set -u

repo=$(cd "$(dirname "$0")/.." && pwd -P)
real_script=$repo/.github/release/release.sh
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
mkdir -p "$t/bin" "$t/sys" "$t/control" "$t/api" "$t/repo/.github/release"
ln -s "$real_script" "$t/repo/.github/release/release.sh"
script=$t/repo/.github/release/release.sh
: > "$t/calls"

cleanup() {
  case $t in
    /tmp/release-test.* | /private/tmp/release-test.*) rm -rf "$t" ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' INT TERM HUP

for tool in dirname sed grep tr sort tail head cat awk; do
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
  if [ -e "$t/control/fail-page-$n" ]; then
    echo "gh: HTTP 502: Bad Gateway (https://api.github.com/$path)" >&2
    exit 1
  fi
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
  rm -f "$t/repo/CHANGELOG.md"
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
    "$sh_path" "$script" "$@"
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

install='Downloads, how to open an unsigned build and how to check the checksums: https://github.com/usecornercase/cornercase-gpui#install'

changelog() {
  {
    printf '# Changelog\n\nA line of preamble.\n'
    for entry in "$@"; do
      case $entry in
        empty:*) printf '\n## %s\n' "${entry#empty:}" ;;
        *) printf '\n## %s\n\n- A change of %s.\n- Another change of %s.\n' "$entry" "$entry" "$entry" ;;
      esac
    done
  } > "$t/repo/CHANGELOG.md"
}

body_is() {
  cat > "$t/want"
  cmp -s "$t/want" "$t/out"
}

no_release_yet_becomes_the_latest() {
  page 1
  runr latest-flag 0.1.0
  check "rc $rc" [ "$rc" = 0 ] &&
    check "flag" flag_is --latest &&
    check "reason" has "$t/err" "no release is published yet"
}

a_version_above_every_published_release_becomes_the_latest() {
  page 1 v0.1.90 v0.1.89
  runr latest-flag 0.1.91
  check "flag" flag_is --latest &&
    check "reason" has "$t/err" "the highest is 0.1.90"
}

a_version_below_a_published_release_is_not_the_latest() {
  page 1 v0.1.92 v0.1.90
  runr latest-flag 0.1.91
  check "flag" flag_is --latest=false &&
    check "reason" has "$t/err" "0.1.92 is published"
}

a_higher_draft_does_not_count() {
  page 1 draft:v0.1.92 v0.1.90
  runr latest-flag 0.1.91
  check "flag" flag_is --latest
}

a_pre_release_is_never_the_latest_and_lists_nothing() {
  page 1 v0.1.90
  : > "$t/control/fail"
  runr latest-flag 0.2.0-rc.1
  check "rc $rc" [ "$rc" = 0 ] &&
    check "flag" flag_is --latest=false &&
    check "calls" no_call
}

a_published_pre_release_does_not_count() {
  page 1 pre:v0.2.0-rc.1 v0.1.90
  runr latest-flag 0.1.91
  check "flag" flag_is --latest
}

the_same_version_published_again_stays_the_latest() {
  page 1 v0.1.91 v0.1.90
  runr latest-flag 0.1.91
  check "flag" flag_is --latest &&
    check "reason" has "$t/err" "published already"
}

a_failed_listing_prints_no_flag_and_fails() {
  page 1 v0.1.90
  : > "$t/control/fail"
  runr latest-flag 0.1.91
  check "rc $rc" [ "$rc" = 1 ] &&
    check "stdout" [ ! -s "$t/out" ] &&
    check "gh's error" has "$t/err" "Bad Gateway" &&
    check "annotation" has "$t/err" "::error::nothing was published"
}

versions_compare_as_numbers() {
  page 1 v0.1.9
  runr latest-flag 0.1.10
  check "0.1.10 over 0.1.9" flag_is --latest || return 1
  page 1 v0.1.10
  runr latest-flag 0.1.9
  check "0.1.9 under 0.1.10" flag_is --latest=false
}

a_release_is_above_its_pre_release() {
  page 1 v0.2.0-rc.1
  runr latest-flag 0.2.0
  check "flag" flag_is --latest
}

a_tag_that_is_no_version_does_not_count() {
  page 1 nightly v1.0 vnext v0.1.90
  runr latest-flag 0.1.91
  check "flag" flag_is --latest &&
    check "reason" has "$t/err" "the highest is 0.1.90"
}

a_higher_release_on_a_later_page_counts() {
  page 1 v0.1.90
  page 2 v0.1.92
  runr latest-flag 0.1.91
  check "flag" flag_is --latest=false
}

a_wrong_argument_fails_and_asks_nothing() {
  page 1 v0.1.90
  for args in "" "0.1.91 0.1.92" v0.1.91 soon; do
    # shellcheck disable=SC2086
    runr latest-flag $args
    check "rc for '$args': $rc" [ "$rc" = 1 ] || return 1
    check "stdout for '$args'" [ ! -s "$t/out" ] || return 1
    check "annotation for '$args'" has "$t/err" "::error::" || return 1
  done
  check "calls" no_call
}

a_missing_gh_repo_fails_and_asks_nothing() {
  page 1 v0.1.90
  REPO=none
  runr latest-flag 0.1.91
  check "rc $rc" [ "$rc" = 1 ] &&
    check "names GH_REPO" has "$t/err" "::error::GH_REPO is not set" &&
    check "calls" no_call
}

one_version_keeps_the_body_as_it_was() {
  changelog 0.1.65 0.1.64 0.1.63
  page 1 v0.1.64 v0.1.63
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

$install
BODY
  check "reason" has "$t/err" "0.1.64 is the highest release below 0.1.65"
}

a_version_never_released_is_carried_under_its_heading() {
  changelog 0.1.65 0.1.64 0.1.63
  page 1 v0.1.63
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

### 0.1.64

- A change of 0.1.64.
- Another change of 0.1.64.

---

$install
BODY
  :
}

several_versions_never_released_come_newest_first() {
  changelog 0.1.65 0.1.64 0.1.63 0.1.62
  page 1 v0.1.62
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

### 0.1.64

- A change of 0.1.64.
- Another change of 0.1.64.

### 0.1.63

- A change of 0.1.63.
- Another change of 0.1.63.

---

$install
BODY
  :
}

sections_come_newest_first_whatever_their_order_in_the_changelog() {
  changelog 0.1.65 0.1.63 0.1.64 0.1.62
  page 1 v0.1.62
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

### 0.1.64

- A change of 0.1.64.
- Another change of 0.1.64.

### 0.1.63

- A change of 0.1.63.
- Another change of 0.1.63.

---

$install
BODY
  :
}

carried_versions_compare_as_numbers() {
  changelog 0.1.11 0.1.10 0.1.9 0.1.8
  page 1 v0.1.8
  runr notes 0.1.11
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.11.
- Another change of 0.1.11.

### 0.1.10

- A change of 0.1.10.
- Another change of 0.1.10.

### 0.1.9

- A change of 0.1.9.
- Another change of 0.1.9.

---

$install
BODY
  :
}

the_previous_release_is_the_highest_below_whatever_the_list_order() {
  changelog 0.1.65 0.1.64 0.1.63 0.1.62
  page 1 v0.1.62 v0.1.64 v0.1.63
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

$install
BODY
  :
}

the_versions_own_release_is_not_the_previous_one() {
  changelog 0.1.65 0.1.64 0.1.63
  page 1 v0.1.65 v0.1.63
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

### 0.1.64

- A change of 0.1.64.
- Another change of 0.1.64.

---

$install
BODY
  :
}

a_draft_is_not_the_previous_release() {
  changelog 0.1.65 0.1.64 0.1.63
  page 1 draft:v0.1.64 v0.1.63
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

### 0.1.64

- A change of 0.1.64.
- Another change of 0.1.64.

---

$install
BODY
  :
}

a_pre_release_is_not_the_previous_release() {
  changelog 0.1.65 0.1.65-rc.1 0.1.64 0.1.63
  page 1 pre:v0.1.65-rc.1 v0.1.63
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

### 0.1.65-rc.1

- A change of 0.1.65-rc.1.
- Another change of 0.1.65-rc.1.

### 0.1.64

- A change of 0.1.64.
- Another change of 0.1.64.

---

$install
BODY
  :
}

a_tag_that_is_no_version_is_not_the_previous_release() {
  changelog 0.1.65 0.1.64 0.1.63
  page 1 nightly v0.1.64.1 v0.1.63
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

### 0.1.64

- A change of 0.1.64.
- Another change of 0.1.64.

---

$install
BODY
  :
}

the_previous_release_on_a_later_page_counts() {
  changelog 0.1.65 0.1.64 0.1.63
  page 1 v0.1.90
  page 2 v0.1.63
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

### 0.1.64

- A change of 0.1.64.
- Another change of 0.1.64.

---

$install
BODY
  :
}

an_empty_section_in_between_is_left_out() {
  changelog 0.1.65 empty:0.1.64 0.1.63
  page 1 v0.1.63
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

$install
BODY
  :
}

sections_above_the_version_are_left_out() {
  changelog 0.1.65 0.1.64 0.1.63 0.1.62
  page 1 v0.1.62
  runr notes 0.1.64
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.64.
- Another change of 0.1.64.

### 0.1.63

- A change of 0.1.63.
- Another change of 0.1.63.

---

$install
BODY
  :
}

a_pre_releases_notes_count_from_the_previous_release() {
  changelog 0.2.0-rc.2 0.2.0-rc.1 0.1.65
  page 1 pre:v0.2.0-rc.1 v0.1.65
  runr notes 0.2.0-rc.2
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.2.0-rc.2.
- Another change of 0.2.0-rc.2.

### 0.2.0-rc.1

- A change of 0.2.0-rc.1.
- Another change of 0.2.0-rc.1.

---

$install
BODY
  :
}

no_release_below_keeps_the_versions_own_section() {
  changelog 0.1.65 0.1.64 0.1.63
  for listed in "" v0.1.90; do
    # shellcheck disable=SC2086
    page 1 $listed
    runr notes 0.1.65
    check "rc for '$listed': $rc" [ "$rc" = 0 ] || return 1
    check "body for '$listed'" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

$install
BODY
    check "reason for '$listed'" has "$t/err" "no release is published below 0.1.65" || return 1
  done
}

a_failed_listing_keeps_the_versions_own_section_and_warns() {
  changelog 0.1.65 0.1.64 0.1.63
  page 1 v0.1.63
  : > "$t/control/fail"
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

$install
BODY
  check "gh's error" has "$t/err" "Bad Gateway" &&
  check "annotation" has "$t/err" "::warning::"
}

a_listing_that_fails_half_way_is_not_used_for_the_notes() {
  changelog 0.1.65 0.1.64 0.1.63
  page 1 v0.1.63
  page 2 v0.1.64
  : > "$t/control/fail-page-2"
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 0 ] || return 1
  check "body" body_is <<BODY || return 1
- A change of 0.1.65.
- Another change of 0.1.65.

$install
BODY
  check "annotation" has "$t/err" "::warning::"
}

a_version_without_a_section_fails_and_asks_nothing() {
  changelog 0.1.65 0.1.63
  page 1 v0.1.63
  runr notes 0.1.64
  check "rc $rc" [ "$rc" = 1 ] &&
    check "stdout" [ ! -s "$t/out" ] &&
    check "annotation" has "$t/err" "::error::" &&
    check "calls" no_call
}

a_wrong_argument_to_notes_fails_and_asks_nothing() {
  changelog 0.1.65 soon v0.1.65
  page 1 v0.1.63
  for args in "" "0.1.65 0.1.64" v0.1.65 soon; do
    # shellcheck disable=SC2086
    runr notes $args
    check "rc for '$args': $rc" [ "$rc" = 1 ] || return 1
    check "stdout for '$args'" [ ! -s "$t/out" ] || return 1
    check "annotation for '$args'" has "$t/err" "::error::" || return 1
  done
  check "calls" no_call
}

notes_without_gh_repo_fails_and_asks_nothing() {
  changelog 0.1.65
  page 1 v0.1.63
  REPO=none
  runr notes 0.1.65
  check "rc $rc" [ "$rc" = 1 ] &&
    check "stdout" [ ! -s "$t/out" ] &&
    check "names GH_REPO" has "$t/err" "::error::GH_REPO is not set" &&
    check "calls" no_call
}

a_listing_that_fails_half_way_publishes_nothing() {
  page 1 v0.1.90
  page 2 v0.1.92
  : > "$t/control/fail-page-2"
  runr latest-flag 0.1.91
  check "rc $rc" [ "$rc" = 1 ] &&
    check "stdout" [ ! -s "$t/out" ] &&
    check "annotation" has "$t/err" "::error::nothing was published"
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
run_test one_version_keeps_the_body_as_it_was
run_test a_version_never_released_is_carried_under_its_heading
run_test several_versions_never_released_come_newest_first
run_test sections_come_newest_first_whatever_their_order_in_the_changelog
run_test carried_versions_compare_as_numbers
run_test the_previous_release_is_the_highest_below_whatever_the_list_order
run_test the_versions_own_release_is_not_the_previous_one
run_test a_draft_is_not_the_previous_release
run_test a_pre_release_is_not_the_previous_release
run_test a_tag_that_is_no_version_is_not_the_previous_release
run_test the_previous_release_on_a_later_page_counts
run_test an_empty_section_in_between_is_left_out
run_test sections_above_the_version_are_left_out
run_test a_pre_releases_notes_count_from_the_previous_release
run_test no_release_below_keeps_the_versions_own_section
run_test a_failed_listing_keeps_the_versions_own_section_and_warns
run_test a_listing_that_fails_half_way_is_not_used_for_the_notes
run_test a_version_without_a_section_fails_and_asks_nothing
run_test a_wrong_argument_to_notes_fails_and_asks_nothing
run_test notes_without_gh_repo_fails_and_asks_nothing
run_test a_listing_that_fails_half_way_publishes_nothing

printf '%s tests, %s failed\n' "$total" "$failed"
[ "$failed" = 0 ]
