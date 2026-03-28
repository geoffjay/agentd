#!/usr/bin/env bash
# infra/setup.sh — One-command local observability stack setup for agentd.
#
# Installs and configures Prometheus + Grafana on macOS, sets up the agentd
# scrape config and dashboards, and registers both services as launchd agents
# so they start automatically at login.
#
# Usage:
#   ./infra/setup.sh              # full install
#   ./infra/setup.sh --dry-run    # show what would be done without doing it
#   ./infra/setup.sh --uninstall  # remove services (see infra/teardown.sh)
#
# Requirements:
#   - macOS (tested on macOS 13+)
#   - Homebrew (https://brew.sh)
#
# Idempotent: safe to run multiple times. Already-installed components are
# detected and skipped.

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
INFRA_DIR="$REPO_ROOT/infra"

# Shared state directory (writable by all users — avoids home-dir path issues
# in launchd plists which run before user home is mounted on some macOS versions)
STATE_DIR="/Users/Shared/agentd"
LOG_DIR="$STATE_DIR/logs"
GRAFANA_DATA_DIR="$STATE_DIR/grafana-data"
PROMETHEUS_DATA_DIR="$STATE_DIR/prometheus-data"

CONFIG_DIR="$HOME/Library/Application Support/agentd"

LAUNCH_AGENTS_DIR="$HOME/Library/LaunchAgents"
PROMETHEUS_PLIST="com.agentd.prometheus.plist"
GRAFANA_PLIST="com.agentd.grafana.plist"

PROMETHEUS_PORT=9090
GRAFANA_PORT=3000

DRY_RUN=false

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

info()    { echo "  [INFO]  $*"; }
ok()      { echo "  [OK]    $*"; }
warn()    { echo "  [WARN]  $*" >&2; }
error()   { echo "  [ERROR] $*" >&2; exit 1; }
step()    { echo; echo "==> $*"; }

run() {
    if [[ "$DRY_RUN" == true ]]; then
        echo "  [DRY]   $*"
    else
        "$@"
    fi
}

require_brew() {
    if ! command -v brew &>/dev/null; then
        error "Homebrew is required but not installed. Install it from https://brew.sh then re-run this script."
    fi
}

brew_prefix() {
    brew --prefix 2>/dev/null
}

is_loaded() {
    launchctl list | grep -q "$1" 2>/dev/null
}

wait_for_port() {
    local port="$1"
    local name="$2"
    local max=30
    local count=0
    printf "  [WAIT]  Waiting for %s on port %d" "$name" "$port"
    while ! curl -sf "http://127.0.0.1:$port" &>/dev/null && [[ $count -lt $max ]]; do
        printf "."
        sleep 1
        ((count++)) || true
    done
    echo
    if [[ $count -ge $max ]]; then
        warn "$name did not start within ${max}s — check logs at $LOG_DIR"
        return 1
    fi
    return 0
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

for arg in "$@"; do
    case "$arg" in
        --dry-run)   DRY_RUN=true ;;
        --uninstall) exec "$SCRIPT_DIR/teardown.sh" "$@" ;;
        --help|-h)
            grep '^#' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        *) warn "Unknown argument: $arg" ;;
    esac
done

[[ "$DRY_RUN" == true ]] && info "Dry-run mode — no changes will be made."

# ---------------------------------------------------------------------------
# Step 1: Homebrew check
# ---------------------------------------------------------------------------

step "Checking dependencies"
require_brew

BREW_PREFIX="$(brew_prefix)"
info "Homebrew prefix: $BREW_PREFIX"

# ---------------------------------------------------------------------------
# Step 2: Install Prometheus and Grafana
# ---------------------------------------------------------------------------

step "Installing Prometheus and Grafana via Homebrew"

if command -v prometheus &>/dev/null; then
    ok "Prometheus already installed: $(prometheus --version 2>&1 | head -1)"
else
    info "Installing Prometheus..."
    run brew install prometheus
fi

if command -v grafana &>/dev/null; then
    ok "Grafana already installed: $(grafana --version 2>&1 | head -1)"
else
    info "Installing Grafana..."
    run brew install grafana
fi

# ---------------------------------------------------------------------------
# Step 3: Create state directories
# ---------------------------------------------------------------------------

step "Creating state, log, and config directories"

for dir in \
    "$LOG_DIR" \
    "$GRAFANA_DATA_DIR" \
    "$PROMETHEUS_DATA_DIR" \
    "$CONFIG_DIR/prometheus" \
    "$CONFIG_DIR/grafana/provisioning/datasources" \
    "$CONFIG_DIR/grafana/provisioning/dashboards" \
    "$CONFIG_DIR/grafana/dashboards"
