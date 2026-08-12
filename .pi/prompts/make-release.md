Make a release of MiniJinja.

Version: "$ARGUMENTS"

## Step-by-Step Process:

### 1. Determine the target version

The `$ARGUMENTS` should be an explicit SemVer version (e.g., `2.7.0` or `3.0.0-alpha.0`). Pre-releases must include a SemVer pre-release suffix.

If no argument is provided, check the current version in `minijinja/Cargo.toml` and ask the user which version to release.

### 2. Check the changelog

Read `CHANGELOG.md` and verify that:
- There is a section for the new version `$ARGUMENTS`
- The release notes are complete and accurate

If the version section is missing or incomplete, ask the user to update the changelog first.

### 3. Run the version bump script

Execute the bump-version script with the version number:

```bash
./scripts/bump-version.sh $ARGUMENTS
```

This script will:
- Update version in `README.md`
- Update versions in the workspace `Cargo.toml` files
- Update version in `minijinja-py/pyproject.toml`
- Update versions in `minijinja-js/package.json` and `package-lock.json`
- Update version references in examples and MiniJinja-Go
- Run `cargo check --all` to verify everything compiles
- Run `scripts/verify-release.sh` to verify all release versions match

### 4. Run formatting and lint checks

```bash
make format
make lint
```

### 5. Create the release commit

Create a commit with the version changes:

```bash
git add -A
git commit -m "Release $ARGUMENTS"
```

### 6. Create the git tag

```bash
git tag $ARGUMENTS
```

### 7. Show push instructions

After the release is prepared, show the user the commands to push:

```bash
git push origin main && git push origin $ARGUMENTS
```

**Important:** Do NOT automatically push. Let the user review the commit and tag first.

Once the tag is pushed, GitHub Actions will automatically:
- Publish all crates to crates.io (`publish-crates.yml`)
- Create a GitHub Release with built binaries (`release.yml`)
- Publish minijinja-js to npm (`publish-npm.yml`)
- Build and publish minijinja-py wheels to PyPI (`build-wheels.yml`)
- Tag and prewarm the matching MiniJinja-Go module version (`release.yml`)

Do not publish to any registry manually. A failed registry job should be retried from GitHub Actions after diagnosing the failure.

## Notes

- Always use explicit version numbers (e.g., `2.7.0` or `3.0.0-alpha.0`), not release types like `patch`/`minor`/`major`
- Ensure CHANGELOG.md is updated before running this command
- The user should review all changes before pushing
- All publishing is automated via GitHub Actions when the tag is pushed
- GitHub Releases are marked as pre-releases for SemVer pre-release versions
- npm pre-releases use the `next` dist-tag; only stable releases update `latest`
- crates.io, PyPI/pip, and Go tooling do not select pre-release versions for existing stable users by default
