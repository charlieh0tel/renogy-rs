#!/bin/bash

set -o errexit
set -o nounset

. "${HOME}/.cargo/env"

HERE="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"

cd "${HERE}"
cargo build --quiet --bin renogy-bms-collector && \
/usr/bin/daemonize \
  -c /tmp \
  -e /tmp/renogy-bms-collector.stderr \
  -o /tmp/renogy-bms-collector.stdout \
  -p /tmp/renogy-bms-collector.pid \
  $(pwd)/target/debug/renogy-bms-collector bt2
