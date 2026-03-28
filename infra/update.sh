#!/usr/bin/env bash
# infra/update.sh - Regenerate and install observability configs, then restart services.
#
# Generates environment-aware Prometheus config via generate-config.sh,
# installs it and the latest Grafana dashboards to the config directory
# used by the running services, and restarts both via launchctl.
#
# This never modifies files in the repo - it writes to the installed
# config directory (~/Library/Application Support/agentd/).
#
# Prerequisites:
#   - infra/setup.sh must have been run at least once
#   - Prometheus and Grafana launchd agents must be registered
#
# Environment variables (passed through to generate-config.sh):
#   AGENTD_ENV                        - "production" or "development" (default: development)
#   AGENTD_ORCHESTRATOR_SERVICE_URL   - Override orchestrator target
#   AGENTD_COMMUNICATE_SERVICE_URL    - Override communicate target
#   AGENTD_NOTIFY_SERVICE_URL         - Override notify target
#   AGENTD_ASK_SERVICE_URL            - Override ask target
#   AGENTD_WRAP_SERVICE_URL           - Override wrap target
#   AGENTD_MEMORY_SERVICE_URL         - Override memory target
#   AGENTD_MONITOR_SERVICE_URL        - Override monitor target
#
# Usage:
#   ./infra/update.sh                       # update with current env settings
#   AGENTD_ENV=production ./infra/update.sh # switch to production ports
#   ./infra/update.sh --dry-run             # show what would be done

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration (must match setup.sh)
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
INFRA_DIR="$REPO_ROOT/infra"

CONFIG_DIR="$HOME/Library/Application Support/agentd"
LAUNCH_AGENTS_DIR="$HOME/Library/LaunchAgents"
PROMETHEUS_PLIST="com.agentd.prometheus.plist"
GRAFANA_PLIST="com.agentd.grafana.plist"

DRY_RUN=false

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

info()  { echo "  [INFO]  $*"; }
ok()    { echo "  [OK]    $*"; }
warn()  { echo "  [WARN]  $*" >&2; }
error() { echo "  [ERROR] $*" >&2; exit 1; }
step()  { echo; echo "==> $*"; }

is_loaded() {
    launchctl list 2>/dev/null | grep -q "$1"
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

for arg in "$@"; do
    case "$arg" in
        --dry-run)  DRY_RUN=true ;;
        --help|-h)
            grep '^#' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        *) warn "Unknown argument: $arg" ;;
    esac
done

[[ "$DRY_RUN" == true ]] && info "Dry-run mode - no changes will be made."

# ---------------------------------------------------------------------------
# Preflight checks
# ---------------------------------------------------------------------------

step "Checking prerequisites"

PROM_CONFIG_DST="$CONFIG_DIR/prometheus/prometheus.yml"
DASHBOARDS_DST="$CONFIG_DIR/grafana/dashboards"

if [[ ! -d "$CONFIG_DIR" ]]; then
    error "Config directory not found: $CONFIG_DIR - run infra/setup.sh first."
fi

if [[ ! -d "$DASHBOARDS_DST" ]]; then
    error "Dashboard directory not found: $DASHBOARDS_DST - run infra/setup.sh first."
fi

ok "Config directory exists: $CONFIG_DIR"

# ---------------------------------------------------------------------------
# Step 1: Generate Prometheus config to a temp file
# ---------------------------------------------------------------------------

step "Generating Prometheus configuration (AGENTD_ENV=${AGENTD_ENV:-development})"

TMPFILE="$(mktemp "${TMPDIR:-/tmp}/prometheus.yml.XXXXXX")"
# Ensure cleanup on exit regardless of success or failure
trap 'rm -f "$TMPFILE"' EXIT

"$INFRA_DIR/prometheus/generate-config.sh" "$TMPFILE"

ok "Generated config to temp file"

if [[ "$DRY_RUN" == true ]]; then
    info "[DRY] Would install $TMPFILE → $PROM_CONFIG_DST"
    info "Generated config preview:"
    head -30 "$TMPFILE" | sed 's/^/        /'
else
    cp "$TMPFILE" "$PROM_CONFIG_DST"
    ok "Installed: $PROM_CONFIG_DST"
fi

# ---------------------------------------------------------------------------
# Step 2: Install latest Grafana dashboards
# ---------------------------------------------------------------------------

step "Installing Grafana dashboards"

DASHBOARDS_SRC="$INFRA_DIR/grafana/dashboards"

if [[ ! -d "$DASHBOARDS_SRC" ]]; then
    error "Dashboard source directory not found: $DASHBOARDS_SRC"
fi

DASH_COUNT=0
for dashboard in "$DASHBOARDS_SRC"/*.json; do
    [[ -f "$dashboard" ]] || continue
    DASH_COUNT=$((DASH_COUNT + 1))
    BASENAME="$(basename "$dashboard")"
    if [[ "$DRY_RUN" == true ]]; then
        info "[DRY] Would copy $BASENAME"
    else
        cp "$dashboard" "$DASHBOARDS_DST/"
    fi
done

if [[ "$DRY_RUN" == true ]]; then
    ok "[DRY] Would install $DASH_COUNT dashboard(s) to $DASHBOARDS_DST"
else
    ok "Installed $DASH_COUNT dashboard(s) to $DASHBOARDS_DST"
fi

# ---------------------------------------------------------------------------
# Step 3: Restart Prometheus
# ---------------------------------------------------------------------------

step "Restarting Prometheus"

PROM_PLIST_PATH="$LAUNCH_AGENTS_DIR/$PROMETHEUS_PLIST"

if [[ ! -f "$PROM_PLIST_PATH" ]]; then
    warn "Prometheus plist not found at $PROM_PLIST_PATH - skipping restart."
    warn "Run infra/setup.sh to register the launchd agent."
elif [[ "$DRY_RUN" == true ]]; then
    info "[DRY] Would restart Prometheus via launchctl"
else
    if is_loaded "com.agentd.prometheus"; then
        launchctl unload "$PROM_PLIST_PATH" 2>/dev/null || true
        info "Stopped Prometheus"
    fi
    launchctl load "$PROM_PLIST_PATH"
    ok "Prometheus restarted"
fi

# ---------------------------------------------------------------------------
# Step 4: Restart Grafana
# ---------------------------------------------------------------------------

step "Restarting Grafana"

GRAFANA_PLIST_PATH="$LAUNCH_AGENTS_DIR/$GRAFANA_PLIST"

if [[ ! -f "$GRAFANA_PLIST_PATH" ]]; then
    warn "Grafana plist not found at $GRAFANA_PLIST_PATH - skipping restart."
    warn "Run infra/setup.sh to register the launchd agent."
elif [[ "$DRY_RUN" == true ]]; then
    info "[DRY] Would restart Grafana via launchctl"
else
    if is_loaded "com.agentd.grafana"; then
        launchctl unload "$GRAFANA_PLIST_PATH" 2>/dev/null || true
        info "Stopped Grafana"
    fi
    launchctl load "$GRAFANA_PLIST_PATH"
    ok "Grafana restarted"
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------

echo
echo "============================================================"
echo " agentd observability stack updated"
echo "============================================================"
echo
echo "  Prometheus config: $PROM_CONFIG_DST"
echo "  Dashboards:        $DASHBOARDS_DST"
echo "  Environment:       ${AGENTD_ENV:-development}"
echo
if [[ "$DRY_RUN" == true ]]; then
    echo "  (Dry-run - no changes were made)"
    echo
fi
