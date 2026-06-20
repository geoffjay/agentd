#!/usr/bin/env bash
#
# agentd CLI integration test
# ============================
#
# Exercises agentd functionality two ways:
#
#   * direct  — talk straight to a service on its own port (HTTP via curl).
#               Validates a service in isolation; no core gateway, no auth.
#   * gateway — drive the `agent` CLI, which always routes through the core
#               gateway (`{core_url}/api/v1/{service}`). Validates routing,
#               auth, and CLI rendering end-to-end. Mutating CLI tests need a
#               session token (run `agent auth login` first, or set AGENT_TOKEN).
#
# Running both pinpoints failures: if direct passes but gateway fails, the
# fault is in the core proxy or the CLI path, not the service.
#
# SAFETY
#   * Read-only health probes run unconditionally.
#   * Mutating tests refuse to run against a non-localhost target unless
#     --allow-remote is given.
#   * Every record this script creates is tagged with a unique run marker and
#     deleted on exit (disable with --no-cleanup).
#
# This script never starts services. Bring the dev stack up first, e.g.:
#   overmind start         # or: foreman start  (uses ./Procfile)
# then run this script. Test groups whose service is down are SKIPped.
#
# Usage:
#   tests/integration/cli_integration.sh [--mode direct|gateway|both]
#                                        [--prod] [--allow-remote]
#                                        [--no-cleanup] [-h|--help]
#
# Env overrides:
#   AGENT_BIN                 path to the CLI binary (else auto-resolved)
#   AGENTD_HOST               host for direct service calls (default 127.0.0.1)
#   AGENTD_<SVC>_PORT         override a service port (e.g. AGENTD_MEMORY_PORT)
#   AGENTD_CORE_SERVICE_URL   core gateway URL the CLI uses (default per ports)
#   AGENT_TOKEN               bearer token for gateway mutation tests

set -uo pipefail

# ── Arguments ────────────────────────────────────────────────────────────────
MODE="both"
USE_PROD=0
ALLOW_REMOTE=0
CLEANUP=1

usage() { sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'; exit "${1:-0}"; }

while [ $# -gt 0 ]; do
    case "$1" in
        --mode) MODE="${2:-}"; shift 2 ;;
        --mode=*) MODE="${1#*=}"; shift ;;
        --prod) USE_PROD=1; shift ;;
        --allow-remote) ALLOW_REMOTE=1; shift ;;
        --no-cleanup) CLEANUP=0; shift ;;
        -h|--help) usage 0 ;;
        *) echo "unknown argument: $1" >&2; usage 1 ;;
    esac
done

case "$MODE" in direct|gateway|both) ;; *) echo "invalid --mode: $MODE" >&2; exit 2 ;; esac

# ── Ports & hosts ────────────────────────────────────────────────────────────
# Dev ports are 170xx; prod ports are 70xx (dev minus 10000).
PORT_BASE=17000
[ "$USE_PROD" -eq 1 ] && PORT_BASE=7000

HOST="${AGENTD_HOST:-127.0.0.1}"

port() { # port <dev-offset>  ->  resolved port honoring env override
    local name="$1" offset="$2" envvar
    envvar="AGENTD_$(echo "$name" | tr '[:lower:]' '[:upper:]')_PORT"
    echo "${!envvar:-$((PORT_BASE + offset))}"
}

CORE_PORT=$(port core 0)
MEMORY_PORT=$(port memory 8)
NOTIFY_PORT=$(port notify 4)
KNOWLEDGE_PORT=$(port knowledge 11)
COMMUNICATE_PORT=$(port communicate 10)
ORCHESTRATOR_PORT=$(port orchestrator 6)

CORE_URL="${AGENTD_CORE_SERVICE_URL:-http://${HOST}:${CORE_PORT}}"
MEMORY_DIRECT="http://${HOST}:${MEMORY_PORT}"

