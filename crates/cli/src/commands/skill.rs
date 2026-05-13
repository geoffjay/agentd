//! Skill management commands for the agentd CLI.
//!
//! Provides `agent skill list` and `agent skill show` for inspecting the skills
//! available to agents.  Skills are discovered from the filesystem directly —
//! no orchestrator service call is required.
//!
//! # Discovery paths
//!
//! 1. `.agentd/skills/` — project-level (relative to the current working directory)
//! 2. `~/.config/agentd/skills/` — user-level fallback
//!
//! # Examples
//!
//! ```bash
//! # List all available skills
//! agent skill list
//!
//! # List as JSON (for scripting)
//! agent skill list --json
//!
//! # Show the full content of a skill
//! agent skill show git-spice
//!
//! # Show skill as JSON
//! agent skill show git-spice --json
//! ```

use anyhow::{bail, Result};
use clap::Subcommand;
use colored::*;

/// Skill management subcommands.
#[derive(Subcommand)]
pub enum SkillCommand {
    /// List all discoverable skills from .agentd/skills/
    ///
    /// Scans the project-level `.agentd/skills/` directory and the user-level
    /// `~/.config/agentd/skills/` directory.  Prints a formatted table with
    /// each skill's name and description.
    ///
    /// Use `--json` for machine-readable output.
    List,

    /// Show the full content of a named skill
    ///
    /// Prints the Markdown content (including frontmatter) of the named skill
    /// to stdout.
    Show {
        /// Name of the skill to display
        name: String,
    },
}

impl SkillCommand {
    pub async fn execute(&self, json: bool) -> Result<()> {
        match self {
            SkillCommand::List => list_skills(json),
            SkillCommand::Show { name } => show_skill(name, json),
        }
    }
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn list_skills(json: bool) -> Result<()> {
    let skills = orchestrator::skills::discover_all_skills();

    if json {
        println!("{}", serde_json::to_string_pretty(&skills)?);
        return Ok(());
    }

    if skills.is_empty() {
        println!("{}", "No skills found.".yellow());
        println!(
            "Create skills in {} or {}",
            ".agentd/skills/".cyan(),
            "~/.config/agentd/skills/".cyan()
        );
        return Ok(());
    }

    // Compute column widths.
    let name_width = skills.iter().map(|s| s.name.len()).max().unwrap_or(4).max(4);

    println!("{}", "agentd Skills".bold());
    println!("{}", "=".repeat(60));

    for skill in &skills {
        let desc = skill.description.as_deref().unwrap_or("");
        println!("  {:<width$}  {}", skill.name.cyan(), desc, width = name_width);
    }

    println!();
    println!("{} skill{} available", skills.len(), if skills.len() == 1 { "" } else { "s" });

    Ok(())
}

// ---------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------

fn show_skill(name: &str, json: bool) -> Result<()> {
    let skills = orchestrator::skills::discover_all_skills();

    let skill = match skills.into_iter().find(|s| s.name == name) {
        Some(s) => s,
        None => {
            bail!("Skill '{}' not found. Run 'agent skill list' to see available skills.", name)
        }
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&skill)?);
        return Ok(());
    }

    println!("{}", skill.content);
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_command_list_variant_exists() {
        // Confirm that the SkillCommand enum can be constructed — compile-time check.
        let _cmd = SkillCommand::List;
    }

    #[test]
    fn test_skill_command_show_variant_exists() {
        let _cmd = SkillCommand::Show { name: "git-spice".to_string() };
    }
}
