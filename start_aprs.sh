#!/bin/bash

set -o errexit
set -o nounset

. "${HOME}/.cargo/env"

HERE="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"

cd "${HERE}"
cargo build --quiet --bin renogy-aprs && \
/usr/bin/daemonize \
  -c /tmp \
  -e /tmp/renogy-aprs.stderr \
  -o /tmp/renogy-aprs.stdout \
  -p /tmp/renogy-aprs.pid \
  $(pwd)/target/debug/renogy-aprs
