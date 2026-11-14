#!/bin/sh
set -eu

name=release-watch.sh
attempts=3
tag_pattern='^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'
tab=$(printf '\t')

say() {
  printf '%s: %s\n' "$name" "$*" >&2
}

usage_text() {
  cat <<'USAGE'
usage: tools/release-watch.sh [--dry-run [--tag TAG [--attempt N]]]

Looks at the newest v* tag of GH_REPO and its last Release run (.github/workflows/release.yml).
When that run ended in failure and every job that did not pass ran no step (no runner took it),
it re-runs those jobs (gh run rerun --failed), up to attempt 3 of the run. When the run ended any
other way but success, it opens or comments on the issue "Release <tag> failed", once per attempt.
It acts only on a GitHub Actions runner (GITHUB_ACTIONS=true); anywhere else, add --dry-run.
  --dry-run     read and decide as usual, print what it would do, and change nothing
  --tag TAG     with --dry-run: take TAG as the newest tag
  --attempt N   with --tag: decide on attempt N of TAG's last Release run as if it had just ended
It needs gh and git on the PATH, and GH_REPO (owner/name).
Exit status: 0 it decided, whatever it did; 1 a gh or git call failed and nothing was decided;
2 a wrong word, or acting outside a runner; 3 gh, git or GH_REPO is missing.
See design/releases.md.
USAGE
}

refuse() {
  say "$1"
  usage_text >&2
  exit 2
}

whole_number() {
  case $1 in
    '' | *[!0-9]* | 0 | 0[0-9]*) return 1 ;;
  esac
}

dry=0
tag_arg=
attempt_arg=
while [ $# -gt 0 ]; do
  case $1 in
    -h | --help)
      usage_text
      exit 0
      ;;
    --dry-run) dry=1 ;;
    --tag)
      [ $# -ge 2 ] || refuse "--tag takes a tag, such as v0.1.82"
      tag_arg=$2
      printf '%s\n' "$tag_arg" | grep -Eq "$tag_pattern" || refuse "--tag takes a tag such as v0.1.82, not '$tag_arg'"
      shift
      ;;
    --attempt)
      [ $# -ge 2 ] || refuse "--attempt takes one whole number"
      whole_number "$2" || refuse "--attempt takes a whole number of at least 1, not '$2'"
      attempt_arg=$2
      shift
      ;;
    *) refuse "unknown word '$1'" ;;
  esac
  shift
done
[ -z "$tag_arg" ] || [ "$dry" = 1 ] || refuse "--tag needs --dry-run"
[ -z "$attempt_arg" ] || [ -n "$tag_arg" ] || refuse "--attempt needs --tag"

absent=0
for tool in gh git; do
  command -v "$tool" > /dev/null 2>&1 || {
    say "nothing was run: $tool is not on the PATH"
    absent=1
  }
done
[ -n "${GH_REPO:-}" ] || {
  say "nothing was run: GH_REPO is not set (owner/name, such as usecornercase/cornercase-gpui)"
  absent=1
}
[ "$absent" = 0 ] || exit 3

if [ "$dry" = 0 ] && [ "${GITHUB_ACTIONS:-}" != true ]; then
  say "it acts only on a GitHub Actions runner: add --dry-run to see what it would do"
  exit 2
fi

errfile=$(mktemp "${TMPDIR:-/tmp}/release-watch.XXXXXX") || exit 1
cleanup() {
  rm -f "$errfile"
}
trap cleanup EXIT
trap 'exit 1' INT TERM HUP

first_line() {
  sed -n '/./{p;q;}' "$errfile"
}

stop() {
  printf '::error::the release watch %s: %s failed: %s\n' "$1" "$2" "$(first_line)"
  exit 1
}

run_cmd() {
  answer=$("$@" 2> "$errfile" < /dev/null)
}

ask() {
  what=$1
  shift
  run_cmd "$@" || stop 'stopped and decided nothing' "$what"
}

ask_write() {
  what=$1
  shift
  run_cmd "$@" || stop 'decided what to do and could not do it' "$what"
}

in_list() {
  case "
$1
" in
    *"
$2
"*) return 0 ;;
  esac
  return 1
}

indent() {
  printf '%s\n' "$1" | sed 's/^/  /'
}

read_tags() {
  ask "listing the v* tags" git -c versionsort.suffix=- ls-remote --tags --refs --sort=-v:refname \
    "https://github.com/$GH_REPO.git" 'v*'
  tag_list=$(printf '%s\n' "$answer" | sed -n 's|^.*refs/tags/||p' | grep -E "$tag_pattern" || true)
}

read_releases() {
  ask "listing the releases" gh api "repos/$GH_REPO/releases?per_page=100" \
    --jq '.[] | select(.draft | not) | .tag_name'
  releases=$answer
}

