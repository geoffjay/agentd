#!/usr/bin/env bash
# infra/teardown.sh — Remove agentd observability stack from macOS.
#
# Unloads launchd services, removes plists from ~/Library/LaunchAgents,
# and optionally removes state data (logs, Grafana DB, Prometheus TSDB).
#
# Usage:
#   ./infra/teardown.sh              # unload services, remove plists
#   ./infra/teardown.sh --purge      # also delete all state/data directories
#   ./infra/teardown.sh --dry-run    # show what would be done
#
# This script is idempotent — safe to run multiple times.

set -euo pipefail

LAUNCH_AGENTS_DIR="$HOME/Library/LaunchAgents"
PROMETHEUS_PLIST="com.agentd.prometheus.plist"
GRAFANA_PLIST="com.agentd.grafana.plist"

STATE_DIR="/Users/Shared/agentd"
LOG_DIR="$STATE_DIR/logs"
GRAFANA_DATA_DIR="$STATE_DIR/grafana-data"
PROMETHEUS_DATA_DIR="$STATE_DIR/prometheus-data"

CONFIG_DIR="$HOME/Library/Application Support/agentd"

DRY_RUN=false
PURGE=false

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

info()  { echo "  [INFO]  $*"; }
ok()    { echo "  [OK]    $*"; }
warn()  { echo "  [WARN]  $*" >&2; }
step()  { echo; echo "==> $*"; }

run() {
    if [[ "$DRY_RUN" == true ]]; then
        echo "  [DRY]   $*"
    else
        "$@"
    fi
}

unload_service() {
    local label="$1"
    local plist="$LAUNCH_AGENTS_DIR/$2"

    if launchctl list | grep -q "$label" 2>/dev/null; then
        info "Unloading $label..."
        run launchctl unload "$plist" 2>/dev/null || true
        ok "Unloaded: $label"
    else
        ok "Not running: $label"
    fi
}

remove_plist() {
    local plist="$LAUNCH_AGENTS_DIR/$1"
    if [[ -f "$plist" ]]; then
        info "Removing plist: $plist"
        run rm -f "$plist"
        ok "Removed: $plist"
    else
        ok "Not found: $plist"
    fi
}

remove_dir() {
    local dir="$1"
    if [[ -d "$dir" ]]; then
        info "Removing directory: $dir"
        run rm -rf "$dir"
        ok "Removed: $dir"
    else
        ok "Not found: $dir"
    fi
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --purge)   PURGE=true ;;
        --help|-h)
            grep '^#' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        *) warn "Unknown argument: $arg" ;;
    esac
done

[[ "$DRY_RUN" == true ]] && info "Dry-run mode — no changes will be made."

# ---------------------------------------------------------------------------
# Step 1: Unload services
# ---------------------------------------------------------------------------

step "Unloading launchd services"

unload_service "com.agentd.prometheus" "$PROMETHEUS_PLIST"
unload_service "com.agentd.grafana"    "$GRAFANA_PLIST"

# ---------------------------------------------------------------------------
# Step 2: Remove plists
# ---------------------------------------------------------------------------

step "Removing launchd plists from ~/Library/LaunchAgents"

remove_plist "$PROMETHEUS_PLIST"
remove_plist "$GRAFANA_PLIST"

# ---------------------------------------------------------------------------
# Step 3: Remove installed config files
# ---------------------------------------------------------------------------

step "Removing installed config directory"

if [[ -d "$CONFIG_DIR" ]]; then
    info "Removing: $CONFIG_DIR"
    run rm -rf "$CONFIG_DIR"
    ok "Removed: $CONFIG_DIR"
else
    ok "Not found: $CONFIG_DIR"
fi

# ---------------------------------------------------------------------------
# Step 4: Optionally purge state data
# ---------------------------------------------------------------------------

if [[ "$PURGE" == true ]]; then
    step "Purging state data (--purge flag set)"
    warn "This will delete all Prometheus metrics history and Grafana configuration."
    echo
    if [[ "$DRY_RUN" == false ]]; then
        read -r -p "  Are you sure? [y/N] " confirm
        if [[ "${confirm,,}" != "y" ]]; then
            info "Purge cancelled."
            PURGE=false
        fi
    fi

    if [[ "$PURGE" == true ]]; then
        remove_dir "$LOG_DIR"
        remove_dir "$GRAFANA_DATA_DIR"
        remove_dir "$PROMETHEUS_DATA_DIR"
    fi
else
    info "State data preserved at: $STATE_DIR"
    info "Run with --purge to also remove logs and data."
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------

echo
echo "============================================================"
echo " agentd observability stack removed"
echo "============================================================"
echo
echo "  Prometheus and Grafana binaries are still installed."
echo "  To uninstall them: brew uninstall prometheus grafana"
echo
if [[ "$DRY_RUN" == true ]]; then
    echo "  (Dry-run — no changes were made)"
    echo
fi
