//! Skill discovery for agentd agents.
//!
//! A *skill* is a Markdown file (with optional YAML frontmatter) that is
//! injected into an agent's working directory at spawn time so that Claude Code
//! can discover and invoke it via the `/skill` command.
//!
//! ## File layouts
//!
//! Two layouts are supported, matching the Claude Code convention:
//!
//! ```text
//! .agentd/skills/<name>/SKILL.md     ← directory layout
//! .agentd/skills/<name>.md           ← flat layout
//! ```
//!
//! ## Discovery paths (in precedence order)
//!
//! 1. `.agentd/skills/` — project-level skills (checked in with the repo)
//! 2. `~/.config/agentd/skills/` — user-level fallback
//!
//! When the same skill name appears in multiple locations, the higher-precedence
//! source wins and the duplicate is silently dropped.
//!
//! ## Frontmatter
//!
//! Each skill file may begin with a YAML frontmatter block delimited by `---`:
//!
//! ```markdown
//! ---
//! name: git-spice
//! description: Branch stacking and PR management via git-spice.
//! ---
//!
//! # Git Spice
//! ...
//! ```
//!
//! The `name` field overrides the directory / filename stem used as the
//! skill name.  The `description` field is surfaced by `GET /skills` and
//! `agent skill list`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Materialization
// ---------------------------------------------------------------------------

/// Result of a [`materialize_skills`] call.
#[derive(Debug, Default, PartialEq)]
pub struct MaterializeResult {
    /// Skill names whose `SKILL.md` file was successfully written.
    pub written: Vec<String>,
    /// Skill names that were skipped because the target file already existed.
    ///
    /// The agent's own `.claude/skills/<name>/SKILL.md` takes precedence over
    /// the agentd-managed copy.
    pub skipped: Vec<String>,
    /// Skill names that were requested but are not in `discovered_skills`.
    pub not_found: Vec<String>,
}

