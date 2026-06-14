//! Integration tests for the `agentd-core migrate` CLI subcommand.
//!
//! Each test spawns the real binary against an isolated temporary SQLite
//! database via `--db-path`, so no state leaks between runs and the
//! developer's own database is never touched.

use std::path::Path;
use std::process::Command;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Returns a `Command` pointing at the compiled `agentd-core` binary.
fn agentd_core() -> Command {
    Command::new(env!("CARGO_BIN_EXE_agentd-core"))
}

/// Run `agentd-core migrate <args>` against `db_path` and return the output.
fn migrate(db_path: &Path, args: &[&str]) -> std::process::Output {
    let mut cmd = agentd_core();
    cmd.arg("migrate");
    for arg in args {
        cmd.arg(arg);
    }
    cmd.arg("--db-path").arg(db_path);
    cmd.output().expect("failed to spawn agentd-core")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// `migrate up` on a fresh DB applies all migrations; `migrate status`
/// subsequently shows every migration as applied with none pending.
#[test]
fn test_migrate_up_then_status() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("core.db");

    // Apply all migrations
    let out = migrate(&db_path, &["up"]);
    assert!(out.status.success(), "migrate up failed:\n{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("up to date") || stdout.contains("applied"), "{stdout}");

    // Status should show only applied migrations
    let out = migrate(&db_path, &["status"]);
    assert!(
        out.status.success(),
        "migrate status failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("applied"), "expected 'applied' in status output:\n{stdout}");
    // Summary line always says "<n> applied, <m> pending"; after a full `up`
    // that count should be "0 pending", meaning no migration is still waiting.
    assert!(stdout.contains("0 pending"), "expected '0 pending' in status output:\n{stdout}");
}

/// `migrate status` on a brand-new database (no migrations run) reports all
/// migrations as pending with none applied.
#[test]
fn test_status_on_empty_database() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("core.db");

    let out = migrate(&db_path, &["status"]);
    assert!(
        out.status.success(),
        "migrate status failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // All migrations should be pending; none should be listed as applied
    assert!(stdout.contains("pending"), "expected 'pending' in output:\n{stdout}");
    assert!(stdout.contains("0 applied"), "expected '0 applied' in output:\n{stdout}");
}

/// After `migrate up`, `migrate down --yes` rolls back the latest migration;
/// `migrate status` then shows that migration as pending.
#[test]
fn test_migrate_down_then_status() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("core.db");

    // First apply everything
    let out = migrate(&db_path, &["up"]);
    assert!(out.status.success(), "migrate up failed:\n{}", String::from_utf8_lossy(&out.stderr));

    // Roll back the latest migration (skip prompt with --yes)
    let out = migrate(&db_path, &["down", "--yes"]);
    assert!(
        out.status.success(),
        "migrate down --yes failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("rolled back"), "expected 'rolled back' in output:\n{stdout}");

    // Status should now show at least one pending migration
    let out = migrate(&db_path, &["status"]);
    assert!(
        out.status.success(),
        "migrate status failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("pending"), "expected 'pending' after down:\n{stdout}");
}

/// `migrate down` without `--yes` when stdin is not a TTY must exit non-zero
/// and produce an error message explaining how to bypass the prompt.
#[test]
fn test_migrate_down_without_yes_exits_nonzero() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("core.db");

    // Apply migrations first so there is something to roll back
    let out = migrate(&db_path, &["up"]);
    assert!(out.status.success(), "migrate up failed:\n{}", String::from_utf8_lossy(&out.stderr));

    // Run `migrate down` without --yes; stdin is not a TTY in cargo test
    let out = migrate(&db_path, &["down"]);
    assert!(
        !out.status.success(),
        "expected non-zero exit but got success; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--yes") || stderr.contains("TTY"),
        "expected --yes or TTY hint in stderr:\n{stderr}"
    );
}