# Unique marker so this run's data is identifiable and removable.
RUN_ID="itest-$$-$(date +%s)"
TEST_TAG="cli-itest"

# ── Output & counters ────────────────────────────────────────────────────────
if [ -t 1 ]; then C_G=$'\033[32m'; C_R=$'\033[31m'; C_Y=$'\033[33m'; C_B=$'\033[34m'; C_0=$'\033[0m'
else C_G=; C_R=; C_Y=; C_B=; C_0=; fi

PASS=0; FAIL=0; SKIP=0
declare -a FAILURES=()

pass()  { PASS=$((PASS+1)); printf '  %s✓%s %s\n' "$C_G" "$C_0" "$1"; }
fail()  { FAIL=$((FAIL+1)); FAILURES+=("$1"); printf '  %s✗%s %s\n' "$C_R" "$C_0" "$1"; }
skip()  { SKIP=$((SKIP+1)); printf '  %s○%s %s%s\n' "$C_Y" "$C_0" "$1" "${2:+ — $2}"; }
info()  { printf '  %s·%s %s\n'  "$C_B" "$C_0" "$1"; }
group() { printf '\n%s== %s ==%s\n' "$C_B" "$1" "$C_0"; }

# ── Dependencies ─────────────────────────────────────────────────────────────
command -v curl >/dev/null 2>&1 || { echo "curl is required" >&2; exit 3; }
HAVE_JQ=0; command -v jq >/dev/null 2>&1 && HAVE_JQ=1

# Extract a top-level string field from a JSON object. Prefers jq; falls back to
# a (best-effort) grep for environments without jq.
json_field() { # json_field <field> <json>
    if [ "$HAVE_JQ" -eq 1 ]; then
        printf '%s' "$2" | jq -r --arg f "$1" '.[$f] // empty' 2>/dev/null
    else
        printf '%s' "$2" | grep -o "\"$1\"[[:space:]]*:[[:space:]]*\"[^\"]*\"" | head -1 | sed 's/.*:[[:space:]]*"\([^"]*\)"/\1/'
    fi
}

# GET/POST/DELETE returning "HTTP_STATUS\nBODY"; capture both for assertions.
http() { # http <method> <url> [json-body]
    local method="$1" url="$2" body="${3:-}"
    if [ -n "$body" ]; then
        curl -sS -m 15 -o /dev/null -w '%{http_code}' -X "$method" "$url" \
            -H 'Content-Type: application/json' -d "$body" 2>/dev/null
    else
        curl -sS -m 15 -o /dev/null -w '%{http_code}' -X "$method" "$url" 2>/dev/null
    fi
}
http_body() { # http_body <method> <url> [json-body]  -> response body only
    local method="$1" url="$2" body="${3:-}"
    if [ -n "$body" ]; then
        curl -sS -m 15 -X "$method" "$url" -H 'Content-Type: application/json' -d "$body" 2>/dev/null
    else
        curl -sS -m 15 -X "$method" "$url" 2>/dev/null
    fi
}
# Single call returning the body with the status code appended on the last line.
http_both() { # http_both <method> <url>  ->  "<body>\n<status>"
    curl -sS -m 15 -w '\n%{http_code}' -X "$1" "$2" 2>/dev/null
}

is_localhost() { case "$1" in localhost|127.0.0.1|::1|"[::1]") return 0 ;; *) return 1 ;; esac; }

# ── CLI binary resolution ────────────────────────────────────────────────────
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CLI=()
resolve_cli() {
    if [ -n "${AGENT_BIN:-}" ] && [ -x "${AGENT_BIN}" ]; then CLI=("$AGENT_BIN"); return; fi
    for c in "$REPO_ROOT/target/debug/cli" "$REPO_ROOT/target/release/cli"; do
        [ -x "$c" ] && { CLI=("$c"); return; }
    done
    for c in agent cli; do
        command -v "$c" >/dev/null 2>&1 && { CLI=("$c"); return; }
    done
    # Last resort: build-and-run via cargo (slow). Warn the caller.
    info "no prebuilt CLI found; falling back to 'cargo run -p cli' (slow)"
    CLI=(cargo run -q -p cli --)
}

