#!/usr/bin/env bash
# Operator/manager runs this after review from the shared checkout.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
if [[ "$root" != "$HOME/koplik" ]]; then
  echo 'Install from the shared ~/koplik checkout after review.' >&2
  exit 1
fi
units=${XDG_CONFIG_HOME:-"$HOME/.config"}/systemd/user
install -d -m 700 "$units" "$HOME/.rsi/koplik-refresh/tmp" "$HOME/.local/lib/koplik-refresh"
install -m 644 "$root/tools/refresh/bootstrap.mjs" "$HOME/.local/lib/koplik-refresh/bootstrap.mjs"
install -m 644 "$root/tools/refresh/koplik-refresh.service" "$root/tools/refresh/koplik-refresh.timer" "$units/"
systemctl --user daemon-reload
systemctl --user enable --now koplik-refresh.timer
systemctl --user list-timers koplik-refresh.timer
