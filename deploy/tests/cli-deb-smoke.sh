#!/bin/sh
# cli-deb-smoke.sh: install a built CLI package on Debian 13 and check it runs.
#
#   deploy/tests/cli-deb-smoke.sh DEB
#
# Uses docker unless CONTAINER is set (e.g. CONTAINER=podman).

set -eu

deb=$(realpath "$1")
run=${CONTAINER:-docker}

$run run --rm -v "$deb:/tmp/$(basename "$deb"):ro" docker.io/library/debian:trixie sh -euc '
    apt-get update -q
    apt-get install -yq --no-install-recommends /tmp/*.deb
    exchequer --help
    dpkg -s exchequer-cli | grep "^Version:"
'