# ── Cleanup ──────────────────────────────────────────────────────────────────
declare -a CREATED_IDS=()
drop_id() { # drop_id <id> — remove an id from the cleanup list (already deleted)
    local keep=() x
    for x in "${CREATED_IDS[@]}"; do [ "$x" = "$1" ] || keep+=("$x"); done
    CREATED_IDS=(${keep[@]+"${keep[@]}"})  # +-guard: empty array is OK under set -u
}
cleanup() {
    [ "$CLEANUP" -eq 1 ] || { info "skipping cleanup (--no-cleanup); marker=$RUN_ID"; return; }
    [ "${#CREATED_IDS[@]}" -eq 0 ] && return
    printf '\n%s== Cleanup ==%s\n' "$C_B" "$C_0"
    for id in "${CREATED_IDS[@]}"; do
        local code; code=$(http DELETE "${MEMORY_DIRECT}/memories/${id}")
        if [ "$code" = "200" ] || [ "$code" = "204" ]; then info "deleted $id"
        else info "could not delete $id (HTTP $code) — remove manually"; fi
    done
}
trap cleanup EXIT

# ── Health probe ─────────────────────────────────────────────────────────────
service_up() { # service_up <url>  -> 0 if /health returns 200
    [ "$(http GET "$1/health")" = "200" ]
}

# ── DIRECT MODE ──────────────────────────────────────────────────────────────
run_direct() {
    group "Direct: service health (port $MEMORY_PORT etc.)"
    local svc
    for svc in "memory:$MEMORY_PORT" "notify:$NOTIFY_PORT" "knowledge:$KNOWLEDGE_PORT" \
               "communicate:$COMMUNICATE_PORT" "orchestrator:$ORCHESTRATOR_PORT"; do
        local name="${svc%%:*}" p="${svc##*:}" code
        code=$(http GET "http://${HOST}:${p}/health")
        if [ "$code" = "200" ]; then pass "$name /health (200)"
        else fail "$name /health on :$p returned ${code:-no-response}"; fi
    done

    group "Direct: memory CRUD round-trip (no gateway, no auth)"
    if ! service_up "$MEMORY_DIRECT"; then
        skip "memory CRUD" "memory service not reachable at $MEMORY_DIRECT"
        return
    fi
    if ! is_localhost "$HOST" && [ "$ALLOW_REMOTE" -eq 0 ]; then
        skip "memory CRUD" "target $HOST is not localhost (use --allow-remote)"
        return
    fi
    if [ "$HAVE_JQ" -eq 0 ]; then
        info "jq not found — using best-effort JSON parsing"
    fi

    # CREATE — content intentionally crosses a multibyte boundary near byte 49
    # so it would have tripped the old `&content[..49]` slice-panic in the CLI
    # list renderer (now char-safe).
    local content="${RUN_ID}: padding padding padding padding 日本語テストデータ end"
    local payload
    payload=$(cat <<JSON
{"content":"${content}","type":"information","tags":["${TEST_TAG}"],"created_by":"${RUN_ID}","visibility":"public"}
JSON
)
    local created code id
    created=$(http_body POST "${MEMORY_DIRECT}/memories" "$payload")
    id=$(json_field id "$created")
    if [ -n "$id" ]; then
        pass "create memory ($id)"; CREATED_IDS+=("$id")
    elif printf '%s' "$created" | grep -qi 'embedding'; then
        # Storing a memory embeds its content, so CRUD needs a configured
        # provider. Not a failure of the service — just unconfigured here.
        skip "memory CRUD" "embedding provider not configured (set AGENTD_MEMORY_EMBEDDING_PROVIDER)"
        return
    else
        fail "create memory — no id in response: $(printf '%.120s' "$created")"; return
    fi

    # READ
    code=$(http GET "${MEMORY_DIRECT}/memories/${id}")
    if [ "$code" = "200" ]; then pass "recall memory (200)"; else fail "recall memory returned $code"; fi

    # LIST (filtered by our creator) — should include the new record
    local listed
    listed=$(http_body GET "${MEMORY_DIRECT}/memories?created_by=${RUN_ID}")
    if printf '%s' "$listed" | grep -q "$id"; then pass "list memories includes new record"
    else fail "list memories did not include $id"; fi

    # SEARCH — optional: requires an embedding provider. Accept success OR a
    # clean error response, but never a hang/crash.
    code=$(http POST "${MEMORY_DIRECT}/memories/search" "{\"query\":\"${TEST_TAG} data\",\"limit\":5}")
    case "$code" in
        200) pass "semantic search (200; embeddings configured)" ;;
        4*|5*) skip "semantic search" "HTTP $code (embedding provider likely 'none')" ;;
        *) fail "semantic search — unexpected response: ${code:-none}" ;;
    esac

    # DELETE
    code=$(http DELETE "${MEMORY_DIRECT}/memories/${id}")
    if [ "$code" = "200" ] || [ "$code" = "204" ]; then
        pass "forget memory ($code)"
        drop_id "$id"  # already deleted; remove from cleanup list
        code=$(http GET "${MEMORY_DIRECT}/memories/${id}")
        if [ "$code" = "404" ]; then pass "recall after delete returns 404"
        else fail "recall after delete returned $code (expected 404)"; fi
    else
        fail "forget memory returned $code"
    fi
}