read_runs() {
  ask "listing the Release runs of $1" gh api \
    "repos/$GH_REPO/actions/workflows/release.yml/runs?branch=$1&per_page=20" \
    --jq '.workflow_runs[] | select(.event == "workflow_dispatch" or .event == "push") | "\(.id) \(.event) \(.status) \(.run_attempt)"'
  runs=$answer
}

read_attempt() {
  ask "reading attempt $2 of run $1" gh api "repos/$GH_REPO/actions/runs/$1/attempts/$2" \
    --jq '"\(.status) \(.conclusion) \(.event) \(.html_url)"'
  read -r _ conclusion _ url <<EOF
$answer
EOF
}

read_jobs() {
  ask "reading the jobs of attempt $2 of run $1" gh api \
    "repos/$GH_REPO/actions/runs/$1/attempts/$2/jobs?per_page=100" \
    --jq '.jobs[] | select(.conclusion != "success" and .conclusion != "skipped" and .conclusion != "neutral") | "\(.steps | length)\t\(.conclusion)\t\(.name)"'
  jobs=$answer
}

sum_jobs() {
  broken=0
  dropped=0
  names=
  described=
  while IFS=$tab read -r steps jconclusion jname; do
    [ -n "$jname" ] || continue
    broken=$((broken + 1))
    case $steps in
      0)
        dropped=$((dropped + 1))
        ran='no step ran'
        ;;
      1) ran='1 step ran' ;;
      *) ran="$steps steps ran" ;;
    esac
    names=${names:+$names, }$jname
    described=${described:+$described, }"\`$jname\` ($jconclusion, $ran)"
  done <<EOF
$jobs
EOF
}

later_released() {
  for other in $tag_list; do
    [ "$other" != "$1" ] || return 1
    if in_list "$releases" "$other"; then
      later=$other
      return 0
    fi
  done
  return 1
}

unfinished() {
  while read -r uid _ ustatus uattempt; do
    [ -n "$uid" ] || continue
    if [ "$ustatus" != completed ]; then
      return 0
    fi
  done <<EOF
$runs
EOF
  return 1
}

rerun() {
  if [ "$dry" = 1 ]; then
    printf 'dry run: gh run rerun %s --failed\n' "$1"
    return
  fi
  ask_write "re-running the failed jobs of run $1" gh run rerun "$1" --failed
}

fix_steps() {
  printf "1. If the cause has passed (Apple's notary service, a download), %s: \`gh run rerun %s --failed --repo %s\`\n" \
    "$1" "$id" "$GH_REPO"
  printf "2. Otherwise, fix it in a pull request with the next version. Once that is released, delete this tag: \`gh api -X DELETE repos/%s/git/refs/tags/%s\`. If \`gh release list --repo %s\` shows a draft of it, delete that too: \`gh release delete %s --yes --repo %s\`." \
    "$GH_REPO" "$tag" "$GH_REPO" "$tag" "$GH_REPO"
}

out_steps() {
  printf "1. When GitHub has runners again (https://www.githubstatus.com), re-run them: \`gh run rerun %s --failed --repo %s\`" \
    "$id" "$GH_REPO"
}

issue_body() {
  printf '%s\n\n' "The [Release run of $tag, attempt $n]($url/attempts/$n) ended in $conclusion, so $tag is tagged and has no release."
  printf 'Jobs that did not pass: %s.\n\n' "${described:-none listed}"
  printf '%s\n\n' "$cause"
  printf 'What to do:\n%s\n\n' "$steps_text"
  printf "Release watch (\`.github/workflows/release-watch.yml\`) tells each attempt once and never closes this issue: close it once %s is released or its tag deleted.\n" "$tag"
}

issue_has_link() {
  ask "reading issue #$1" gh api "repos/$GH_REPO/issues/$1" --jq '.body // ""'
  text=$answer
  ask "reading the comments of issue #$1" gh api "repos/$GH_REPO/issues/$1/comments?per_page=100" --jq '.[].body'
  text=$text$answer
  case $text in
    *"]($url/attempts/$n)"*) return 0 ;;
  esac
  return 1
}

find_issues() {
  told_in=
  open_issue=
  ask "listing the issues" gh api "repos/$GH_REPO/issues?state=all&per_page=100" \
    --jq '.[] | select(.pull_request | not) | "\(.number)\t\(.state)\t\(.title)"'
  matches=$(printf '%s\n' "$answer" | awk -F '\t' -v t="$title" '$3 == t { print $1 "\t" $2 }')
  while IFS=$tab read -r number state; do
    [ -n "$number" ] || continue
    if issue_has_link "$number"; then
      told_in=$number
      break
    fi
    if [ "$state" = open ] && [ -z "$open_issue" ]; then
      open_issue=$number
    fi
  done <<EOF
$matches
EOF
}

