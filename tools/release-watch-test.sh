#!/bin/sh
set -u

repo=$(cd "$(dirname "$0")/.." && pwd -P)
script=$repo/tools/release-watch.sh
fixtures=$repo/tests/fixtures/release-watch
sh_path=$(command -v "${SH:-sh}")
jq_path=$(command -v jq) || {
  echo "release-watch-test.sh: these tests need jq, to answer gh's --jq as gh does"
  exit 1
}
total=0
failed=0
slug=usecornercase/cornercase-gpui
base=https://github.com/$slug

t=$(mktemp -d /tmp/rwt.XXXXXX) || exit 1
t=$(cd "$t" && pwd -P)
mkdir -p "$t/bin" "$t/sys" "$t/control" "$t/api"
: > "$t/calls"

cleanup() {
  case $t in
    /tmp/rwt.* | /private/tmp/rwt.*) rm -rf "$t" ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' INT TERM HUP

for tool in sed awk grep cat tr rm mktemp; do
  ln -s "$(command -v "$tool")" "$t/sys/$tool"
done

cat > "$t/bin/gh" <<'STUB'
#!/bin/sh
t=@T@
case ${1-} in
  issue) printf 'issue %s %s\n' "${2-}" "$([ "${2-}" = comment ] && echo "$3")" >> "$t/calls" ;;
  *) printf '%s\n' "$*" >> "$t/calls" ;;
esac
case ${1-} in
  api)
    path=$2
    shift 2
    expr=
    [ "${1-}" = --jq ] && expr=$2
    key=$(printf '%s' "$path" | sed -e 's|^repos/[^/]*/[^/]*/||' -e 's|?.*||' | tr / -)
    if [ -e "$t/control/fail-$key" ]; then
      echo "gh: HTTP 502: Bad Gateway (https://api.github.com/$path)" >&2
      exit 1
    fi
    if [ ! -e "$t/api/$key.json" ]; then
      echo 'gh: Not Found (HTTP 404)' >&2
      exit 1
    fi
    if [ -n "$expr" ]; then
      @JQ@ -r "$expr" "$t/api/$key.json"
    else
      cat "$t/api/$key.json"
    fi
    exit 0 ;;
  run)
    if [ "${2-}" = rerun ]; then
      if [ -e "$t/control/fail-rerun" ]; then
        echo 'failed to rerun: HTTP 403' >&2
        exit 1
      fi
      exit 0
    fi ;;
  issue)
    case ${2-} in
      create)
        printf '%s\n' "$4" > "$t/created-title"
        printf '%s\n' "$6" > "$t/created-body"
        echo https://github.com/usecornercase/cornercase-gpui/issues/99
        exit 0 ;;
      comment)
        printf '%s\n' "$5" > "$t/comment-$3"
        echo "https://github.com/usecornercase/cornercase-gpui/issues/$3#issuecomment-1"
        exit 0 ;;
    esac ;;
esac
echo "stub gh: unexpected call: $*" >&2
exit 64
STUB
cat > "$t/bin/git" <<'STUB'
#!/bin/sh
t=@T@
printf 'git %s\n' "$*" >> "$t/calls"
if [ -e "$t/control/fail-git" ]; then
  echo 'fatal: unable to access the repository: Could not resolve host' >&2
  exit 128
fi
case "$*" in
  '-c versionsort.suffix=- ls-remote --tags --refs --sort=-v:refname https://github.com/usecornercase/cornercase-gpui.git v*')
    while read -r name; do
      printf '0000000000000000000000000000000000000000\trefs/tags/%s\n' "$name"
    done < "$t/control/tags"
    exit 0 ;;
esac
echo "stub git: unexpected call: $*" >&2
exit 64
STUB
for stub in gh git; do
  sed -e "s|@T@|$t|g" -e "s|@JQ@|$jq_path|g" "$t/bin/$stub" > "$t/bin/$stub.new"
  mv "$t/bin/$stub.new" "$t/bin/$stub"
  chmod 755 "$t/bin/$stub"
