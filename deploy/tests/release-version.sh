#!/usr/bin/env bash
# deploy/release-version on a scratch repository, with a stand-in for `gh`
# that answers each commit's pull request author and labels from
# $LABELS_DIR/<commit>.

set -euo pipefail

here=$(cd "$(dirname "$0")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

mkdir -p "$work/repo/deploy" "$work/bin" "$work/labels"
cp "$here/deploy/release-version" "$work/repo/deploy/"
cat > "$work/bin/gh" << 'EOF'
#!/usr/bin/env bash
# gh api repos/<repo>/commits/<commit>/pulls --jq ...: the author, then one label per line.
commit=$(basename "$(dirname "$2")")
cat "$LABELS_DIR/$commit" 2> /dev/null || true
EOF
chmod +x "$work/bin/gh"
export PATH="$work/bin:$PATH" LABELS_DIR="$work/labels" GITHUB_REPOSITORY=example/exchequer

cd "$work/repo"
git init -q -b main
git config user.name test && git config user.email test@example.com
merge() { # a merged pull request by $author with these labels
    git commit -q --allow-empty -m "pr"
    printf '%s\n' "${author:-someone}" "$@" > "$LABELS_DIR/$(git rev-parse HEAD)"
}
expect() {
    got=$(deploy/release-version)
    [ "$got" = "$1" ] || { echo "expected $1, got $got"; exit 1; }
}

merge && git tag -a v0.27.0 -m v0.27.0
expect 0.27.0                  # already tagged: the same version again
merge
expect 0.28.0                  # no label: minor
merge patch
expect 0.28.0                  # the largest bump since the last release
git tag -a v0.28.0 -m v0.28.0
merge patch
expect 0.28.1
merge major patch
expect 1.0.0
git tag -a v1.0.0 -m v1.0.0
author='dependabot[bot]' merge major dependencies
expect 1.1.0                   # dependabot's major label doesn't count
author='dependabot[bot]' merge major patch
expect 1.1.0
echo "release-version: ok"
