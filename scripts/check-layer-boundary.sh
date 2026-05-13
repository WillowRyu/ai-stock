#!/usr/bin/env bash
# DDD layer-boundary enforcement.
# - crates/domain must not depend on tokio, reqwest, sqlx, tauri, keyring (it's pure).
# - crates/application must not depend on reqwest, sqlx, keyring, tauri (no IO/UI).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

check_no_dep() {
  local manifest="$1"
  local forbidden="$2"
  if grep -E "^${forbidden}(\s|\.|=)" "$manifest" > /dev/null; then
    echo "FAIL: ${manifest} contains forbidden direct dependency '${forbidden}'"
    return 1
  fi
}

FAIL=0

DOMAIN_MANIFEST="${ROOT}/crates/domain/Cargo.toml"
for f in tokio reqwest sqlx tauri keyring; do
  check_no_dep "${DOMAIN_MANIFEST}" "${f}" || FAIL=1
done

APP_MANIFEST="${ROOT}/crates/application/Cargo.toml"
for f in reqwest sqlx keyring tauri; do
  check_no_dep "${APP_MANIFEST}" "${f}" || FAIL=1
done

if [ "${FAIL}" -eq 0 ]; then
  echo "Layer boundary OK"
fi
exit $FAIL
