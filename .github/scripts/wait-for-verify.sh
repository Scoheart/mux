#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: wait-for-verify.sh <owner/repository> <commit-sha>" >&2
  exit 64
fi

repository=$1
sha=$2
attempts=${WAIT_FOR_VERIFY_ATTEMPTS:-60}
interval=${WAIT_FOR_VERIFY_INTERVAL:-10}

[[ "$repository" =~ ^[^/]+/[^/]+$ ]] || {
  echo "invalid repository: $repository" >&2
  exit 64
}
[[ "$sha" =~ ^[0-9a-fA-F]{6,40}$ ]] || {
  echo "invalid commit SHA: $sha" >&2
  exit 64
}
[[ "$attempts" =~ ^[1-9][0-9]*$ ]] || exit 64
[[ "$interval" =~ ^[0-9]+$ ]] || exit 64

endpoint="repos/$repository/commits/$sha/check-runs?check_name=verify&filter=latest&per_page=100"

for ((attempt = 1; attempt <= attempts; attempt += 1)); do
  response=$(gh api "$endpoint")
  matches=$(jq --arg sha "$sha" '[
    .check_runs[]
    | select(.name == "verify")
    | select(.head_sha == $sha)
    | select(.app.slug == "github-actions")
  ]' <<<"$response")
  count=$(jq 'length' <<<"$matches")

  if [[ "$count" -gt 1 ]]; then
    echo "ambiguous verify checks for $sha" >&2
    exit 1
  fi

  if [[ "$count" -eq 1 ]]; then
    status=$(jq -r '.[0].status' <<<"$matches")
    conclusion=$(jq -r '.[0].conclusion // ""' <<<"$matches")
    if [[ "$status" == completed ]]; then
      if [[ "$conclusion" == success ]]; then
        echo "verify succeeded for $sha"
        exit 0
      fi
      echo "verify completed with $conclusion for $sha" >&2
      exit 1
    fi
  fi

  if [[ "$attempt" -lt "$attempts" ]]; then
    sleep "$interval"
  fi
done

echo "verify did not succeed for $sha before the timeout" >&2
exit 1