done

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
  rm -rf "$t/control" "$t/api" "$t/created-title" "$t/created-body"
  rm -f "$t"/comment-*
  mkdir -p "$t/control" "$t/api"
  GA=true
  REPO=$slug
  P=$t/bin:$t/sys
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

runw() {
  (
    if [ "$GA" = none ]; then unset GITHUB_ACTIONS; else GITHUB_ACTIONS=$GA; export GITHUB_ACTIONS; fi
    if [ "$REPO" = none ]; then unset GH_REPO; else GH_REPO=$REPO; export GH_REPO; fi
    PATH=$P
    export PATH
    "$sh_path" "$script" "$@"
  ) > "$t/out" 2> "$t/err"
  rc=$?
}

has() {
  grep -F -e "$2" "$1" > /dev/null
}

nohas() {
  ! has "$1" "$2"
}

put() {
  printf '%s\n' "$2" > "$t/api/$1.json"
}

tags() {
  printf '%s\n' "$@" > "$t/control/tags"
}

releases() {
  list=
  for entry in "$@"; do
    draft=false
    case $entry in
      draft:*)
        draft=true
        entry=${entry#draft:}
        ;;
    esac
    list="$list${list:+,}{\"tag_name\":\"$entry\",\"draft\":$draft}"
  done
  put releases "[$list]"
}

runs() {
  list=
  for entry in "$@"; do
    IFS=' ' read -r r_id r_event r_status r_attempt <<EOF
$entry
EOF
    list="$list${list:+,}{\"id\":$r_id,\"event\":\"$r_event\",\"status\":\"$r_status\",\"run_attempt\":$r_attempt}"
  done
  put actions-workflows-release.yml-runs "{\"workflow_runs\":[$list]}"
}

attempt() {
  put "actions-runs-$1-attempts-$2" "{\"status\":\"$3\",\"conclusion\":\"$4\",\"event\":\"$5\",\"html_url\":\"$base/actions/runs/$1\"}"
}