post() {
  if [ -n "$open_issue" ] && [ "$dry" = 1 ]; then
    printf 'dry run: gh issue comment %s\n' "$open_issue"
    indent "$body"
    result="would comment on #$open_issue"
  elif [ -n "$open_issue" ]; then
    ask_write "commenting on issue #$open_issue" gh issue comment "$open_issue" --body "$body"
    result="told in #$open_issue"
  elif [ "$dry" = 1 ]; then
    printf 'dry run: gh issue create --title "%s"\n' "$title"
    indent "$body"
    result="would open an issue"
  else
    ask_write "opening the issue \"$title\"" gh issue create --title "$title" --body "$body"
    result="opened $answer"
  fi
}

tell() {
  title="Release $tag failed"
  body=$(issue_body)
  find_issues
  if [ -n "$told_in" ]; then
    result="already told in #$told_in, nothing more"
  else
    post
  fi
  printf '%s\n' "$tag's Release run $id $verb on attempt $n$tail: $result"
}

explain_failure() {
  verb='ended in failure'
  steps_text=$(fix_steps 're-run the jobs that did not pass')
  if [ "$broken" -eq 0 ]; then
    tail=', but no job is listed as failed'
    cause='GitHub lists no job of this attempt as failed, so Release watch did not re-run it.'
  elif [ "$dropped" -ne "$broken" ]; then
    tail=', and a job ran and failed'
    cause='A job ran and failed. Release watch re-runs only jobs that no runner took (no step ran).'
  else
    tail=", and no runner took $names in $attempts attempts"
    cause="No runner took these jobs in $attempts attempts. Release watch stops re-running them there."
    steps_text=$(out_steps)
  fi
}

explain_other() {
  tail=
  steps_text=$(fix_steps 're-run its cancelled and failed jobs')
  if [ "$conclusion" = cancelled ]; then
    verb='was cancelled'
    cause='The run was cancelled. Release watch re-runs only a run that failed because no runner took a job.'
  else
    verb="ended in $conclusion"
    cause="The run ended in $conclusion. Release watch re-runs only a run that failed because no runner took a job."
  fi
}

decide() {
  id=$1
  n=$2
  read_attempt "$id" "$n"
  if [ "$conclusion" = success ]; then
    printf '%s\n' "$tag's last Release run ($id) ended in success on attempt $n, yet $tag has no published release: left alone"
    return
  fi
  read_jobs "$id" "$n"
  sum_jobs
  if [ "$conclusion" != failure ]; then
    explain_other
  elif [ "$broken" -gt 0 ] && [ "$dropped" -eq "$broken" ] && [ "$n" -lt "$attempts" ]; then
    rerun "$id"
    printf '%s\n' "re-running the jobs no runner took in $tag's Release run $id (attempt $((n + 1)) of $attempts): $names"
    return
  else
    explain_failure
  fi
  tell
}

by_hand() {
  printf '%s\n' "$tag's last Release run ($id) started from a push of the tag, not from the Version workflow: left to whoever pushed it"
}

pinned() {
  read_runs "$tag"
  read -r id event _ run_attempt <<EOF
$runs
EOF
  if [ -z "$id" ]; then
    say "$tag has no Release run to decide on"
    exit 2
  fi
  if [ "$run_attempt" -lt "$attempt_arg" ]; then
    say "run $id has no attempt $attempt_arg"
    exit 2
  fi
  printf '%s\n' "dry run: deciding on attempt $attempt_arg of $tag's last Release run ($id) as if it had just ended"
  if [ "$event" = push ]; then
    by_hand
  else
    decide "$id" "$attempt_arg"
  fi
}

present() {
  read_releases
  if in_list "$releases" "$tag"; then
    printf '%s\n' "$tag is released: nothing to do"
  elif later_released "$tag"; then
    printf '%s\n' "$tag has no release, but the later $later has: left alone, so $tag cannot become the Latest release"
  else
    present_run
  fi
}

present_run() {
  read_runs "$tag"
  if [ -z "$runs" ]; then
    printf '%s\n' "$tag has no Release run yet: left to the Version workflow, which dispatches it on the next push to main"
  elif unfinished; then
    printf '%s\n' "$tag's Release run has not ended (run $uid, attempt $uattempt, $ustatus): nothing to do yet"
  else
    read -r id event _ run_attempt <<EOF
$runs
EOF
    if [ "$event" = push ]; then
      by_hand
    else
      decide "$id" "$run_attempt"
    fi
  fi
}

read_tags
tag=$(printf '%s\n' "$tag_list" | sed -n 1p)
if [ -n "$tag_arg" ]; then
  in_list "$tag_list" "$tag_arg" || {
    say "no tag $tag_arg in $GH_REPO"
    exit 2
  }
  tag=$tag_arg
  printf '%s\n' "dry run: taking $tag as the newest tag"
fi
if [ -z "$tag" ]; then
  printf '%s\n' "no v* tag yet: nothing to do"
elif [ -n "$attempt_arg" ]; then
  pinned
else
  present
fi
