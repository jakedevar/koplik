#!/usr/bin/env bash
# Execute tests with no network interfaces except loopback. Build/install first.
set -euo pipefail

if (( $# == 0 )); then
  echo "Usage: tools/offline-test.sh <command> [args...]" >&2
  exit 2
fi

gate_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
isolation_error='unshare and ip (iproute2) are required on Linux'
if command -v unshare >/dev/null && command -v ip >/dev/null &&
   isolation_error=$(unshare -rn sh -eu -c 'ip link set lo up' 2>&1); then
  # unshare -r maps the invoking user to uid 0 inside the namespace; permission-based tests may differ there.
  # Probe separately so a failing test is never mistaken for unavailable isolation.
  # All descendants, including browsers and local web servers, inherit the namespace.
  exec unshare -rn sh -eu -c '
    ip link set lo up
    node "$1"
    shift
    exec "$@"
  ' sh "$gate_dir/offline-test-self-test.mjs" "$@"
fi

echo "Offline test isolation unavailable: $isolation_error" >&2
if [[ ${KOPLIK_ALLOW_NETWORK_TESTS:-} == 1 ]]; then
  echo 'WARNING: KOPLIK_ALLOW_NETWORK_TESTS=1; running tests WITH NETWORK ACCESS (isolation self-test skipped).' >&2
  exec "$@"
fi
echo 'Refusing to run tests. Install/enable unprivileged user and network namespaces and iproute2, or explicitly opt out with KOPLIK_ALLOW_NETWORK_TESTS=1.' >&2
exit 1