jobs() {
  id=$1
  n=$2
  shift 2
  list='{"name":"version","conclusion":"success","steps":[{"number":1},{"number":2}]},{"name":"publish","conclusion":"skipped","steps":[]}'
  for spec in "$@"; do
    steps=${spec%%|*}
    rest=${spec#*|}
    items=
    i=0
    while [ "$i" -lt "$steps" ]; do
      i=$((i + 1))
      items="$items${items:+,}{\"number\":$i}"
    done
    list="$list,{\"name\":\"${rest#*|}\",\"conclusion\":\"${rest%%|*}\",\"steps\":[$items]}"
  done
  put "actions-runs-$id-attempts-$n-jobs" "{\"jobs\":[$list]}"
}

issues() {
  list=
  for entry in "$@"; do
    i_number=${entry%%|*}
    rest=${entry#*|}
    list="$list${list:+,}{\"number\":$i_number,\"state\":\"${rest%%|*}\",\"title\":\"${rest#*|}\"}"
  done
  put issues "[$list]"
}

issue_text() {
  number=$1
  put "issues-$number" "$("$jq_path" -n --arg b "$2" "{body: \$b}")"
  shift 2
  put "issues-$number-comments" "$("$jq_path" -n "\$ARGS.positional | map({body: .})" --args "$@")"
}

real_case() {
  cp "$fixtures/run-43441808932-attempt-1.json" "$t/api/actions-runs-43441808932-attempts-1.json"
  cp "$fixtures/run-43441808932-attempt-1-jobs.json" "$t/api/actions-runs-43441808932-attempts-1-jobs.json"
  tags v0.1.82 v0.1.81
  releases v0.1.81
  runs '43441808932 workflow_dispatch completed 1'
}

scenario() {
  s_tag=$1
  s_id=$2
  s_n=$3
  s_conclusion=$4
  shift 4
  tags "$s_tag" v0.1.1
  releases v0.1.1
  runs "$s_id workflow_dispatch completed $s_n"
  attempt "$s_id" "$s_n" completed "$s_conclusion" workflow_dispatch
  jobs "$s_id" "$s_n" "$@"
}

dropped() {
  scenario "$1" "$2" "$3" failure '0|cancelled|macOS (sign and notarize)'
}

real_failure() {
  scenario "$1" "$2" "$3" failure '6|failure|macOS (sign and notarize)'
}

link() {
  printf '[Release run of v0.1.90, attempt %s](%s/actions/runs/123/attempts/%s)' "$1" "$base" "$1"
}

the_real_case_re_runs_the_failed_jobs_of_attempt_1() {
  real_case
  runw
  check "status $rc" [ "$rc" = 0 ] &&
    check "no rerun" has "$t/calls" 'run rerun 43441808932 --failed' &&
    check "line" has "$t/out" '(attempt 2 of 3): macOS (sign and notarize)' &&
    check "issue" nohas "$t/calls" 'issue '
}

a_job_no_runner_took_on_attempt_2_is_re_run_again() {
  dropped v0.1.90 123 2
  runw
  check "no rerun" has "$t/calls" 'run rerun 123 --failed' &&
    check "line" has "$t/out" '(attempt 3 of 3)'
}

on_attempt_3_it_tells_which_jobs_no_runner_took() {
  dropped v0.1.90 123 3
  issues
  runw
  check "rerun" nohas "$t/calls" 'run rerun' &&
    check "title" [ "$(cat "$t/created-title" 2> /dev/null)" = 'Release v0.1.90 failed' ] &&
    check "attempts" has "$t/created-body" 'No runner took these jobs on attempt 3.' &&
    check "link" has "$t/created-body" "](${base}/actions/runs/123/attempts/3)" &&
    check "line" has "$t/out" "and no runner took macOS (sign and notarize) on attempt 3 (Release watch re-runs only up to attempt 3): opened $base/issues/99"
}

a_job_no_runner_took_on_attempt_4_is_told_without_claiming_the_watch_ran_it() {
  dropped v0.1.90 123 4
  issues
  runw
  check "rerun" nohas "$t/calls" 'run rerun' &&
    check "create" has "$t/calls" 'issue create' &&
    check "line" has "$t/out" 'on attempt 4 (Release watch re-runs only up to attempt 3): opened' &&
    check "false claim" nohas "$t/out" 'the last Release watch re-runs' &&
    check "cause" has "$t/created-body" 'No runner took these jobs on attempt 4.'
}

a_job_that_ran_a_step_and_failed_is_told_not_re_run() {
  scenario v0.1.90 123 1 failure '1|failure|macOS (sign and notarize)'
  issues
  runw
  check "rerun" nohas "$t/calls" 'run rerun' &&
    check "create" has "$t/calls" 'issue create' &&
    check "steps" has "$t/created-body" "\`macOS (sign and notarize)\` (failure, 1 step ran)" &&
    check "line" has "$t/out" 'ended in failure on attempt 1, and a job ran and failed: opened'
}

a_dropped_job_beside_a_real_failure_is_told_not_re_run() {
  scenario v0.1.90 123 1 failure '0|cancelled|macOS (build)' '9|failure|Linux (x86_64)'
  issues
  runw
  check "rerun" nohas "$t/calls" 'run rerun' &&
    check "create" has "$t/calls" 'issue create'
}

a_failure_with_no_job_listed_is_told_not_re_run() {
  scenario v0.1.90 123 1 failure
  issues
  runw
  check "rerun" nohas "$t/calls" 'run rerun' &&
    check "create" has "$t/calls" 'issue create' &&
    check "none" has "$t/created-body" 'none listed' &&
    check "no failed job to re-run" nohas "$t/created-body" '--failed' &&
    check "whole run" has "$t/created-body" "If the cause has passed, re-run it: \`gh run rerun 123 --repo $slug\`" &&
    check "open the run" has "$t/created-body" 'Open the run' &&
    check "line" has "$t/out" 'but no job is listed as failed'
}

a_cancelled_run_is_told_not_re_run() {
  scenario v0.1.90 123 1 cancelled '0|cancelled|macOS (sign and notarize)'
  issues
  runw
  check "rerun" nohas "$t/calls" 'run rerun' &&
    check "create" has "$t/calls" 'issue create' &&
    check "body" has "$t/created-body" 'was cancelled, so v0.1.90 is tagged' &&
    check "line" has "$t/out" 'was cancelled on attempt 1: opened'
}

a_run_that_has_not_ended_is_left_alone() {
  dropped v0.1.90 123 1
  runs '124 workflow_dispatch in_progress 1' '123 workflow_dispatch completed 1'
  runw
  check "line" has "$t/out" "v0.1.90's Release run has not ended (run 124, attempt 1, in_progress): nothing to do yet" &&
    check "rerun" nohas "$t/calls" 'run rerun' &&
    check "issue" nohas "$t/calls" 'issue ' &&
    check "attempts" nohas "$t/calls" 'attempts'
}

only_the_newest_tag_is_looked_at() {
  dropped v0.1.90 123 1
  tags v0.1.91 v0.1.90
  releases v0.1.91
  runw
  check "line" has "$t/out" 'v0.1.91 is released: nothing to do' &&
    check "asked about the older" nohas "$t/calls" 'v0.1.90'
}

a_tag_is_left_alone_once_a_later_version_is_released() {
  dropped v0.1.90 123 1
  tags v0.1.91 v0.1.90
  releases v0.1.91
  GA=none
  runw --dry-run --tag v0.1.90
  check "line" has "$t/out" 'v0.1.90 has no release, but the later v0.1.91 has: left alone' &&
    check "would re-run" nohas "$t/out" 'dry run: gh run rerun' &&
    check "runs asked" nohas "$t/calls" 'release.yml/runs'
}

a_pull_requests_run_is_never_re_run() {
  dropped v0.1.90 123 1
  runs '123 pull_request completed 1'
  runw
  check "line" has "$t/out" 'v0.1.90 has no Release run yet' &&
    check "rerun" nohas "$t/calls" 'run rerun' &&
    check "issue" nohas "$t/calls" 'issue '
}

a_tag_pushed_by_hand_is_left_to_whoever_pushed_it() {
  dropped v0.1.90 123 1
  runs '123 push completed 1'
  runw
  check "line" has "$t/out" 'left to whoever pushed it' &&
    check "rerun" nohas "$t/calls" 'run rerun' &&
    check "issue" nohas "$t/calls" 'issue '
}

a_pre_release_is_re_run_like_a_release() {
  dropped v0.2.0-rc.1 123 1
  tags v0.2.0-rc.1 v0.1.90
  runw
  check "rerun" has "$t/calls" 'run rerun 123 --failed'
}

a_tag_with_no_release_run_is_left_to_the_version_workflow() {
  dropped v0.1.90 123 1
  runs
  runw
  check "line" has "$t/out" 'v0.1.90 has no Release run yet: left to the Version workflow, which dispatches a tag right after making it, and again on the next push to main if that dispatch was lost' &&
    check "rerun" nohas "$t/calls" 'rerun' &&
    check "dispatch" nohas "$t/calls" 'workflow run' &&
    check "issue" nohas "$t/calls" 'issue '
}

a_failure_it_leaves_opens_one_issue_with_commands() {
  real_failure v0.1.90 123 1
  issues
  runw
  check "title" [ "$(cat "$t/created-title" 2> /dev/null)" = 'Release v0.1.90 failed' ] &&
    check "link" has "$t/created-body" "](${base}/actions/runs/123/attempts/1)" &&
    check "steps" has "$t/created-body" '(failure, 6 steps ran)' &&
    check "rerun command" has "$t/created-body" "gh run rerun 123 --failed --repo $slug" &&
    check "delete command" has "$t/created-body" "gh api -X DELETE repos/$slug/git/refs/tags/v0.1.90"
}

an_attempt_already_told_is_not_told_again() {
  real_failure v0.1.90 123 1
  issues '7|open|Release v0.1.90 failed'
  issue_text 7 'first' "$(link 1)"
  runw
  check "line" has "$t/out" 'already told in #7, nothing more' &&
    check "create" nohas "$t/calls" 'issue create' &&
    check "comment" nohas "$t/calls" 'issue comment'
}

a_later_attempt_is_told_on_the_open_issue() {
  real_failure v0.1.90 123 2
  issues '7|open|Release v0.1.90 failed'
  issue_text 7 "$(link 1)"
  runw
  check "comment" has "$t/calls" 'issue comment 7' &&
    check "create" nohas "$t/calls" 'issue create' &&
    check "text" has "$t/comment-7" '/attempts/2)' &&
    check "line" has "$t/out" 'told in #7'
}

a_closed_issue_that_told_the_attempt_counts() {
  real_failure v0.1.90 123 1
  issues '7|closed|Release v0.1.90 failed'
  issue_text 7 "$(link 1)"
  runw
  check "create" nohas "$t/calls" 'issue create' &&
    check "comment" nohas "$t/calls" 'issue comment' &&
    check "line" has "$t/out" 'already told in #7'
}

a_closed_issue_without_the_attempt_leaves_a_new_one() {
  real_failure v0.1.90 123 2
  issues '7|closed|Release v0.1.90 failed'
  issue_text 7 "$(link 1)"
  runw
  check "create" has "$t/calls" 'issue create' &&
    check "comment" nohas "$t/calls" 'issue comment'
}

the_issue_is_matched_by_its_exact_title_and_attempt() {
  real_failure v0.1.90 123 1
  issues '9|open|Release v0.1.90 failed again' '8|open|Release v0.1.9 failed' '7|open|Release v0.1.90 failed'
  issue_text 7 'first' "$(link 10)"
  issue_text 8 'other'
  issue_text 9 'again'
  runw
  check "comment on 7" has "$t/calls" 'issue comment 7' &&
    check "comment on 8" nohas "$t/calls" 'issue comment 8' &&
    check "comment on 9" nohas "$t/calls" 'issue comment 9' &&
    check "told" nohas "$t/out" 'already told'
}

a_draft_is_not_a_release() {
  dropped v0.1.90 123 1
  releases v0.1.1 draft:v0.1.90
  runw
  check "line" nohas "$t/out" 'is released' &&
    check "rerun" has "$t/calls" 'run rerun 123 --failed'
}

every_failing_read_stops_it_with_status_1() {
  real_failure v0.1.90 123 1
  issues '7|open|Release v0.1.90 failed'
  issue_text 7 'first'
  for pair in \
    'git|listing the v* tags' \
    'releases|listing the releases' \
    'actions-workflows-release.yml-runs|listing the Release runs of v0.1.90' \
    'actions-runs-123-attempts-1|reading attempt 1 of run 123' \
    'actions-runs-123-attempts-1-jobs|reading the jobs of attempt 1 of run 123' \
    'issues|listing the issues' \
    'issues-7|reading issue #7' \
    'issues-7-comments|reading the comments of issue #7'; do
    rm -f "$t"/control/fail-*
    : > "$t/control/fail-${pair%%|*}"
    : > "$t/calls"
    runw
    case ${pair%%|*} in
      issues*) lead="the release watch found that v0.1.90's Release run 123 ended in failure on attempt 1 and could not tell it" ;;
      *) lead='the release watch stopped and decided nothing' ;;
    esac
    check "status $rc for $pair" [ "$rc" = 1 ] &&
      check "error for $pair" has "$t/out" "::error::$lead: ${pair#*|} failed: " &&
      check "said nothing to do for $pair" nohas "$t/out" 'nothing to do' &&
      check "wrote for $pair" nohas "$t/calls" 'issue create' &&
      check "rerun for $pair" nohas "$t/calls" 'run rerun' || return 1
  done
}