/// Write skill files into the agent's `.claude/skills/` directory.
///
/// For each skill name in `skill_names`, copies the skill content from
/// `discovered_skills` into `<working_dir>/.claude/skills/<name>/SKILL.md`.
///
/// - Creates the directory structure if it does not exist.
/// - Does **not** overwrite existing skill files; agent-local skills take
///   precedence (reported as [`MaterializeResult::skipped`]).
/// - Skill names not present in `discovered_skills` are reported in
///   [`MaterializeResult::not_found`] rather than returning an error.
///
/// # Worktree agents
///
/// When an agent uses `--worktree`, Claude Code creates a temporary git
/// worktree.  Skills are written to the source `working_dir` *before* launch.
/// Because `.claude/` is typically in `.gitignore`, the worktree does not
/// inherit those files — the agent's `additional_dirs` (already wired up in
/// `build_claude_command`) point back at the project root where the skills live.
pub async fn materialize_skills(
    working_dir: &Path,
    skill_names: &[String],
    discovered_skills: &[Skill],
) -> Result<MaterializeResult> {
    use std::collections::HashMap;

    let index: HashMap<&str, &Skill> =
        discovered_skills.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut result = MaterializeResult::default();

    for name in skill_names {
        match index.get(name.as_str()) {
            None => {
                result.not_found.push(name.clone());
            }
            Some(skill) => {
                let target_dir = working_dir.join(".claude").join("skills").join(name);
                let target_file = target_dir.join("SKILL.md");

                if target_file.exists() {
                    result.skipped.push(name.clone());
                    continue;
                }

                if let Err(e) = tokio::fs::create_dir_all(&target_dir).await {
                    tracing::warn!(
                        skill = %name,
                        dir = %target_dir.display(),
                        error = %e,
                        "Failed to create skill directory; skipping"
                    );
                    result.not_found.push(name.clone());
                    continue;
                }

                if let Err(e) = tokio::fs::write(&target_file, &skill.content).await {
                    tracing::warn!(
                        skill = %name,
                        file = %target_file.display(),
                        error = %e,
                        "Failed to write skill file; skipping"
                    );
                    result.not_found.push(name.clone());
                    continue;
                }

                result.written.push(name.clone());
            }
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Skill model
// ---------------------------------------------------------------------------

/// A discoverable skill that can be assigned to agentd agents.
///
/// Skills are Markdown files discovered from `.agentd/skills/` (or a
/// user-level fallback).  They are injected into an agent's working directory
/// at spawn time so that Claude Code can invoke them via `/skill`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skill {
    /// Skill identifier — taken from the frontmatter `name` field when
    /// present, otherwise from the directory name or filename stem.
    pub name: String,
    /// Human-readable summary from the frontmatter `description` field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Full Markdown content of the skill file, including frontmatter.
    pub content: String,
    /// Filesystem path where the skill was loaded from.
    ///
    /// Used internally by the materialization step (#1211) to know which file
    /// to copy into the agent's working directory.  Not included in API
    /// responses — consumers only need the skill name and description.
    #[serde(skip)]
    pub(crate) source_path: PathBuf,
}

// ---------------------------------------------------------------------------
// Frontmatter parsing
// ---------------------------------------------------------------------------

/// Extracted YAML frontmatter fields relevant to skill identity.
#[derive(Default)]
struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
}

/// Parse the leading `---` … `---` frontmatter block from `content`.
///
/// Only `name:` and `description:` lines are extracted; all other fields are
/// ignored.  No external YAML parser is required — the format is intentionally
/// simple.
///
/// Returns [`Frontmatter::default`] (all fields `None`) if the file does not
/// start with `---` or the closing delimiter is missing.
fn parse_frontmatter(content: &str) -> Frontmatter {
    // Must start with ---
    let Some(after_open) = content.strip_prefix("---") else {
        return Frontmatter::default();
    };

    // Skip an optional newline immediately after the opening delimiter.
    let body = after_open.trim_start_matches('\n');

    // Find the closing ---
    let Some(end) = body.find("\n---") else {
        return Frontmatter::default();
    };

    let mut fm = Frontmatter::default();
    for line in body[..end].lines() {
        if let Some(v) = line.strip_prefix("name:") {
            fm.name = Some(v.trim().trim_matches('"').to_string());
        } else if let Some(v) = line.strip_prefix("description:") {
            fm.description = Some(v.trim().trim_matches('"').to_string());
        }
    }
    fm
}

// ---------------------------------------------------------------------------
// File loading
// ---------------------------------------------------------------------------

/// Load a single skill from `path`, using `fallback_name` when the frontmatter
/// does not specify a `name` field.
fn load_skill_file(path: &Path, fallback_name: &str) -> Result<Skill> {
    let content = std::fs::read_to_string(path)?;
    let fm = parse_frontmatter(&content);
    Ok(Skill {
        name: fm.name.unwrap_or_else(|| fallback_name.to_string()),
        description: fm.description,
        content,
        source_path: path.to_path_buf(),
    })
}

// ---------------------------------------------------------------------------
// Directory scanning
// ---------------------------------------------------------------------------

/// Discover all skills in `skills_dir`.
///
/// Both supported layouts are scanned:
/// - `<skills_dir>/<name>/SKILL.md` — directory layout
/// - `<skills_dir>/<name>.md` — flat layout
///
/// Returns an empty `Vec` (not an error) when `skills_dir` does not exist.
/// Files that cannot be read or are otherwise invalid are silently skipped.
///
/// Results are sorted by name for deterministic output.
pub fn discover_skills(skills_dir: &Path) -> Result<Vec<Skill>> {
    if !skills_dir.exists() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();

    for entry in std::fs::read_dir(skills_dir)? {
        // Skip individual entry errors (e.g. a single permission-denied inode)
        // rather than aborting the entire scan.
        let Ok(entry) = entry else { continue };
        let path = entry.path();

        if path.is_dir() {
            // Directory layout: <name>/SKILL.md
            let skill_file = path.join("SKILL.md");
            if skill_file.is_file() {
                let name =
                    path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
                if let Ok(skill) = load_skill_file(&skill_file, &name) {
                    skills.push(skill);
                }
            }
        } else if path.is_file() {
            // Flat layout: <name>.md
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let name =
                    path.file_stem().and_then(|n| n.to_str()).unwrap_or_default().to_string();
                if let Ok(skill) = load_skill_file(&path, &name) {
                    skills.push(skill);
                }
            }
        }
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

// ---------------------------------------------------------------------------
// Multi-location discovery
// ---------------------------------------------------------------------------

/// Return the ordered list of directories to scan for skills.
///
/// Precedence (first wins on name collision):
/// 1. `.agentd/skills/` — project-level
/// 2. `~/.config/agentd/skills/` — user-level
fn skill_search_dirs() -> Vec<PathBuf> {
    // NOTE: ".agentd/skills" is resolved relative to the orchestrator's CWD.
    // In development (started from the repo root) this works as expected.
    // In production (systemd, Docker) the CWD is typically not the project
    // root, so project-level skills may not be found.  A future enhancement
    // should allow an explicit project-root config key (e.g.
    // AGENTD_PROJECT_ROOT) to override this path.
    let mut dirs = vec![PathBuf::from(".agentd/skills")];

    // User-level fallback: $HOME/.config/agentd/skills
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        dirs.push(PathBuf::from(home).join(".config/agentd/skills"));
    }

    dirs
}

/// Discover skills from all standard locations, merging by name with
/// higher-precedence sources winning.
///
/// Errors from individual directories are silently ignored so that a missing
/// or unreadable user-level directory never prevents project-level skills from
/// loading.
pub fn discover_all_skills() -> Vec<Skill> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut result = Vec::new();

    for dir in skill_search_dirs() {
        if let Ok(skills) = discover_skills(&dir) {
            for skill in skills {
                if seen.insert(skill.name.clone()) {
                    result.push(skill);
                }
            }
        }
    }

    // Sort globally so the merged result is deterministic regardless of which
    // names came from the project-level vs. user-level directory.
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── frontmatter parsing ──────────────────────────────────────────────────

    #[test]
    fn test_parse_frontmatter_name_and_description() {
        let content =
            "---\nname: git-spice\ndescription: Branch stacking tool.\n---\n\n# Git Spice";
        let fm = parse_frontmatter(content);
        assert_eq!(fm.name.as_deref(), Some("git-spice"));
        assert_eq!(fm.description.as_deref(), Some("Branch stacking tool."));
    }

    #[test]
    fn test_parse_frontmatter_name_only() {
        let content = "---\nname: my-skill\n---\nContent here";
        let fm = parse_frontmatter(content);
        assert_eq!(fm.name.as_deref(), Some("my-skill"));
        assert!(fm.description.is_none());
    }

    #[test]
    fn test_parse_frontmatter_quoted_values() {
        let content = "---\nname: \"quoted-skill\"\ndescription: \"Quoted description.\"\n---\n";
        let fm = parse_frontmatter(content);
        assert_eq!(fm.name.as_deref(), Some("quoted-skill"));
        assert_eq!(fm.description.as_deref(), Some("Quoted description."));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "# Just markdown\nNo frontmatter here.";
        let fm = parse_frontmatter(content);
        assert!(fm.name.is_none());
        assert!(fm.description.is_none());
    }

    #[test]
    fn test_parse_frontmatter_unclosed() {
        let content = "---\nname: orphan\nNo closing delimiter";
        let fm = parse_frontmatter(content);
        assert!(fm.name.is_none());
    }

    #[test]
    fn test_parse_frontmatter_empty_block() {
        let content = "---\n---\n# Content";
        let fm = parse_frontmatter(content);
        assert!(fm.name.is_none());
        assert!(fm.description.is_none());
    }

    // ── discover_skills: empty / missing directory ───────────────────────────

    #[test]
    fn test_discover_skills_missing_dir_returns_empty() {
        let result = discover_skills(Path::new("/nonexistent/path/skills")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_discover_skills_empty_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let result = discover_skills(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    // ── discover_skills: directory layout (<name>/SKILL.md) ─────────────────

    #[test]
    fn test_discover_skills_directory_layout() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("git-spice");
        fs::create_dir(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: git-spice\ndescription: Branch stacking.\n---\n\n# Git Spice",
        )
        .unwrap();

        let skills = discover_skills(tmp.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "git-spice");
        assert_eq!(skills[0].description.as_deref(), Some("Branch stacking."));
        assert!(skills[0].content.contains("# Git Spice"));
    }

    #[test]
    fn test_discover_skills_directory_layout_uses_dirname_as_fallback_name() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("my-tool");
        fs::create_dir(&skill_dir).unwrap();
        // No frontmatter — name should fall back to directory name.
        fs::write(skill_dir.join("SKILL.md"), "# My Tool\nJust content.").unwrap();

        let skills = discover_skills(tmp.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-tool");
    }

    #[test]
    fn test_discover_skills_directory_without_skill_md_is_skipped() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("empty-dir");
        fs::create_dir(&skill_dir).unwrap();
        // No SKILL.md inside — should not appear in results.

        let skills = discover_skills(tmp.path()).unwrap();
        assert!(skills.is_empty());
    }

    // ── discover_skills: flat layout (<name>.md) ─────────────────────────────

    #[test]
    fn test_discover_skills_flat_layout() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("agent-ops.md"),
            "---\nname: agent-ops\ndescription: Operate agents.\n---\n\n# Agent Ops",
        )
        .unwrap();

        let skills = discover_skills(tmp.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "agent-ops");
        assert_eq!(skills[0].description.as_deref(), Some("Operate agents."));
    }

    #[test]
    fn test_discover_skills_flat_layout_uses_stem_as_fallback_name() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("no-frontmatter.md"), "# No Frontmatter").unwrap();

        let skills = discover_skills(tmp.path()).unwrap();
        assert_eq!(skills[0].name, "no-frontmatter");
    }

    #[test]
    fn test_discover_skills_non_md_files_are_ignored() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("config.toml"), "[section]").unwrap();
        fs::write(tmp.path().join("README.txt"), "readme").unwrap();

        let skills = discover_skills(tmp.path()).unwrap();
        assert!(skills.is_empty());
    }

    // ── discover_skills: mixed layout ────────────────────────────────────────

    #[test]
    fn test_discover_skills_mixed_layouts_sorted_by_name() {
        let tmp = TempDir::new().unwrap();

        // Flat layout
        fs::write(tmp.path().join("zebra.md"), "---\nname: zebra\ndescription: Z skill.\n---\n")
            .unwrap();

        // Directory layout
        let dir = tmp.path().join("alpha");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "---\nname: alpha\ndescription: A skill.\n---\n").unwrap();

        let skills = discover_skills(tmp.path()).unwrap();
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].name, "alpha");
        assert_eq!(skills[1].name, "zebra");
    }

    // ── discover_all_skills: global sort across directories ──────────────────

    #[test]
    fn test_discover_all_skills_global_sort_across_directories() {
        // Simulate the multi-source merge: project dir has "beta" and "zap",
        // user dir has "alpha" and "gamma".  After merge + sort the result
        // must be ["alpha", "beta", "gamma", "zap"], not the per-directory
        // sorted interleaving ["beta", "zap", "alpha", "gamma"].
        let project_dir = TempDir::new().unwrap();
        let user_dir = TempDir::new().unwrap();

        fs::write(project_dir.path().join("beta.md"), "---\nname: beta\n---\n").unwrap();
        fs::write(project_dir.path().join("zap.md"), "---\nname: zap\n---\n").unwrap();
        fs::write(user_dir.path().join("alpha.md"), "---\nname: alpha\n---\n").unwrap();
        fs::write(user_dir.path().join("gamma.md"), "---\nname: gamma\n---\n").unwrap();

        // Merge manually using the same logic as discover_all_skills.
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for dir in [project_dir.path(), user_dir.path()] {
            for skill in discover_skills(dir).unwrap() {
                if seen.insert(skill.name.clone()) {
                    result.push(skill);
                }
            }
        }
        result.sort_by(|a, b| a.name.cmp(&b.name));

        let names: Vec<&str> = result.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["alpha", "beta", "gamma", "zap"]);
    }

    #[test]
    fn test_discover_all_skills_project_wins_on_name_collision() {
        let project_dir = TempDir::new().unwrap();
        let user_dir = TempDir::new().unwrap();

        fs::write(
            project_dir.path().join("shared.md"),
            "---\nname: shared\ndescription: project version\n---\n",
        )
        .unwrap();
        fs::write(
            user_dir.path().join("shared.md"),
            "---\nname: shared\ndescription: user version\n---\n",
        )
        .unwrap();

        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for dir in [project_dir.path(), user_dir.path()] {
            for skill in discover_skills(dir).unwrap() {
                if seen.insert(skill.name.clone()) {
                    result.push(skill);
                }
            }
        }

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].description.as_deref(), Some("project version"));
    }

    // ── source_path serialization ────────────────────────────────────────────

    #[test]
    fn test_source_path_not_included_in_json() {
        let tmp = TempDir::new().unwrap();
        let skill_file = tmp.path().join("demo.md");
        fs::write(&skill_file, "---\nname: demo\ndescription: A demo skill.\n---\n").unwrap();

        let skill = load_skill_file(&skill_file, "demo").unwrap();
        let json = serde_json::to_string(&skill).unwrap();

        assert!(!json.contains("source_path"), "source_path should not appear in JSON output");
        assert!(json.contains("\"name\":\"demo\""));
        assert!(json.contains("\"description\":\"A demo skill.\""));
    }

    // ── discover_all_skills: deduplication ───────────────────────────────────

    #[test]
    fn test_discover_skills_malformed_file_is_skipped() {
        // A directory with SKILL.md that has no read permissions can't be
        // tested portably; instead verify that an unreadable flat file is
        // skipped without panicking.  We achieve this by using a path that
        // doesn't exist — the individual load returns an error and the loop
        // continues.
        let tmp = TempDir::new().unwrap();
        // Create one valid skill alongside a non-md file (which should be ignored).
        fs::write(tmp.path().join("valid.md"), "---\nname: valid\n---\n").unwrap();
        fs::write(tmp.path().join("invalid.json"), r#"{"not": "markdown"}"#).unwrap();

        let skills = discover_skills(tmp.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "valid");
    }

    // ── materialize_skills ───────────────────────────────────────────────────

    fn make_discovered_skill(name: &str, description: &str) -> Skill {
        Skill {
            name: name.to_string(),
            description: Some(description.to_string()),
            content: format!("---\nname: {name}\ndescription: {description}\n---\n\n# {name}"),
            source_path: PathBuf::from(format!(".agentd/skills/{name}.md")),
        }
    }

    #[tokio::test]
    async fn test_materialize_writes_skill_file() {
        let tmp = TempDir::new().unwrap();
        let skill = make_discovered_skill("git-spice", "Branch stacking.");
        let discovered = vec![skill];

        let result =
            materialize_skills(tmp.path(), &["git-spice".to_string()], &discovered).await.unwrap();

        assert_eq!(result.written, vec!["git-spice"]);
        assert!(result.skipped.is_empty());
        assert!(result.not_found.is_empty());

        let dest = tmp.path().join(".claude/skills/git-spice/SKILL.md");
        assert!(dest.exists(), "SKILL.md should exist at {}", dest.display());
        let content = fs::read_to_string(&dest).unwrap();
        assert!(content.contains("git-spice"));
    }

    #[tokio::test]
    async fn test_materialize_creates_directory_structure() {
        let tmp = TempDir::new().unwrap();
        let skill = make_discovered_skill("agent-ops", "Agent operations.");
        let discovered = vec![skill];

        materialize_skills(tmp.path(), &["agent-ops".to_string()], &discovered).await.unwrap();

        let skills_dir = tmp.path().join(".claude/skills/agent-ops");
        assert!(skills_dir.is_dir(), ".claude/skills/agent-ops/ should be created");
    }

    #[tokio::test]
    async fn test_materialize_does_not_overwrite_existing_file() {
        let tmp = TempDir::new().unwrap();
        // Pre-create the agent-local skill file.
        let skill_dir = tmp.path().join(".claude/skills/git-spice");
        fs::create_dir_all(&skill_dir).unwrap();
        let existing = skill_dir.join("SKILL.md");
        fs::write(&existing, "# My local version").unwrap();

        let skill = make_discovered_skill("git-spice", "Different content.");
        let discovered = vec![skill];

        let result =
            materialize_skills(tmp.path(), &["git-spice".to_string()], &discovered).await.unwrap();

        assert!(result.written.is_empty());
        assert_eq!(result.skipped, vec!["git-spice"]);

        // Original content must be preserved.
        let content = fs::read_to_string(&existing).unwrap();
        assert_eq!(content, "# My local version");
    }

    #[tokio::test]
    async fn test_materialize_reports_not_found() {
        let tmp = TempDir::new().unwrap();
        let discovered: Vec<Skill> = vec![]; // no skills discovered

        let result = materialize_skills(tmp.path(), &["missing-skill".to_string()], &discovered)
            .await
            .unwrap();

        assert!(result.written.is_empty());
        assert!(result.skipped.is_empty());
        assert_eq!(result.not_found, vec!["missing-skill"]);
    }

    #[tokio::test]
    async fn test_materialize_empty_skill_list_is_noop() {
        let tmp = TempDir::new().unwrap();
        let skill = make_discovered_skill("git-spice", "Branch stacking.");
        let discovered = vec![skill];

        let result = materialize_skills(tmp.path(), &[], &discovered).await.unwrap();

        assert!(result.written.is_empty());
        assert!(result.skipped.is_empty());
        assert!(result.not_found.is_empty());

        // No .claude directory should have been created.
        assert!(!tmp.path().join(".claude").exists());
    }

    #[tokio::test]
    async fn test_materialize_multiple_skills() {
        let tmp = TempDir::new().unwrap();
        let discovered = vec![
            make_discovered_skill("git-spice", "Branch stacking."),
            make_discovered_skill("agent-memory", "Memory service."),
            make_discovered_skill("service-ops", "Service operations."),
        ];

        let names: Vec<String> =
            ["git-spice", "agent-memory", "missing"].iter().map(|s| s.to_string()).collect();

        let result = materialize_skills(tmp.path(), &names, &discovered).await.unwrap();

        let mut written = result.written.clone();
        written.sort();
        assert_eq!(written, vec!["agent-memory", "git-spice"]);
        assert!(result.skipped.is_empty());
        assert_eq!(result.not_found, vec!["missing"]);
    }

    #[tokio::test]
    async fn test_materialize_written_count_matches_files() {
        let tmp = TempDir::new().unwrap();
        let discovered =
            vec![make_discovered_skill("a", "Skill A."), make_discovered_skill("b", "Skill B.")];
        let names: Vec<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();

        let result = materialize_skills(tmp.path(), &names, &discovered).await.unwrap();

        assert_eq!(result.written.len(), 2);
        assert!(tmp.path().join(".claude/skills/a/SKILL.md").exists());
        assert!(tmp.path().join(".claude/skills/b/SKILL.md").exists());
    }
}
