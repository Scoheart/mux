# MUX repository Rulesets

The JSON files in this directory are auditable request bodies for the GitHub Repository Rulesets API. They are inert until an authorized maintainer sends them to GitHub. Never change `enforcement` to `active` in the committed source files.

`main.json` requires squash PRs, an up-to-date GitHub Actions `verify` result, resolved review conversations, linear history, and immutable branch history. The GitHub Actions App ID `15368` was verified from the live `verify` check rather than inferred from its name.

`tags.json` allows a new `v*` tag to be created, then blocks every update, force-push, and deletion. It deliberately has no `creation` rule and no bypass actor, so Release Please can create a tag but cannot move it later.

## Temporary Fast Lane

The audited window in `../fast-lane.json` temporarily permits direct `main` development through `2026-07-30T08:27:23Z`. The live repository variables `MUX_FAST_LANE_UNTIL` and `MUX_MAIN_RULESET_ID` drive automation; the JSON file records the authorized interval and expected Ruleset ID.

During the window, `release-please.yml` may auto-merge the single Release Please PR only after the exact current PR head passes the GitHub Actions `verify` check. `fast-lane-expiry.yml` uses the dedicated `MUX_RULESET_ADMIN_TOKEN` to stop that automation at the deadline and restore the **MUX main delivery** Ruleset. If restoration fails, it opens a visible repository issue and fails the workflow. The **MUX immutable stable tags** Ruleset remains active throughout and is never modified by Fast Lane automation.

## Authorization boundary

Reading repository settings is safe. Every POST, PATCH, or PUT below changes GitHub and requires explicit authorization for that step. Creating a Ruleset in `evaluate` or `disabled` mode is still an external mutation.

GitHub currently limits `evaluate` mode to Enterprise plans. If GitHub rejects `evaluate`, stop: never retry with `active`. With separate authorization, create the same payload as `disabled`, inspect it, and activate it only after the workflow proof is complete.

## Capture live state

Run from the repository root:

```bash
set -euo pipefail
REPO=Scoheart/mux
EVIDENCE_DIR="${TMPDIR:-/tmp}/mux-rulesets-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$EVIDENCE_DIR"

gh api "repos/$REPO" >"$EVIDENCE_DIR/repository.json"
gh api "repos/$REPO/rulesets" >"$EVIDENCE_DIR/rulesets.json"
gh api "repos/$REPO/commits/main/check-runs?check_name=verify&filter=latest&per_page=100" \
  >"$EVIDENCE_DIR/verify-checks.json"

jq '{allow_squash_merge,allow_merge_commit,allow_rebase_merge,delete_branch_on_merge,squash_merge_commit_title,squash_merge_commit_message}' \
  "$EVIDENCE_DIR/repository.json"
jq '[.check_runs[] | {name,app:{id:.app.id,slug:.app.slug},status,conclusion}]' \
  "$EVIDENCE_DIR/verify-checks.json"
jq 'map({id,name,target,enforcement})' "$EVIDENCE_DIR/rulesets.json"
```

Stop if `verify` is absent, not successful, or not owned by `github-actions` with App ID `15368`. Stop if either committed Ruleset name already exists; update the existing Ruleset deliberately instead of creating a duplicate.

## Create non-enforcing Rulesets

Preferred Enterprise path, after authorization:

```bash
set -euo pipefail
REPO=Scoheart/mux
test "$(gh api "repos/$REPO/rulesets" --jq '[.[] | select(.name == "MUX main delivery" or .name == "MUX immutable stable tags")] | length')" -eq 0
gh api --method POST "repos/$REPO/rulesets" --input .github/rulesets/main.json
gh api --method POST "repos/$REPO/rulesets" --input .github/rulesets/tags.json
```

If and only if the API returns a plan-related validation error for `evaluate`, request authorization for the compatible disabled fallback:

```bash
set -euo pipefail
REPO=Scoheart/mux
TMP_RULESET=$(mktemp -d)
trap 'rm -rf "$TMP_RULESET"' EXIT
jq '.enforcement = "disabled"' .github/rulesets/main.json >"$TMP_RULESET/main.json"
jq '.enforcement = "disabled"' .github/rulesets/tags.json >"$TMP_RULESET/tags.json"
gh api --method POST "repos/$REPO/rulesets" --input "$TMP_RULESET/main.json"
gh api --method POST "repos/$REPO/rulesets" --input "$TMP_RULESET/tags.json"
```

Do not run both paths. After a partial POST failure, re-read the live list before retrying.

## Configure merge behavior

After separate authorization:

```bash
REPO=Scoheart/mux
gh api --method PATCH "repos/$REPO" \
  -F allow_squash_merge=true \
  -F allow_merge_commit=false \
  -F allow_rebase_merge=false \
  -F delete_branch_on_merge=true \
  -f squash_merge_commit_title=PR_TITLE \
  -f squash_merge_commit_message=PR_BODY
```

Verify the effective settings and full Ruleset payloads:

```bash
REPO=Scoheart/mux
gh api "repos/$REPO" \
  --jq '{allow_squash_merge,allow_merge_commit,allow_rebase_merge,delete_branch_on_merge,squash_merge_commit_title,squash_merge_commit_message}'
gh api "repos/$REPO/rulesets" --jq '.[] | [.id,.name,.target,.enforcement] | @tsv'
for id in $(gh api "repos/$REPO/rulesets" --jq '.[].id'); do
  gh api "repos/$REPO/rulesets/$id" \
    --jq '{id,name,target,enforcement,bypass_actors,conditions,rules}'
done
```

## Activate after proof

Observe Rule Insights when `evaluate` is available. Confirm a feature PR, an out-of-date Release PR, and a Release Please update all behave as designed. For the disabled fallback, prepare a disposable documentation PR and keep the rollback commands ready before activation.

Activation is an independent, authorized mutation. The current API updates Rulesets with `PUT`:

```bash
set -euo pipefail
REPO=Scoheart/mux
activate() {
  local name=$1 payload=$2 id tmp
  id=$(gh api "repos/$REPO/rulesets" --jq ".[] | select(.name == \"$name\") | .id")
  test "$(wc -w <<<"$id")" -eq 1
  tmp=$(mktemp)
  jq '.enforcement = "active"' "$payload" >"$tmp"
  gh api --method PUT "repos/$REPO/rulesets/$id" --input "$tmp"
  rm -f "$tmp"
}
activate "MUX main delivery" .github/rulesets/main.json
activate "MUX immutable stable tags" .github/rulesets/tags.json
```

Immediately confirm that direct main updates are blocked, the PR requires `verify`, stale PRs cannot merge, and a new stable tag can still be created by the release workflow.

## Emergency rollback

Disable the Rulesets; do not delete them. This preserves the configuration and audit history:

```bash
set -euo pipefail
REPO=Scoheart/mux
disable() {
  local name=$1 payload=$2 id tmp
  id=$(gh api "repos/$REPO/rulesets" --jq ".[] | select(.name == \"$name\") | .id")
  test "$(wc -w <<<"$id")" -eq 1
  tmp=$(mktemp)
  jq '.enforcement = "disabled"' "$payload" >"$tmp"
  gh api --method PUT "repos/$REPO/rulesets/$id" --input "$tmp"
  rm -f "$tmp"
}
disable "MUX main delivery" .github/rulesets/main.json
disable "MUX immutable stable tags" .github/rulesets/tags.json
```

Re-read both Rulesets and repository merge settings after any rollback. Do not weaken `verify`, move a stable tag, or publish a partial Draft as a recovery shortcut.
