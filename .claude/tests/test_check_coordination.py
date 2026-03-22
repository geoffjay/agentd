"""
Unit tests for .claude/hooks/check-coordination.py

Run with:
    python3 -m pytest .claude/tests/test_check_coordination.py -v   # if pytest available
    python3 .claude/tests/test_check_coordination.py                 # stdlib unittest
"""

import importlib.util
import io
import json
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import patch


# ── Load the hook module from its non-package path ────────────────────────────

HOOK_PATH = Path(__file__).parent.parent / "hooks" / "check-coordination.py"


def _load_hook() -> types.ModuleType:
    spec = importlib.util.spec_from_file_location("check_coordination", HOOK_PATH)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


hook = _load_hook()


# ── Helpers ───────────────────────────────────────────────────────────────────

def _msg(content: str, actor: str = "agent-x", ts: str = "2099-01-01T00:00:00Z") -> dict:
    """Build a minimal message dict."""
    return {"content": content, "sender": actor, "created_at": ts}


def _run_main(command: str) -> dict:
    """Feed a Bash tool call into main() and capture the JSON output."""
    tool_input = json.dumps({
        "tool_name": "Bash",
        "tool_input": {"command": command},
    })
    captured = {}

    def fake_print(data):
        captured["output"] = json.loads(data)

    orig_stdin = sys.stdin
    sys.stdin = io.StringIO(tool_input)
    try:
        with (
            patch("builtins.print", side_effect=fake_print),
            patch("sys.exit", side_effect=SystemExit),
        ):
            try:
                hook.main()
            except SystemExit:
                pass
    finally:
        sys.stdin = orig_stdin

    return captured.get("output", {})


# ── check_active_lock ─────────────────────────────────────────────────────────

class TestCheckActiveLock(unittest.TestCase):

    def test_no_messages_returns_none(self):
        self.assertIsNone(hook.check_active_lock([]))

    def test_single_lock_is_returned(self):
        msgs = [_msg("[LOCK] planner: filing issues")]
        result = hook.check_active_lock(msgs)
        self.assertEqual(result, "[LOCK] planner: filing issues")

    def test_lock_then_unlock_returns_none(self):
        msgs = [
            _msg("[LOCK] planner: filing issues", actor="planner"),
            _msg("[UNLOCK] planner: done", actor="planner"),
        ]
        self.assertIsNone(hook.check_active_lock(msgs))

    def test_concurrent_locks_independent(self):
        """Agent A's UNLOCK must not release Agent B's lock."""
        msgs = [
            _msg("[LOCK] planner: bulk create", actor="planner"),
            _msg("[LOCK] conductor: bulk merge", actor="conductor"),
            _msg("[UNLOCK] planner: done", actor="planner"),
        ]
        result = hook.check_active_lock(msgs)
        # conductor's lock is still active
        self.assertIsNotNone(result)
        self.assertIn("conductor", result)

    def test_both_agents_unlock_returns_none(self):
        msgs = [
            _msg("[LOCK] planner: bulk create", actor="planner"),
            _msg("[LOCK] conductor: bulk merge", actor="conductor"),
            _msg("[UNLOCK] planner: done", actor="planner"),
            _msg("[UNLOCK] conductor: done", actor="conductor"),
        ]
        self.assertIsNone(hook.check_active_lock(msgs))

    def test_lock_case_insensitive(self):
        msgs = [_msg("[lock] worker: hold", actor="worker")]
        self.assertIsNotNone(hook.check_active_lock(msgs))

    def test_unlock_without_prior_lock_returns_none(self):
        msgs = [_msg("[UNLOCK] planner: done", actor="planner")]
        self.assertIsNone(hook.check_active_lock(msgs))

    def test_unknown_actor_key_used_when_sender_absent(self):
        msgs = [{"content": "[LOCK] something"}]  # no sender/actor key
        self.assertIsNotNone(hook.check_active_lock(msgs))

    def test_second_lock_from_same_actor_overwrites(self):
        msgs = [
            _msg("[LOCK] planner: first batch", actor="planner"),
            _msg("[LOCK] planner: second batch", actor="planner"),
        ]
        result = hook.check_active_lock(msgs)
        self.assertEqual(result, "[LOCK] planner: second batch")


# ── _extract_title ────────────────────────────────────────────────────────────

class TestExtractTitle(unittest.TestCase):

    def test_double_quoted_title(self):
        cmd = 'gh issue create --title "Add retry logic" --body "..."'
        self.assertEqual(hook._extract_title(cmd), "Add retry logic")

    def test_single_quoted_title(self):
        cmd = "gh issue create --title 'Fix the bug' --repo foo/bar"
        self.assertEqual(hook._extract_title(cmd), "Fix the bug")

    def test_short_flag(self):
        cmd = 'gh issue create -t "Short title" --body x'
        self.assertEqual(hook._extract_title(cmd), "Short title")

    def test_no_title_returns_none(self):
        cmd = "gh issue create --body 'some body'"
        self.assertIsNone(hook._extract_title(cmd))


# ── main() routing ────────────────────────────────────────────────────────────

class TestMainRouting(unittest.TestCase):
    """Verify that main() dispatches to the correct handler for each command."""

    def test_non_bash_tool_allows(self):
        """Non-Bash tool calls must be allowed without any check."""
        tool_input = json.dumps({
            "tool_name": "Write",
            "tool_input": {"file_path": "/tmp/x", "content": ""},
        })
        orig_stdin = sys.stdin
        sys.stdin = io.StringIO(tool_input)
        try:
            with patch("sys.exit", side_effect=SystemExit) as mock_exit:
                try:
                    hook.main()
                except SystemExit:
                    pass
            mock_exit.assert_called_once_with(0)
        finally:
            sys.stdin = orig_stdin

    def test_reopen_routes_to_reopen_handler_not_close(self):
        """gh issue reopen must NOT invoke handle_issue_close."""
        tool_input = json.dumps({
            "tool_name": "Bash",
            "tool_input": {"command": "gh issue reopen 42 --repo geoffjay/agentd"},
        })
        orig_stdin = sys.stdin
        sys.stdin = io.StringIO(tool_input)
        try:
            with (
                patch.object(hook, "handle_issue_reopen") as mock_reopen,
                patch.object(hook, "handle_issue_close") as mock_close,
            ):
                try:
                    hook.main()
                except SystemExit:
                    pass
            mock_reopen.assert_called_once()
            mock_close.assert_not_called()
        finally:
            sys.stdin = orig_stdin

    def test_issue_create_routes_to_create_handler(self):
        tool_input = json.dumps({
            "tool_name": "Bash",
            "tool_input": {"command": 'gh issue create --title "test" --body "body"'},
        })
        orig_stdin = sys.stdin
        sys.stdin = io.StringIO(tool_input)
        try:
            with patch.object(hook, "handle_issue_create") as mock_create:
                try:
                    hook.main()
                except SystemExit:
                    pass
            mock_create.assert_called_once()
        finally:
            sys.stdin = orig_stdin

    def test_issue_close_routes_to_close_handler(self):
        tool_input = json.dumps({
            "tool_name": "Bash",
            "tool_input": {"command": "gh issue close 99 --repo geoffjay/agentd"},
        })
        orig_stdin = sys.stdin
        sys.stdin = io.StringIO(tool_input)
        try:
            with patch.object(hook, "handle_issue_close") as mock_close:
                try:
                    hook.main()
                except SystemExit:
                    pass
            mock_close.assert_called_once()
        finally:
            sys.stdin = orig_stdin


if __name__ == "__main__":
    unittest.main(verbosity=2)
