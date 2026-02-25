#!/bin/bash
set -e

# bump version
vim ./Cargo.toml

# update lock file
cargo update -p robotlb
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' ./Cargo.toml | head -n1)

# keep Helm chart metadata in sync with the application release
sed -i -E "s/^version: .*/version: ${VERSION}/" ./helm/Chart.yaml
sed -i -E "s/^appVersion: .*/appVersion: \"${VERSION}\"/" ./helm/Chart.yaml

git cliff -t ${VERSION} -u -p CHANGELOG.md

git add .
git commit -m "release: Version $VERSION"

echo "After merging the PR, tag and release are automatically done"
