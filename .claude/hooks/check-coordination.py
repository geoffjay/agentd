#!/usr/bin/env python3
"""
Claude Code PreToolUse hook: agent coordination checkpoint.

Intercepts high-impact Bash commands before they execute and checks for
active coordination locks and duplicate work in the communicate service.

Intercepted commands:
  gh issue create           — checks for active LOCK and similar open issues
  gh issue close/reopen     — checks for active LOCK and quality mismatch
  agent communicate ... announcements — checks for recent duplicate announcements

Fail-open behaviour:
  If the communicate service is unreachable, or if the check takes longer than
  the configured timeout, the command is ALLOWED to proceed. Coordination is
  best-effort — a service outage must never block agent work entirely.

Lock/unlock protocol:
  Before filing multiple issues or doing bulk operations, agents post to the
  engineering room:
    [LOCK] <agent>: <reason> -- hold all issue creation
  After completing:
    [UNLOCK] <agent>: <reason>

Lock window:
  LOCK_WINDOW_MINUTES (default 30) limits how far back the hook scans for
  active locks. This is intentional: a stale lock from a crashed agent should
  not block work indefinitely. The trade-off is that a very long bulk operation
  (>30 min) will silently lose its lock. Agents doing extended work should post
  periodic [LOCK] refreshes or break the work into shorter batches.

Configure in .claude/settings.json:
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/check-coordination.py",
            "timeout": 8
          }
        ]
      }
    ]
  }
}
"""

import json
import os
import re
import subprocess
import sys
import urllib.request
import urllib.error
from datetime import datetime, timezone, timedelta


# ── Configuration ─────────────────────────────────────────────────────────────

COMMUNICATE_BASE_URL = os.environ.get(
    "AGENTD_COMMUNICATE_SERVICE_URL", "http://localhost:17010"
)
ENGINEERING_ROOM_NAME = "engineering"
ANNOUNCEMENTS_ROOM_NAME = "announcements"

# How far back to look for LOCK messages (minutes)
LOCK_WINDOW_MINUTES = 30
# How far back to look for duplicate announcements (minutes)
ANNOUNCE_DEDUP_WINDOW_MINUTES = 60
# Timeout for each HTTP call (seconds)
HTTP_TIMEOUT_SECS = 2
# Number of recent messages to fetch
RECENT_MESSAGE_COUNT = 20


# ── Patterns for matching intercepted commands ─────────────────────────────────

# gh issue create  (any variant: --title "...", -t "...", etc.)
ISSUE_CREATE_RE = re.compile(r"\bgh\s+issue\s+create\b")
# gh issue close <number>
ISSUE_CLOSE_RE = re.compile(r"\bgh\s+issue\s+close\b")
# gh issue reopen <number>
ISSUE_REOPEN_RE = re.compile(r"\bgh\s+issue\s+reopen\b")
# agent communicate message send <room> (catch announcements room)
ANNOUNCE_SEND_RE = re.compile(
    r"\bagent\s+communicate\s+message\s+send\s+announcements?\b"
)
# Extract the --title / -t value from gh issue create
ISSUE_TITLE_RE = re.compile(
    r"""(?:--title|-t)\s+(?:"([^"]+)"|'([^']+)'|(\S+))"""
)
# Extract issue number from gh issue close/reopen <number>
ISSUE_NUMBER_RE = re.compile(r"\bgh\s+issue\s+(?:close|reopen)\s+(\d+)")
# LOCK marker posted by agents
LOCK_MARKER_RE = re.compile(r"\[LOCK\]", re.IGNORECASE)
# UNLOCK marker — used to cancel an active lock
UNLOCK_MARKER_RE = re.compile(r"\[UNLOCK\]", re.IGNORECASE)


# ── Output helpers ─────────────────────────────────────────────────────────────

def warn(message: str) -> None:
    """
    Inject a warning into the agent's context without blocking the action.
    Exits 0 so Claude Code treats the command as allowed.
    """
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "approve",
            "permissionDecisionReason": message,
        }
    }))
    sys.exit(0)


def block(message: str) -> None:
    """
    Block the command and inject the reason into the agent's context.
    Exits 0 — Claude Code reads the JSON and denies the tool call.
    """
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": message,
        }
    }))
    sys.exit(0)


def allow() -> None:
    """Allow the command to proceed (no output needed)."""
    sys.exit(0)


# ── Communicate service helpers ────────────────────────────────────────────────

def _http_get(path: str) -> dict | list | None:
    """
    Perform a GET against the communicate service. Returns parsed JSON or None
    on any error (timeout, connection refused, non-200, etc.).
    """
    url = f"{COMMUNICATE_BASE_URL}{path}"
    try:
        req = urllib.request.Request(url, headers={"Accept": "application/json"})
        with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT_SECS) as resp:
            if resp.status == 200:
                return json.loads(resp.read().decode())
    except Exception:
        pass
    return None