do
    if [[ -d "$dir" ]]; then
        ok "Already exists: $dir"
    else
        info "Creating: $dir"
        run mkdir -p "$dir"
        run chmod 755 "$dir"
    fi
done

# ---------------------------------------------------------------------------
# Step 4: Configure Prometheus
# ---------------------------------------------------------------------------

step "Configuring Prometheus"

PROM_CONFIG_SRC="$INFRA_DIR/prometheus/prometheus.yml"
PROM_CONFIG_DST="$CONFIG_DIR/prometheus/prometheus.yml"

if [[ ! -f "$PROM_CONFIG_SRC" ]]; then
    error "Prometheus config not found: $PROM_CONFIG_SRC"
fi

if [[ "$DRY_RUN" == false ]]; then
    cp "$PROM_CONFIG_SRC" "$PROM_CONFIG_DST"
    ok "Installed: $PROM_CONFIG_DST"
else
    info "[DRY] Would copy $PROM_CONFIG_SRC → $PROM_CONFIG_DST"
fi
info "Scrape targets are read from the installed config — edit the source at $PROM_CONFIG_SRC and re-run setup."

# ---------------------------------------------------------------------------
# Step 5: Configure Grafana
# ---------------------------------------------------------------------------

step "Configuring Grafana"

GRAFANA_INI_SRC="$INFRA_DIR/grafana/grafana.ini"
GRAFANA_INI_DST="$CONFIG_DIR/grafana/grafana.ini"

if [[ ! -f "$GRAFANA_INI_SRC" ]]; then
    error "Grafana config not found: $GRAFANA_INI_SRC"
fi

# Substitute placeholder paths and install to config directory
if [[ "$DRY_RUN" == false ]]; then
    sed \
        -e "s|/Users/Shared/agentd/infra/grafana/provisioning|$CONFIG_DIR/grafana/provisioning|g" \
        -e "s|/Users/Shared/agentd/logs|$LOG_DIR|g" \
        -e "s|/Users/Shared/agentd/grafana-data|$GRAFANA_DATA_DIR|g" \
        "$GRAFANA_INI_SRC" > "$GRAFANA_INI_DST"
    ok "Installed: $GRAFANA_INI_DST"
else
    info "[DRY] Would install $GRAFANA_INI_DST with resolved paths"
fi

# Install dashboard provisioning config
DASH_PROV_SRC="$INFRA_DIR/grafana/provisioning/dashboards/agentd.yml"
DASH_PROV_DST="$CONFIG_DIR/grafana/provisioning/dashboards/agentd.yml"

if [[ "$DRY_RUN" == false ]]; then
    sed \
        -e "s|/Users/Shared/agentd/infra/grafana/dashboards|$CONFIG_DIR/grafana/dashboards|g" \
        "$DASH_PROV_SRC" > "$DASH_PROV_DST"
    ok "Installed: $DASH_PROV_DST"
else
    info "[DRY] Would install $DASH_PROV_DST with resolved paths"
fi

# Copy datasource provisioning config
DATASOURCE_SRC="$INFRA_DIR/grafana/provisioning/datasources/agentd-prometheus.yml"
DATASOURCE_DST="$CONFIG_DIR/grafana/provisioning/datasources/agentd-prometheus.yml"

if [[ "$DRY_RUN" == false ]]; then
    cp "$DATASOURCE_SRC" "$DATASOURCE_DST"
    ok "Installed: $DATASOURCE_DST"
else
    info "[DRY] Would copy $DATASOURCE_SRC → $DATASOURCE_DST"
fi

# Copy dashboard JSON files
if [[ "$DRY_RUN" == false ]]; then
    for dashboard in "$INFRA_DIR/grafana/dashboards/"*.json; do
        cp "$dashboard" "$CONFIG_DIR/grafana/dashboards/"
    done
    ok "Installed dashboards to: $CONFIG_DIR/grafana/dashboards/"
else
    info "[DRY] Would copy dashboard JSONs to $CONFIG_DIR/grafana/dashboards/"
fi

# ---------------------------------------------------------------------------
# Step 6: Install launchd plists
# ---------------------------------------------------------------------------

step "Installing launchd plists"

run mkdir -p "$LAUNCH_AGENTS_DIR"

# --- Prometheus plist ---

PROM_PLIST_SRC="$INFRA_DIR/launchd/$PROMETHEUS_PLIST"
PROM_PLIST_DST="$LAUNCH_AGENTS_DIR/$PROMETHEUS_PLIST"

