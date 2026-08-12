#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' minijinja/Cargo.toml | head -n 1)"

crate_index_url() {
  local name
  local length

  name="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  length=${#name}

  if [[ $length -eq 1 ]]; then
    printf 'https://index.crates.io/1/%s\n' "$name"
  elif [[ $length -eq 2 ]]; then
    printf 'https://index.crates.io/2/%s\n' "$name"
  elif [[ $length -eq 3 ]]; then
    printf 'https://index.crates.io/3/%s/%s\n' "${name:0:1}" "$name"
  else
    printf 'https://index.crates.io/%s/%s/%s\n' "${name:0:2}" "${name:2:2}" "$name"
  fi
}

wait_for_crate() {
  local crate="$1"
  local index_url
  local index_contents
  local attempt

  index_url="$(crate_index_url "$crate")"
  for attempt in $(seq 1 60); do
    if index_contents="$(curl \
      --user-agent 'minijinja-release-workflow (https://github.com/mitsuhiko/minijinja)' \
      --fail --silent --show-error --location "$index_url")" && \
      grep -Fq "\"vers\":\"$VERSION\"" <<<"$index_contents"
    then
      echo "$crate $VERSION is available in the crates.io index"
      return 0
    fi

    echo "Waiting for $crate $VERSION to reach the crates.io index ($attempt/60)"
    sleep 5
  done

  echo "error: timed out waiting for $crate $VERSION in the crates.io index" >&2
  return 1
}

# Dependent crates cannot be packaged until their exact prerelease dependency is
# visible in the registry index.  Wait for index propagation between layers.
cargo publish -p minijinja
wait_for_crate minijinja

cargo publish -p minijinja-autoreload
cargo publish -p minijinja-embed
cargo publish -p minijinja-contrib
wait_for_crate minijinja-contrib

cargo publish -p minijinja-cli