a_refused_re_run_is_an_error() {
  dropped v0.1.90 123 1
  : > "$t/control/fail-rerun"
  runw
  check "status $rc" [ "$rc" = 1 ] &&
    check "error" has "$t/out" '::error::the release watch decided what to do and could not do it: re-running the failed jobs of run 123 failed: failed to rerun: HTTP 403'
}

no_tag_yet_is_nothing_to_do() {
  : > "$t/control/tags"
  runw
  check "status $rc" [ "$rc" = 0 ] &&
    check "line" has "$t/out" 'no v* tag yet: nothing to do' &&
    check "gh" nohas "$t/calls" 'api'
}

a_released_newest_tag_is_nothing_to_do() {
  tags v0.1.88 v0.1.87
  releases v0.1.88 v0.1.87
  runw
  check "line" has "$t/out" 'v0.1.88 is released: nothing to do' &&
    check "runs asked" nohas "$t/calls" 'release.yml/runs'
}

a_success_without_a_release_is_left_alone() {
  scenario v0.1.90 123 1 success
  runw
  check "line" has "$t/out" "v0.1.90's last Release run (123) ended in success on attempt 1, yet v0.1.90 has no published release: left alone" &&
    check "issue" nohas "$t/calls" 'issue '
}

a_dry_run_re_runs_nothing_and_says_what_it_would() {
  real_case
  GA=none
  runw --dry-run
  check "status $rc" [ "$rc" = 0 ] &&
    check "line" has "$t/out" 'dry run: gh run rerun 43441808932 --failed' &&
    check "rerun" nohas "$t/calls" 'run rerun'
}