if [[ "$DRY_RUN" == false ]]; then
    sed \
        -e "s|/opt/homebrew|$BREW_PREFIX|g" \
        -e "s|/Users/Shared/agentd/infra/prometheus/prometheus.yml|$CONFIG_DIR/prometheus/prometheus.yml|g" \
        -e "s|/opt/homebrew/var/prometheus|$PROMETHEUS_DATA_DIR|g" \
        -e "s|/Users/Shared/agentd/logs/prometheus.log|$LOG_DIR/prometheus.log|g" \
        "$PROM_PLIST_SRC" > "$PROM_PLIST_DST"
    ok "Installed: $PROM_PLIST_DST"
else
    info "[DRY] Would install $PROM_PLIST_DST"
fi

# --- Grafana plist ---

GRAFANA_PLIST_SRC="$INFRA_DIR/launchd/$GRAFANA_PLIST"
GRAFANA_PLIST_DST="$LAUNCH_AGENTS_DIR/$GRAFANA_PLIST"

GRAFANA_HOME="$BREW_PREFIX/share/grafana"

if [[ "$DRY_RUN" == false ]]; then
    sed \
        -e "s|/opt/homebrew|$BREW_PREFIX|g" \
        -e "s|/Users/Shared/agentd/infra/grafana/grafana.ini|$CONFIG_DIR/grafana/grafana.ini|g" \
        -e "s|/Users/Shared/agentd/logs/grafana.log|$LOG_DIR/grafana.log|g" \
        "$GRAFANA_PLIST_SRC" > "$GRAFANA_PLIST_DST"
    ok "Installed: $GRAFANA_PLIST_DST"
else
    info "[DRY] Would install $GRAFANA_PLIST_DST"
fi

# ---------------------------------------------------------------------------
# Step 7: Load (or reload) services
# ---------------------------------------------------------------------------

step "Loading services via launchctl"

if [[ "$DRY_RUN" == false ]]; then
    # Prometheus
    if is_loaded "com.agentd.prometheus"; then
        info "Reloading Prometheus..."
        launchctl unload "$PROM_PLIST_DST" 2>/dev/null || true
    fi
    launchctl load "$PROM_PLIST_DST"
    ok "Prometheus loaded"

    # Grafana
    if is_loaded "com.agentd.grafana"; then
        info "Reloading Grafana..."
        launchctl unload "$GRAFANA_PLIST_DST" 2>/dev/null || true
    fi
    launchctl load "$GRAFANA_PLIST_DST"
    ok "Grafana loaded"
else
    info "[DRY] Would load com.agentd.prometheus via launchctl"
    info "[DRY] Would load com.agentd.grafana via launchctl"
fi

# ---------------------------------------------------------------------------
# Step 8: Wait for services and print status
# ---------------------------------------------------------------------------

step "Verifying services"

if [[ "$DRY_RUN" == false ]]; then
    sleep 2

    PROM_OK=false
    GRAFANA_OK=false

    if wait_for_port "$PROMETHEUS_PORT" "Prometheus" 2>/dev/null; then
        PROM_STATUS=$(curl -sf "http://127.0.0.1:$PROMETHEUS_PORT/-/ready" 2>/dev/null && echo "ready" || echo "starting")
        ok "Prometheus: http://localhost:$PROMETHEUS_PORT ($PROM_STATUS)"
        PROM_OK=true
    else
        warn "Prometheus not responding on port $PROMETHEUS_PORT"
    fi

    if wait_for_port "$GRAFANA_PORT" "Grafana" 2>/dev/null; then
        ok "Grafana: http://localhost:$GRAFANA_PORT (admin/admin)"
        GRAFANA_OK=true
    else
        warn "Grafana not responding on port $GRAFANA_PORT"
    fi
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------

echo
echo "============================================================"
echo " agentd observability stack setup complete"
echo "============================================================"
echo
echo "  Prometheus UI:  http://localhost:$PROMETHEUS_PORT"
echo "  Grafana UI:     http://localhost:$GRAFANA_PORT  (admin / admin)"
echo
echo "  Logs:           $LOG_DIR"
echo "  Config:         $CONFIG_DIR"
echo
echo "  To stop services:"
echo "    launchctl unload $LAUNCH_AGENTS_DIR/$PROMETHEUS_PLIST"
echo "    launchctl unload $LAUNCH_AGENTS_DIR/$GRAFANA_PLIST"
echo
echo "  To remove completely: ./infra/teardown.sh"
echo
if [[ "$DRY_RUN" == true ]]; then
    echo "  (Dry-run — no changes were made)"
    echo
fi