def get_room_id(room_name: str) -> str | None:
    """Resolve a room name to its UUID."""
    data = _http_get("/rooms?limit=100&offset=0")
    if not data:
        return None
    items = data.get("items") or data.get("data") or (data if isinstance(data, list) else [])
    for room in items:
        if room.get("name") == room_name:
            return room.get("id")
    return None


def get_latest_messages(room_id: str, count: int = RECENT_MESSAGE_COUNT) -> list[dict]:
    """Return the most recent `count` messages from a room (oldest-first)."""
    data = _http_get(f"/rooms/{room_id}/messages/latest?count={count}")
    if not data:
        return []
    if isinstance(data, list):
        return data
    return data.get("items") or data.get("messages") or []


def is_recent(timestamp_str: str, window_minutes: int) -> bool:
    """Return True if the ISO 8601 timestamp is within `window_minutes` ago."""
    try:
        dt = datetime.fromisoformat(timestamp_str.replace("Z", "+00:00"))
        cutoff = datetime.now(timezone.utc) - timedelta(minutes=window_minutes)
        return dt >= cutoff
    except Exception:
        return False


# ── Lock/unlock check ──────────────────────────────────────────────────────────

def check_active_lock(messages: list[dict]) -> str | None:
    """
    Scan messages for lock/unlock pairs per actor. Returns the text of a
    still-active lock if any exist, otherwise None.

    Each LOCK is keyed on the posting actor so that concurrent locks from
    different agents are tracked independently: an UNLOCK from Agent A only
    releases Agent A's lock, leaving Agent B's lock intact.
    """
    # Process in chronological order
    active_locks: dict[str, str] = {}  # actor -> lock message
    for msg in messages:
        content = msg.get("content") or msg.get("body") or msg.get("text") or ""
        actor = msg.get("sender") or msg.get("actor") or "unknown"
        if LOCK_MARKER_RE.search(content):
            active_locks[actor] = content.strip()
        elif UNLOCK_MARKER_RE.search(content):
            active_locks.pop(actor, None)
    return next(iter(active_locks.values()), None)


# ── Issue-create checks ────────────────────────────────────────────────────────

def _extract_title(command: str) -> str | None:
    m = ISSUE_TITLE_RE.search(command)
    if not m:
        return None
    return m.group(1) or m.group(2) or m.group(3)


def search_similar_issues(title: str, repo: str = "geoffjay/agentd") -> list[dict]:
    """
    Search open GitHub issues for titles similar to `title`.
    Returns a list of matching issue dicts (number, title).
    Falls back to empty list if gh CLI is unavailable or fails.
    """
    if not title:
        return []
    # Use the first few significant words as keywords
    words = [w for w in re.split(r"\W+", title) if len(w) > 3][:5]
    if not words:
        return []
    query = " ".join(words[:3])
    try:
        result = subprocess.run(
            ["gh", "issue", "list", "--repo", repo, "--state", "open",
             "--search", query, "--json", "number,title", "--limit", "10"],
            capture_output=True, text=True, timeout=HTTP_TIMEOUT_SECS + 1
        )
        if result.returncode == 0 and result.stdout.strip():
            issues = json.loads(result.stdout)
            # Filter out issues whose title is completely unrelated
            return [i for i in issues if any(
                w.lower() in i.get("title", "").lower() for w in words
            )]
    except Exception:
        pass
    return []


def handle_issue_create(command: str) -> None:
    """Check for active locks and similar issues before creating a new issue."""
    eng_id = get_room_id(ENGINEERING_ROOM_NAME)
    if eng_id:
        messages = get_latest_messages(eng_id)
        # Filter to recent messages only
        recent = [m for m in messages
                  if is_recent(m.get("created_at") or m.get("timestamp") or "", LOCK_WINDOW_MINUTES)]
        lock = check_active_lock(recent)
        if lock:
            block(
                f"⛔ Issue creation blocked — an active coordination lock is in effect.\n\n"
                f"Lock message from engineering room:\n  {lock}\n\n"
                "Wait for the [UNLOCK] signal before creating new issues. "
                "Check the engineering room for the current status."
            )

    # Check for similar existing issues (best-effort)
    title = _extract_title(command)
    if title:
        similar = search_similar_issues(title)
        if similar:
            issue_list = "\n".join(
                f"  #{i['number']}: {i['title']}" for i in similar[:5]
            )
            warn(
                f"⚠️  Possible duplicate detected for: \"{title}\"\n\n"
                f"Similar open issues found:\n{issue_list}\n\n"
                "Review the list above before proceeding. "
                "If this is genuinely a new issue, continue. "
                "If it overlaps, consider commenting on or updating an existing issue instead."
            )

    allow()


# ── Issue-close checks ─────────────────────────────────────────────────────────