# ── GATEWAY MODE (via the CLI) ───────────────────────────────────────────────
run_gateway() {
    resolve_cli
    info "CLI: ${CLI[*]}"
    info "core gateway: $CORE_URL"
    export AGENTD_CORE_SERVICE_URL="$CORE_URL"

    group "Gateway: core reachability"
    local code
    code=$(http GET "${CORE_URL}/health")
    if [ "$code" = "200" ]; then pass "core /health (200)"
    else
        fail "core /health returned ${code:-no-response}"
        skip "gateway CLI tests" "core gateway not reachable at $CORE_URL"
        return
    fi
    # Aggregate downstream health. The gateway returns 503 if ANY downstream is
    # unhealthy — that reflects environment state, not a gateway fault — so a
    # well-formed 503 is reported as "degraded" (with the offending services),
    # not a failure. Only a malformed/unreachable response fails the run.
    local out
    out=$(http_both GET "${CORE_URL}/api/v1/health")
    code="${out##*$'\n'}"          # status = last line
    local agg_body="${out%$'\n'*}" # body = everything before it
    case "$code" in
        200)
            pass "core /api/v1/health aggregate (all downstreams healthy)"
            ;;
        503)
            local down=""
            if [ "$HAVE_JQ" -eq 1 ]; then
                down=$(printf '%s' "$agg_body" \
                    | jq -r '.services[]? | select(.healthy==false) | .name' 2>/dev/null \
                    | paste -sd, - 2>/dev/null)
            fi
            if printf '%s' "$agg_body" | grep -q '"services"'; then
                skip "aggregate health (gateway OK, downstream degraded)" "unhealthy: ${down:-see body}"
            else
                fail "core /api/v1/health 503 with no health body: $(printf '%.120s' "$agg_body")"
            fi
            ;;
        401|403) skip "aggregate health" "auth required (HTTP $code)" ;;
        *) fail "core /api/v1/health returned ${code:-no-response}" ;;
    esac

    group "Gateway: 'agent status' (probes per-service ports)"
    if "${CLI[@]}" status >/tmp/itest-status.$$ 2>&1; then
        pass "agent status exited 0"
    else
        fail "agent status failed (exit $?) — see /tmp/itest-status.$$"
    fi

    # Determine whether we have a usable token for authenticated calls.
    local have_auth=0
    if [ -n "${AGENT_TOKEN:-}" ]; then have_auth=1; fi
    # `agent auth login` writes a session file; a successful unauthenticated
    # call below would still 401, so gate CRUD on token presence.

    group "Gateway: memory through the CLI (regression guard)"
    if ! is_localhost "$HOST" && [ "$ALLOW_REMOTE" -eq 0 ]; then
        skip "CLI memory CRUD" "target not localhost (use --allow-remote)"
        return
    fi
    if [ "$have_auth" -eq 0 ]; then
        skip "CLI memory CRUD" "no AGENT_TOKEN and gateway requires auth — run 'agent auth login'"
        info "tip: read-only 'agent memory list' is still attempted below"
    fi

    # The list renderer is the path that previously panicked on multibyte
    # content. Even with no rows it must exit cleanly; with rows it must not
    # panic. This is the core regression guard.
    if "${CLI[@]}" memory list --tag "$TEST_TAG" --limit 50 >/tmp/itest-list.$$ 2>&1; then
        pass "agent memory list rendered without panic"
    else
        # A 401 (auth) is an expected non-panic failure; a panic is not.
        if grep -qi 'panic\|char boundary\|byte index' /tmp/itest-list.$$; then
            fail "agent memory list PANICKED (regression!) — see /tmp/itest-list.$$"
        else
            skip "agent memory list" "non-panic error (likely auth); see /tmp/itest-list.$$"
        fi
    fi

    [ "$have_auth" -eq 1 ] || return

    # Full CLI round-trip through the gateway with auth.
    local content="${RUN_ID} gateway 日本語 round-trip padding padding padding end"
    local out id
    out=$("${CLI[@]}" --json memory remember "$content" \
            --created-by "$RUN_ID" --type information --tags "$TEST_TAG" 2>/tmp/itest-remember.$$)
    id=$(json_field id "$out")
    if [ -n "$id" ]; then pass "agent memory remember ($id)"; CREATED_IDS+=("$id")
    else fail "agent memory remember — no id; see /tmp/itest-remember.$$"; return; fi

    if "${CLI[@]}" memory list --created-by "$RUN_ID" >/tmp/itest-list2.$$ 2>&1 \
        && grep -q "$id" /tmp/itest-list2.$$; then
        pass "agent memory list shows the new record (human render)"
    else
        fail "agent memory list missing $id (or render error); see /tmp/itest-list2.$$"
    fi

    if "${CLI[@]}" memory recall "$id" >/dev/null 2>&1; then pass "agent memory recall"
    else fail "agent memory recall failed"; fi

    if "${CLI[@]}" memory forget "$id" >/dev/null 2>&1; then
        pass "agent memory forget"
        drop_id "$id"
    else
        fail "agent memory forget failed"
    fi
}

# ── Run ──────────────────────────────────────────────────────────────────────
printf '%sagentd CLI integration test%s  (mode=%s, %s ports, marker=%s)\n' \
    "$C_B" "$C_0" "$MODE" "$([ "$USE_PROD" -eq 1 ] && echo prod || echo dev)" "$RUN_ID"

case "$MODE" in
    direct)  run_direct ;;
    gateway) run_gateway ;;
    both)    run_direct; run_gateway ;;
esac

# ── Summary ──────────────────────────────────────────────────────────────────
printf '\n%s== Summary ==%s\n' "$C_B" "$C_0"
printf '  passed: %s%d%s   failed: %s%d%s   skipped: %s%d%s\n' \
    "$C_G" "$PASS" "$C_0" "$C_R" "$FAIL" "$C_0" "$C_Y" "$SKIP" "$C_0"
if [ "$FAIL" -gt 0 ]; then
    printf '\n%sFailures:%s\n' "$C_R" "$C_0"
    for f in "${FAILURES[@]}"; do printf '  - %s\n' "$f"; done
    exit 1
fi
exit 0
