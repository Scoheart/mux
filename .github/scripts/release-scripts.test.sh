#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/bin" "$TMP/assets"

cat >"$TMP/bin/gh" <<'FAKE_GH'
#!/usr/bin/env bash
set -euo pipefail

checks() {
  case "$FAKE_SCENARIO" in
    verify-success)
      echo '{"check_runs":[{"name":"verify","head_sha":"abc123","status":"completed","conclusion":"success","app":{"slug":"github-actions"}}]}'
      ;;
    verify-pending)
      count=0
      [[ -f "$FAKE_STATE/count" ]] && count=$(cat "$FAKE_STATE/count")
      count=$((count + 1))
      echo "$count" >"$FAKE_STATE/count"
      if [[ "$count" -eq 1 ]]; then
        echo '{"check_runs":[{"name":"verify","head_sha":"abc123","status":"in_progress","conclusion":null,"app":{"slug":"github-actions"}}]}'
      else
        echo '{"check_runs":[{"name":"verify","head_sha":"abc123","status":"completed","conclusion":"success","app":{"slug":"github-actions"}}]}'
      fi
      ;;
    verify-failure)
      echo '{"check_runs":[{"name":"verify","head_sha":"abc123","status":"completed","conclusion":"failure","app":{"slug":"github-actions"}}]}'
      ;;
    verify-wrong-app)
      echo '{"check_runs":[{"name":"verify","head_sha":"abc123","status":"completed","conclusion":"success","app":{"slug":"external-ci"}}]}'
      ;;
    *) echo '{"check_runs":[]}' ;;
  esac
}

asset_json() {
  jq -n \
    --arg n1 "$FAKE_ASSET_1" --arg n2 "$FAKE_ASSET_2" \
    --arg n3 "$FAKE_ASSET_3" --arg n4 "$FAKE_ASSET_4" \
    '[
      {id:1,name:$n1,label:"Desktop installer · Apple Silicon"},
      {id:2,name:$n2,label:"Command-line tool · Apple Silicon"},
      {id:3,name:$n3,label:"Desktop auto-update payload · no manual download"},
      {id:4,name:$n4,label:"Auto-update manifest · no manual download"}
    ]'
}

release_json() {
  local draft=$1 assets=$2
  jq -n --arg tag "v1.2.18" --argjson draft "$draft" --argjson assets "$assets" \
    '{id:99,tag_name:$tag,draft:$draft,prerelease:false,assets:$assets}'
}

if [[ "$1" == api && "$2" == repos/*/commits/*/check-runs* ]]; then
  checks