a_dry_run_opens_no_issue_and_prints_it() {
  real_failure v0.1.90 123 1
  issues
  GA=none
  runw --dry-run
  check "line" has "$t/out" 'dry run: gh issue create --title "Release v0.1.90 failed"' &&
    check "body" has "$t/out" '  Jobs that did not pass:' &&
    check "state" has "$t/out" 'would open an issue' &&
    check "create" nohas "$t/calls" 'issue create'
}

a_dry_run_comments_nowhere_and_says_where_it_would() {
  real_failure v0.1.90 123 2
  issues '7|open|Release v0.1.90 failed'
  issue_text 7 "$(link 1)"
  GA=none
  runw --dry-run
  check "line" has "$t/out" 'dry run: gh issue comment 7' &&
    check "state" has "$t/out" 'would comment on #7' &&
    check "comment" nohas "$t/calls" 'issue comment'
}

pointed_at_attempt_1_of_the_real_case_it_decides_a_re_run() {
  real_case
  releases v0.1.82 v0.1.81
  runs '43441808932 workflow_dispatch completed 2'
  GA=none
  runw --dry-run --tag v0.1.82 --attempt 1
  check "status $rc" [ "$rc" = 0 ] &&
    check "taking" has "$t/out" 'dry run: taking v0.1.82 as the newest tag' &&
    check "deciding" has "$t/out" "dry run: deciding on attempt 1 of v0.1.82's last Release run (43441808932) as if it had just ended" &&
    check "rerun line" has "$t/out" '(attempt 2 of 3): macOS (sign and notarize)' &&
    check "dry" has "$t/out" 'dry run: gh run rerun 43441808932 --failed' &&
    check "attempt asked" has "$t/calls" 'attempts/1'
}

