#!/usr/bin/env bash
set -euo pipefail

action=${1:?Usage: quality-failure-monitor.sh report|recover}
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"

title=${FAILURE_ISSUE_TITLE:-"[CI] Automated verification is failing"}

find_issue() {
  local state=$1
  gh issue list \
    --repo "$GITHUB_REPOSITORY" \
    --state "$state" \
    --label ci-failure \
    --search "\"$title\" in:title" \
    --limit 1 \
    --json number,state \
    --jq '.[0] | [.number, .state] | @tsv'
}

write_output() {
  local key=$1 value=$2
  if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    echo "$key=$value" >>"$GITHUB_OUTPUT"
  fi
}

case "$action" in
  report)
    : "${GITHUB_SHA:?GITHUB_SHA is required}"
    : "${GITHUB_RUN_ID:?GITHUB_RUN_ID is required}"

    main_sha=$(gh api "repos/$GITHUB_REPOSITORY/commits/main" --jq '.sha')
    if [[ "$GITHUB_SHA" != "$main_sha" ]]; then
      echo "::notice::Skipping failure Issue because $GITHUB_SHA is no longer the main head."
      write_output issue_number ""
      write_output activated false
      exit 0
    fi

    run_url="${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}"
    if [[ "${AUTOFIX_CONFIGURED:-false}" == true ]]; then
      repair_status="Automatic repair is enabled and is dispatched once when this failure cycle is activated."
    else
      repair_status="Automatic repair is inactive because \`COPILOT_PAT\` is not configured. Configure it only if cloud repair is desired."
    fi
    details=$(cat <<EOF
Automated verification is currently failing on the latest \`main\` commit.

- Commit: \`${GITHUB_SHA}\`
- Event: \`${GITHUB_EVENT_NAME:-unknown}\`
- Branch/ref: \`${GITHUB_REF:-unknown}\`
- Workflow run: ${run_url}
- Repair: ${repair_status}

Acceptance criteria:
- Reproduce the failing check from the workflow logs.
- Fix the root cause without weakening the check.
- Add or update focused tests when appropriate.
- Make the full Quality monitor workflow pass.

This is the repository's sticky CI failure Issue. Later failure cycles reopen and update this
Issue instead of creating a new one; superseded commits do not update it.
EOF
    )

    match=$(find_issue all)
    issue=${match%%$'\t'*}
    state=${match#*$'\t'}
    activated=false

    if [[ -z "$match" || -z "$issue" ]]; then
      issue_url=$(gh issue create \
        --repo "$GITHUB_REPOSITORY" \
        --title "$title" \
        --body "$details" \
        --label bug \
        --label automated \
        --label ci-failure)
      issue=${issue_url##*/}
      activated=true
    else
      if [[ "$state" == CLOSED ]]; then
        gh issue reopen "$issue" --repo "$GITHUB_REPOSITORY"
        activated=true
      fi
      gh issue edit "$issue" \
        --repo "$GITHUB_REPOSITORY" \
        --body "$details" \
        --add-label bug \
        --add-label automated \
        --add-label ci-failure
    fi

    write_output issue_number "$issue"
    write_output activated "$activated"
    ;;

  recover)
    match=$(find_issue open)
    issue=${match%%$'\t'*}
    if [[ -n "$match" && -n "$issue" ]]; then
      gh issue close "$issue" --repo "$GITHUB_REPOSITORY" --reason completed
    fi
    ;;

  *)
    echo "Usage: quality-failure-monitor.sh report|recover" >&2
    exit 2
    ;;
esac
