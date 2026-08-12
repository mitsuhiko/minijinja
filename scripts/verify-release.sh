#!/bin/bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <version>" >&2
  exit 2
fi

VERSION="$1"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]]; then
  echo "error: '$VERSION' is not a supported release version" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

failed=0

check_line() {
  local path="$1"
  local expected="$2"

  if ! grep -Fqx "$expected" "$path"; then
    echo "error: $path does not contain: $expected" >&2
    failed=1
  fi
}

for manifest in \
  minijinja/Cargo.toml \
  minijinja-autoreload/Cargo.toml \
  minijinja-cabi/Cargo.toml \
  minijinja-cli/Cargo.toml \
  minijinja-contrib/Cargo.toml \
  minijinja-embed/Cargo.toml \
  minijinja-js/Cargo.toml \
  minijinja-py/Cargo.toml
do
  check_line "$manifest" "version = \"$VERSION\""
done

check_line minijinja-py/pyproject.toml "version = \"$VERSION\""
check_line minijinja-go/version.go "const Version = \"$VERSION\""
check_line CHANGELOG.md "## $VERSION"

if ! grep -Fq "minijinja v$VERSION (minijinja)" README.md; then
  echo "error: README.md does not contain the release version $VERSION" >&2
  failed=1
fi

for package_file in minijinja-js/package.json minijinja-js/package-lock.json; do
  if ! grep -Eq "^[[:space:]]*\"version\": \"$VERSION\",?$" "$package_file"; then
    echo "error: $package_file does not contain the release version $VERSION" >&2
    failed=1
  fi
done

while IFS= read -r dependency; do
  if [[ "$dependency" != *"\"$VERSION\""* && "$dependency" != *"\"=$VERSION\""* ]]; then
    echo "error: stale MiniJinja dependency version: $dependency" >&2
    failed=1
  fi
done < <(
  find minijinja-* examples -name Cargo.toml -type f -exec \
    grep -HnE '^minijinja[^=]*=.*version = "' {} + || true
)

if [[ $failed -ne 0 ]]; then
  exit 1
fi

echo "Release version $VERSION is consistent."