an_attempt_that_succeeded_does_not_claim_there_is_no_release() {
  real_case
  releases v0.1.82 v0.1.81
  runs '43441808932 workflow_dispatch completed 2'
  attempt 43441808932 2 completed success workflow_dispatch
  GA=none
  runw --dry-run --tag v0.1.82 --attempt 2
  check "line" has "$t/out" "v0.1.82's Release run 43441808932 ended in success on attempt 2: nothing to re-run or tell" &&
    check "claim" nohas "$t/out" 'no published release'
}

acting_outside_a_runner_is_refused() {
  real_case
  GA=none
  runw
  check "status $rc" [ "$rc" = 2 ] &&
    check "line" has "$t/err" 'it acts only on a GitHub Actions runner: add --dry-run to see what it would do' &&
    check "calls" [ ! -s "$t/calls" ]
}

refused() {
  runw "$@"
  check "status $rc for: $*" [ "$rc" = 2 ] &&
    check "usage for: $*" has "$t/err" 'usage: tools/release-watch.sh' &&
    check "called for: $*" [ ! -s "$t/calls" ]
}

a_wrong_word_exits_2_and_asks_nothing() {
  real_case
  GA=none
  refused --nope &&
    refused --tag v0.1.82 &&
    refused --dry-run --attempt 1 &&
    refused --dry-run --tag v0.1.82 --attempt 0 &&
    refused --dry-run --tag v0.1.82 --attempt x &&
    refused --dry-run --tag v0.1.82 --attempt 01 &&
    refused --dry-run --tag nonsense &&
    refused --dry-run --tag
}