def handle_issue_close(command: str) -> None:
    """Check for active locks and quality mismatch before closing an issue."""
    # Extract issue number from the command
    m = ISSUE_NUMBER_RE.search(command)
    issue_number = m.group(1) if m else None

    eng_id = get_room_id(ENGINEERING_ROOM_NAME)
    if eng_id:
        messages = get_latest_messages(eng_id)
        recent = [msg for msg in messages
                  if is_recent(msg.get("created_at") or msg.get("timestamp") or "", LOCK_WINDOW_MINUTES)]
        lock = check_active_lock(recent)
        if lock:
            block(
                f"⛔ Issue close blocked — an active coordination lock is in effect.\n\n"
                f"Lock message from engineering room:\n  {lock}\n\n"
                "Wait for the [UNLOCK] signal before modifying issues."
            )

    # Quality check: if a --comment flag names a duplicate, compare lengths
    if issue_number:
        duplicate_re = re.compile(
            r"(?:duplicate|dup(?:licate)?\s+of|closes?\s+#|superseded\s+by)\s+#?(\d+)",
            re.IGNORECASE
        )
        dup_match = duplicate_re.search(command)
        if not dup_match:
            # Also check the --comment flag text
            comment_re = re.compile(r"--comment\s+['\"]([^'\"]+)['\"]")
            comment_match = comment_re.search(command)
            if comment_match:
                dup_match = duplicate_re.search(comment_match.group(1))

        if dup_match:
            kept_number = dup_match.group(1)
            warn(
                f"⚠️  Closing issue #{issue_number} as duplicate of #{kept_number}.\n\n"
                f"Before proceeding, verify that the issue being KEPT (#{kept_number}) "
                f"is at least as detailed as the one being closed (#{issue_number}). "
                "If the issue you are closing is more detailed, consider merging its "
                "content into the kept issue first."
            )

    allow()


# ── Issue-reopen checks ────────────────────────────────────────────────────────

def handle_issue_reopen(command: str) -> None:
    """Check for active locks before reopening an issue (no duplicate logic)."""
    eng_id = get_room_id(ENGINEERING_ROOM_NAME)
    if eng_id:
        messages = get_latest_messages(eng_id)
        recent = [msg for msg in messages
                  if is_recent(
                      msg.get("created_at") or msg.get("timestamp") or "",
                      LOCK_WINDOW_MINUTES,
                  )]
        lock = check_active_lock(recent)
        if lock:
            block(
                f"⛔ Issue reopen blocked — an active coordination lock is in effect.\n\n"
                f"Lock message from engineering room:\n  {lock}\n\n"
                "Wait for the [UNLOCK] signal before modifying issues."
            )
    allow()


# ── Announcement checks ────────────────────────────────────────────────────────

def handle_announce(command: str) -> None:
    """Check for recent duplicate announcements before posting."""
    ann_id = get_room_id(ANNOUNCEMENTS_ROOM_NAME)
    if not ann_id:
        allow()

    messages = get_latest_messages(ann_id, count=10)
    recent = [m for m in messages
              if is_recent(m.get("created_at") or m.get("timestamp") or "", ANNOUNCE_DEDUP_WINDOW_MINUTES)]
    if recent:
        # Extract a snippet of what's about to be announced from the command
        text_match = re.search(r"""(?:send\s+\w+\s+)["']?([^"'\n]{10,80})""", command)
        snippet = text_match.group(1)[:60] if text_match else None

        recent_summaries = []
        for msg in recent[-5:]:
            content = (msg.get("content") or msg.get("body") or "")[:100]
            sender = msg.get("sender") or msg.get("actor") or "unknown"
            recent_summaries.append(f"  [{sender}]: {content}")

        context = "\n".join(recent_summaries) if recent_summaries else "  (none)"
        warn(
            f"⚠️  {len(recent)} announcement(s) were posted in the last "
            f"{ANNOUNCE_DEDUP_WINDOW_MINUTES} minutes.\n\n"
            f"Recent announcements:\n{context}\n\n"
            f"Posting: \"{snippet}...\"\n\n"
            "Verify this announcement is not a duplicate before proceeding. "
            "Ensure the engineering channel discussion preceded this announcement."
        )

    allow()


# ── Main ───────────────────────────────────────────────────────────────────────

def main() -> None:
    try:
        hook_input = json.load(sys.stdin)
    except (json.JSONDecodeError, EOFError):
        allow()

    tool_name = hook_input.get("tool_name", "")
    tool_input = hook_input.get("tool_input", {})

    # Only intercept Bash commands
    if tool_name not in ("Bash", "bash", "shell"):
        allow()

    command = tool_input.get("command", "")
    if not isinstance(command, str) or not command:
        allow()

    # Route to the appropriate check
    if ISSUE_CREATE_RE.search(command):
        handle_issue_create(command)
    elif ISSUE_CLOSE_RE.search(command):
        handle_issue_close(command)
    elif ISSUE_REOPEN_RE.search(command):
        handle_issue_reopen(command)  # lock check only; no duplicate logic
    elif ANNOUNCE_SEND_RE.search(command):
        handle_announce(command)
    else:
        allow()


if __name__ == "__main__":
    main()
