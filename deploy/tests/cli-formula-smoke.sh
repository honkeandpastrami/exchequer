#!/bin/sh
# cli-formula-smoke.sh: install the CLI's Homebrew formula from a tarball of
# HEAD, through a throwaway local tap, and check it runs and passes audit.
#
#   deploy/tests/cli-formula-smoke.sh
#
# Uninstalls it and removes the tap afterwards. Needs Homebrew.

set -eu

cd "$(dirname "$0")/../.."
export HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_FROM_API=1
tap=local/exchequer-smoke
formula=$tap/exchequer-cli

tarball=$(deploy/build-python-source)
brew tap-new --no-git "$tap" > /dev/null
trap 'brew uninstall --formula "$formula" > /dev/null 2>&1 || true; brew untap "$tap" > /dev/null' EXIT
deploy/exchequer-cli-formula "$tarball" "file://$PWD/$tarball" 0.0.1 \
    > "$(brew --repository "$tap")/Formula/exchequer-cli.rb"

brew install --formula "$formula"
brew test "$formula"
brew audit --formula "$formula"