a_tag_that_does_not_exist_exits_2() {
  tags v0.1.90
  GA=none
  runw --dry-run --tag v9.9.9
  check "status $rc" [ "$rc" = 2 ] &&
    check "line" has "$t/err" "no tag v9.9.9 in $slug" &&
    check "api" nohas "$t/calls" 'api'
}

an_attempt_the_run_never_had_exits_2() {
  real_case
  GA=none
  runw --dry-run --tag v0.1.82 --attempt 3
  check "status $rc" [ "$rc" = 2 ] &&
    check "line" has "$t/err" 'run 43441808932 has no attempt 3'
}

help_prints_the_usage_and_exits_0() {
  runw --help
  check "status $rc" [ "$rc" = 0 ] &&
    check "usage" has "$t/out" 'usage: tools/release-watch.sh' &&
    check "statuses" has "$t/out" 'Exit status: 0 it decided' &&
    check "calls" [ ! -s "$t/calls" ]
}

missing_gh_git_or_gh_repo_exits_3_naming_each() {
  P=$t/sys
  REPO=none
  runw
  check "status $rc" [ "$rc" = 3 ] &&
    check "gh" has "$t/err" 'gh is not on the PATH' &&
    check "git" has "$t/err" 'git is not on the PATH' &&
    check "repo" has "$t/err" 'GH_REPO is not set' &&
    check "calls" [ ! -s "$t/calls" ]
}

run_test the_real_case_re_runs_the_failed_jobs_of_attempt_1
run_test a_job_no_runner_took_on_attempt_2_is_re_run_again
run_test on_attempt_3_it_tells_which_jobs_no_runner_took
run_test a_job_no_runner_took_on_attempt_4_is_told_without_claiming_the_watch_ran_it
run_test a_job_that_ran_a_step_and_failed_is_told_not_re_run
run_test a_dropped_job_beside_a_real_failure_is_told_not_re_run
run_test a_failure_with_no_job_listed_is_told_not_re_run
run_test a_cancelled_run_is_told_not_re_run
run_test a_run_that_has_not_ended_is_left_alone
run_test only_the_newest_tag_is_looked_at
run_test a_tag_is_left_alone_once_a_later_version_is_released
run_test a_pull_requests_run_is_never_re_run
run_test a_tag_pushed_by_hand_is_left_to_whoever_pushed_it
run_test a_pre_release_is_re_run_like_a_release
run_test a_tag_with_no_release_run_is_left_to_the_version_workflow
run_test a_failure_it_leaves_opens_one_issue_with_commands
run_test an_attempt_already_told_is_not_told_again
run_test a_later_attempt_is_told_on_the_open_issue
run_test a_closed_issue_that_told_the_attempt_counts
run_test a_closed_issue_without_the_attempt_leaves_a_new_one
run_test the_issue_is_matched_by_its_exact_title_and_attempt
run_test a_draft_is_not_a_release
run_test every_failing_read_stops_it_with_status_1
run_test a_refused_re_run_is_an_error
run_test no_tag_yet_is_nothing_to_do
run_test a_released_newest_tag_is_nothing_to_do
run_test a_success_without_a_release_is_left_alone
run_test a_dry_run_re_runs_nothing_and_says_what_it_would
run_test a_dry_run_opens_no_issue_and_prints_it
run_test a_dry_run_comments_nowhere_and_says_where_it_would
run_test pointed_at_attempt_1_of_the_real_case_it_decides_a_re_run
run_test an_attempt_that_succeeded_does_not_claim_there_is_no_release
run_test acting_outside_a_runner_is_refused
run_test a_wrong_word_exits_2_and_asks_nothing
run_test a_tag_that_does_not_exist_exits_2
run_test an_attempt_the_run_never_had_exits_2
run_test help_prints_the_usage_and_exits_0
run_test missing_gh_git_or_gh_repo_exits_3_naming_each

printf '%s tests, %s failed\n' "$total" "$failed"
[ "$failed" = 0 ]
