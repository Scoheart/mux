#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
  echo "usage: publish-release-assets.sh <vX.Y.Z> <installer> <cli> <updater> <latest.json>" >&2
  exit 64
fi

tag=$1
shift
assets=("$@")
repository=${GITHUB_REPOSITORY:-}

[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "invalid stable tag: $tag" >&2
  exit 64
}
[[ "$repository" =~ ^[^/]+/[^/]+$ ]] || {
  echo "GITHUB_REPOSITORY must be owner/repository" >&2
  exit 64
}

names=()
paths=()
labels=()

for path in "${assets[@]}"; do
  [[ -f "$path" ]] || {
    echo "missing release asset: $path" >&2
    exit 1
  }
  name=$(basename "$path")
  for existing_name in "${names[@]:-}"; do
    [[ "$existing_name" != "$name" ]] || {
      echo "duplicate release asset name: $name" >&2
      exit 1
    }
  done

  case "$name" in
    MUX-Desktop-Installer-"$tag"-macOS-Apple-Silicon.dmg)
      label="Desktop installer · Apple Silicon"
      ;;
    MUX-Desktop-Installer-"$tag"-macOS-Intel.dmg)
      label="Desktop installer · Intel"
      ;;
    mux_"$tag"_aarch64-apple-darwin.tar.gz)
      label="Command-line tool · Apple Silicon"
      ;;
    mux_"$tag"_x86_64-apple-darwin.tar.gz)
      label="Command-line tool · Intel"
      ;;
    MUX-Desktop-AutoUpdate-"$tag"-macOS-Apple-Silicon.app.tar.gz)
      label="Desktop auto-update payload · no manual download"
      ;;
    MUX-Desktop-AutoUpdate-"$tag"-macOS-Intel.app.tar.gz)
      label="Desktop auto-update payload · no manual download"
      ;;
    latest.json)
      label="Auto-update manifest · no manual download"
      ;;
    *)
      echo "unexpected stable release asset: $name" >&2
      exit 1
      ;;
  esac
  names+=("$name")
  paths+=("$path")
  labels+=("$label")
done

required_names=$(printf '%s\n' "${names[@]}" | LC_ALL=C sort)
[[ $(grep -c '^MUX-Desktop-Installer-' <<<"$required_names") -eq 1 ]]
[[ $(grep -c '^mux_' <<<"$required_names") -eq 1 ]]
[[ $(grep -c '^MUX-Desktop-AutoUpdate-' <<<"$required_names") -eq 1 ]]
[[ $(grep -c '^latest\.json$' <<<"$required_names") -eq 1 ]]

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

release_endpoint="repos/$repository/releases/tags/$tag"
release=$(gh api "$release_endpoint")
[[ $(jq -r '.tag_name' <<<"$release") == "$tag" ]]
[[ $(jq -r '.draft' <<<"$release") == true ]] || {
  echo "stable release $tag is not a Draft" >&2
  exit 1
}
[[ $(jq -r '.prerelease' <<<"$release") == false ]] || {
  echo "stable release $tag is marked as a Pre-release" >&2
  exit 1
}
release_id=$(jq -r '.id' <<<"$release")

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
missing=()

for index in "${!names[@]}"; do
  name=${names[$index]}
  path=${paths[$index]}
  existing_id=$(jq -r --arg name "$name" '.assets[] | select(.name == $name) | .id' <<<"$release")
  if [[ -z "$existing_id" ]]; then
    missing+=("$path")
    continue
  fi

  downloaded="$tmp/$name"
  gh api "repos/$repository/releases/assets/$existing_id" \
    -H "Accept: application/octet-stream" >"$downloaded"
  if [[ $(sha256 "$path") != $(sha256 "$downloaded") ]]; then
    echo "existing asset differs and will not be overwritten: $name" >&2
    exit 1
  fi
done

if [[ ${#missing[@]} -gt 0 ]]; then
  gh release upload "$tag" "${missing[@]}" --repo "$repository"
fi

release=$(gh api "$release_endpoint")
actual_names=$(jq -r '.assets[].name' <<<"$release" | LC_ALL=C sort)
[[ "$actual_names" == "$required_names" ]] || {
  echo "Draft asset set does not match the required set" >&2
  exit 1
}

for index in "${!names[@]}"; do
  name=${names[$index]}
  asset_id=$(jq -r --arg name "$name" '.assets[] | select(.name == $name) | .id' <<<"$release")
  [[ -n "$asset_id" ]]
  gh api --method PATCH "repos/$repository/releases/assets/$asset_id" \
    -f "label=${labels[$index]}" --silent
done

release=$(gh api "$release_endpoint")
actual_names=$(jq -r '.assets[].name' <<<"$release" | LC_ALL=C sort)
[[ "$actual_names" == "$required_names" ]]
for index in "${!names[@]}"; do
  name=${names[$index]}
  expected_label=${labels[$index]}
  actual_label=$(jq -r --arg name "$name" '.assets[] | select(.name == $name) | .label // ""' <<<"$release")
  [[ "$actual_label" == "$expected_label" ]] || {
    echo "asset label verification failed: $name" >&2
    exit 1
  }
done

gh api --method PATCH "repos/$repository/releases/$release_id" \
  -F draft=false --silent
echo "Published stable release $tag after verifying all assets."
