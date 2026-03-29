---
name: release
description: Guide the release process for robotlb, including version bumping, Helm chart sync, changelog updates, and creating release branches.
---

## Purpose

Provide step-by-step instructions for releasing a new version of robotlb, ensuring proper versioning, Helm chart sync, changelog updates, and release branch management.

## When to use

Use this skill when asked to:
- Create a new release
- Bump the version
- Prepare a release PR
- Update the changelog for a release

## Prerequisites

Before starting a release:
1. Ensure you are on the `main` branch
2. Ensure the working tree is clean (no uncommitted changes)
3. Ensure local main is up to date with origin/main

## Version Decision Guide

Use Semantic Versioning (MAJOR.MINOR.PATCH). Determine the bump type by analyzing commits since the last release:

### Major Version (X.0.0)

Bump MAJOR when:
- Breaking changes to the CLI interface (removed/renamed commands or flags)
- Breaking changes to configuration format
- Breaking changes to public APIs
- Commit message contains `BREAKING CHANGE:` or `!` (e.g., `feat!: ...`)

### Minor Version (0.X.0)

Bump MINOR when:
- New features added (`feat:` commits)
- New CLI commands or flags
- New configuration options
- Backward-compatible enhancements

### Patch Version (0.0.X)

Bump PATCH when:
- Bug fixes (`fix:` commits)
- Documentation updates (`docs:` commits)
- Internal refactoring (`refactor:` commits)
- Performance improvements without API changes
- Dependency updates

### Decision Process

1. Run: `git log v$(sed -n 's/^version = "\(.*\)"/\1/p' ./Cargo.toml | head -n1)..HEAD --oneline`
2. Check commit messages for:
   - `!` or `BREAKING CHANGE:` -> MAJOR
   - `feat:` -> MINOR
   - `fix:`, `docs:`, `refactor:`, etc. -> PATCH
3. If multiple types, use the highest precedence (MAJOR > MINOR > PATCH)

## Release Process

### Step 1: Verify Clean State

Ensure you're on main with no uncommitted changes and up to date with origin:

```bash
git checkout main
git pull origin main
git status  # Should show "nothing to commit, working tree clean"
```

If there are local commits not on origin/main, they must be merged first.

### Step 2: Determine Version

1. Get current version:
   ```bash
   cat Cargo.toml | grep '^version ='
   ```

2. Review commits since last release:
   ```bash
   git log v<CURRENT_VERSION>..HEAD --oneline
   ```

3. Decide on MAJOR, MINOR, or PATCH bump based on the Version Decision Guide above.

### Step 3: Create Release Branch

Create a branch named `release/v{NEW_VERSION}`:

```bash
git checkout -b release/v<NEW_VERSION>
```

Example: `git checkout -b release/v0.3.0`

### Step 4: Update Version in Cargo.toml

Edit `Cargo.toml` and update the version field:

```toml
version = "<NEW_VERSION>"
```

The version is in the `[package]` section.

### Step 5: Update Cargo.lock

After changing Cargo.toml, update the lock file:

```bash
cargo update -p robotlb
```

### Step 6: Update Helm Chart Metadata

Keep the Helm chart version and appVersion in sync with the application release:

```bash
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' ./Cargo.toml | head -n1)
sed -i -E "s/^version: .*/version: ${VERSION}/" ./helm/Chart.yaml
sed -i -E "s/^appVersion: .*/appVersion: \"${VERSION}\"/" ./helm/Chart.yaml
```

### Step 7: Update Changelog

Generate the changelog using git-cliff:

```bash
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' ./Cargo.toml | head -n1)
git cliff -t ${VERSION} -u -p CHANGELOG.md
```

The changelog will be automatically updated with commits since the last release, grouped by type (Added, Fixed, Documentation, etc.).

### Step 8: Commit Changes

Stage and commit all changes:

```bash
git add .
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' ./Cargo.toml | head -n1)
git commit -m "release: Version $VERSION"
```

### Step 9: Push Branch and Create PR

Push the release branch:

```bash
git push -u origin release/v<NEW_VERSION>
```

Create a pull request to merge into main.

### Step 10: After Merge

After the PR is merged to main:
1. Create and push a tag: `git tag v<VERSION> && git push origin v<VERSION>`
2. The CI workflow automatically builds release artifacts and creates a GitHub Release

## Quick Reference

| Step | Command |
|------|---------|
| Check current version | `grep '^version =' Cargo.toml` |
| View recent commits | `git log v<CUR>..HEAD --oneline` |
| Create branch | `git checkout -b release/v<VER>` |
| Update lock file | `cargo update -p robotlb` |
| Update Helm chart | `sed -i -E "s/^version: .*/version: ${VERSION}/" ./helm/Chart.yaml` |
| Update changelog | `git cliff -t ${VERSION} -u -p CHANGELOG.md` |
| Commit | `git commit -m "release: Version <VER>"` |

## Checklist

- [ ] On main branch, clean working tree
- [ ] Pulled latest from origin/main
- [ ] Determined version bump type (MAJOR/MINOR/PATCH)
- [ ] Created release branch `release/v<VERSION>`
- [ ] Updated version in Cargo.toml
- [ ] Ran `cargo update -p robotlb`
- [ ] Updated Helm chart version and appVersion in `./helm/Chart.yaml`
- [ ] Ran `git cliff -t ${VERSION} -u -p CHANGELOG.md`
- [ ] Committed with message `release: Version <VERSION>`
- [ ] Pushed branch and created PR
- [ ] After merge: created tag `v<VERSION>`