elif [[ "$1" == api && "$2" == repos/*/releases/tags/* ]]; then
  case "$FAKE_SCENARIO" in
    publish-no-draft) release_json false '[]' ;;
    publish-missing)
      if [[ -f "$FAKE_STATE/uploaded" ]]; then
        names=()
        while IFS= read -r name; do
          names+=("$name")
        done <"$FAKE_STATE/uploaded"
        FAKE_ASSET_1=${names[0]} FAKE_ASSET_2=${names[1]} \
          FAKE_ASSET_3=${names[2]} FAKE_ASSET_4=${names[3]} \
          release_json true "$(FAKE_ASSET_1=${names[0]} FAKE_ASSET_2=${names[1]} FAKE_ASSET_3=${names[2]} FAKE_ASSET_4=${names[3]} asset_json)"
      else
        release_json true '[]'
      fi
      ;;
    *) release_json true "$(asset_json)" ;;
  esac
elif [[ "$1" == api && "$2" == repos/*/releases/assets/* ]]; then
  id=${2##*/}
  if [[ "$FAKE_SCENARIO" == publish-different && "$id" == 1 ]]; then
    printf 'different bytes'
  else
    variable="FAKE_ASSET_PATH_${id}"
    cat "${!variable}"
  fi
elif [[ "$1" == api && "$2" == --method && "$3" == PATCH && "$4" == repos/*/releases/assets/* ]]; then
  id=${4##*/}
  echo "label:$id" >>"$FAKE_STATE/log"
  if [[ "$FAKE_SCENARIO" == publish-label-failure && "$id" == 2 ]]; then
    exit 1
  fi
elif [[ "$1" == api && "$2" == --method && "$3" == PATCH && "$4" == repos/*/releases/* ]]; then
  echo publish >>"$FAKE_STATE/log"
elif [[ "$1" == release && "$2" == upload ]]; then
  : >"$FAKE_STATE/uploaded"
  shift 3
  while [[ $# -gt 0 && "$1" != --repo ]]; do
    basename "$1" >>"$FAKE_STATE/uploaded"
    echo "upload:$(basename "$1")" >>"$FAKE_STATE/log"
    shift
  done
else
  echo "unexpected fake gh invocation: $*" >&2
  exit 64
fi
FAKE_GH
chmod +x "$TMP/bin/gh"

export PATH="$TMP/bin:$PATH"
export FAKE_STATE="$TMP/state"
export GITHUB_REPOSITORY="Scoheart/mux"
mkdir -p "$FAKE_STATE"

expect_failure() {
  if "$@" >/dev/null 2>&1; then
    echo "expected command to fail: $*" >&2
    exit 1
  fi
}

FAKE_SCENARIO=verify-success \
  WAIT_FOR_VERIFY_ATTEMPTS=2 WAIT_FOR_VERIFY_INTERVAL=0 \
  bash "$ROOT/.github/scripts/wait-for-verify.sh" "$GITHUB_REPOSITORY" abc123

rm -f "$FAKE_STATE/count"
FAKE_SCENARIO=verify-pending \
  WAIT_FOR_VERIFY_ATTEMPTS=2 WAIT_FOR_VERIFY_INTERVAL=0 \
  bash "$ROOT/.github/scripts/wait-for-verify.sh" "$GITHUB_REPOSITORY" abc123

for scenario in verify-failure verify-missing verify-wrong-app; do
  expect_failure env FAKE_SCENARIO="$scenario" \
    WAIT_FOR_VERIFY_ATTEMPTS=2 WAIT_FOR_VERIFY_INTERVAL=0 \
    bash "$ROOT/.github/scripts/wait-for-verify.sh" "$GITHUB_REPOSITORY" abc123
done

TAG=v1.2.18
FAKE_ASSET_1="MUX-Desktop-Installer-$TAG-macOS-Apple-Silicon.dmg"
FAKE_ASSET_2="mux_${TAG}_aarch64-apple-darwin.tar.gz"
FAKE_ASSET_3="MUX-Desktop-AutoUpdate-$TAG-macOS-Apple-Silicon.app.tar.gz"
FAKE_ASSET_4=latest.json
export FAKE_ASSET_1 FAKE_ASSET_2 FAKE_ASSET_3 FAKE_ASSET_4

for index in 1 2 3 4; do
  name_variable="FAKE_ASSET_${index}"
  path="$TMP/assets/${!name_variable}"
  printf 'asset %s\n' "$index" >"$path"
  export "FAKE_ASSET_PATH_${index}=$path"
done

assets=(
  "$FAKE_ASSET_PATH_1"
  "$FAKE_ASSET_PATH_2"
  "$FAKE_ASSET_PATH_3"
  "$FAKE_ASSET_PATH_4"
)

: >"$FAKE_STATE/log"
FAKE_SCENARIO=publish-missing \
  bash "$ROOT/.github/scripts/publish-release-assets.sh" "$TAG" "${assets[@]}"
[[ $(grep -c '^upload:' "$FAKE_STATE/log") -eq 4 ]]
[[ $(grep -c '^label:' "$FAKE_STATE/log") -eq 4 ]]
[[ $(tail -n 1 "$FAKE_STATE/log") == publish ]]

: >"$FAKE_STATE/log"
rm -f "$FAKE_STATE/uploaded"
FAKE_SCENARIO=publish-identical \
  bash "$ROOT/.github/scripts/publish-release-assets.sh" "$TAG" "${assets[@]}"
! grep -q '^upload:' "$FAKE_STATE/log"
[[ $(tail -n 1 "$FAKE_STATE/log") == publish ]]

for scenario in publish-different publish-no-draft publish-label-failure; do
  : >"$FAKE_STATE/log"
  expect_failure env FAKE_SCENARIO="$scenario" \
    bash "$ROOT/.github/scripts/publish-release-assets.sh" "$TAG" "${assets[@]}"
  ! grep -q '^publish$' "$FAKE_STATE/log"
done

echo "Release helper tests passed."
