# Agent Skills

Agent skills are Markdown files that are automatically injected into an agent's
working directory at spawn time. They extend what Claude Code can do by making
pre-written instructions, workflows, and reference material available to the
agent via the `/skill` command.

## Overview

Skills bridge the gap between generic Claude Code capability and project-specific
or team-specific workflows. For example, a `git-spice` skill teaches an agent
exactly how the project uses git-spice for branch stacking, including naming
conventions, required flags, and TLS workarounds — without that context needing
to appear in every prompt.

**How it works:**

1. You write skill files in `.agentd/skills/` (checked into the repo)
2. Agent templates declare which skills to assign via the `skills` field
3. When `agent apply` spawns the agent, the orchestrator writes assigned skills
   to `<working_dir>/.claude/skills/` before Claude Code starts
4. Claude Code discovers the skill files and makes them available via `/skill`

## Skill File Format

A skill file is a Markdown file with an optional YAML frontmatter block.

### Directory layout (recommended)

```
.agentd/skills/
  git-spice/
    SKILL.md          # <-- skill content here
  agent-memory/
    SKILL.md
```

### Flat layout

```
.agentd/skills/
  git-spice.md        # name derived from filename stem
  agent-memory.md
```

Both layouts are supported. The directory layout is recommended because it
allows you to add supplementary files alongside the skill in the future.

### Frontmatter

```markdown
---
name: git-spice
description: Branch stacking and PR management with git-spice.
---

# Git Spice

Instructions for the agent...
```

**Supported frontmatter fields:**

| Field | Required | Description |
|---|---|---|
| `name` | No | Override the skill name (defaults to directory or filename stem) |
| `description` | No | Short summary shown by `agent skill list` |

## Discovery Paths

agentd scans skill directories in priority order. When the same skill name
appears in multiple locations, the higher-priority source wins.

| Priority | Path | Purpose |
|---|---|---|
| 1 (highest) | `.agentd/skills/` | Project-level skills (checked into the repo) |
| 2 | `~/.config/agentd/skills/` | User-level fallback (personal skills) |

## Assigning Skills to Agents

Skills are assigned in the agent YAML template using the `skills` field.

### Specific skills

```yaml
# .agentd/agents/worker.yml
name: worker
model: claude-sonnet-4-6
skills:
  - git-spice
  - agent-memory
  - service-ops
```

### All discovered skills

```yaml
name: worker
model: claude-sonnet-4-6
skills: all
```

Using `skills: all` expands to every skill discovered from `.agentd/skills/`
and `~/.config/agentd/skills/` at apply time.

### No skills (default)

```yaml
name: worker
model: claude-sonnet-4-6
# skills field omitted — no agentd-managed skills
```

Omitting the `skills` field or setting `skills: []` assigns no skills.

## Materialization

When an agent is spawned, the orchestrator writes assigned skills to:

```
<working_dir>/.claude/skills/<name>/SKILL.md
```

**Important behavior:**

- **Existing files are not overwritten.** If the agent's working directory
  already has a `.claude/skills/<name>/SKILL.md`, the agentd-managed version is
  skipped. This means the agent's own local skills always take precedence.
- **Missing skills produce warnings, not errors.** If a skill listed in the
  template is not found in the discovered skill set, the agent still launches
  but a warning is logged.
- **Directory structure is created automatically.** The `.claude/skills/<name>/`
  directory is created if it does not exist.

### Worktree agents

When `worktree: true`, Claude Code creates a temporary git worktree for the
agent. Because `.claude/` is typically in `.gitignore`, the worktree does not
inherit materialized skills from the main working directory.

Skills are written to the source `working_dir` *before* launch. The agent's
`additional_dirs` configuration (already wired through `build_claude_command`)
points back at the project root, so Claude Code discovers the skills via the
parent directory mechanism.

## CLI Commands

### List available skills

```bash
agent skill list
```

Output:

```
agentd Skills
============================================================
  agent-memory  Store, search, and manage shared knowledge
  example       A minimal example skill
  git-spice     Branch stacking and PR management

3 skills available
```

With `--json` for scripting:

```bash
agent skill list --json
```

```json
[
  {
    "name": "agent-memory",
    "description": "Store, search, and manage shared knowledge",
    "content": "---\nname: agent-memory\n..."
  }
]
```

### Show a skill

```bash
agent skill show git-spice
```

Prints the full Markdown content of the skill to stdout.

```bash
agent skill show git-spice --json
```

Outputs the full `Skill` object as JSON.

## Writing Good Skills

### Agent-facing vs human-facing

Skills are instructions *to the agent*, not documentation for humans. Write
them as you would write a system prompt or a concise reference guide:

- Use imperative voice ("Run X", "Always include Y")
- Use code blocks for commands the agent should execute
- Highlight constraints with `> [!IMPORTANT]` callouts
- Keep each skill focused on a single domain or tool

### Minimal skill example

```markdown
---
name: my-tool
description: How to use my-tool in this project.
---

# my-tool

Run `my-tool --help` to see available subcommands.

## Common Commands

\`\`\`bash
my-tool list
my-tool run <task-name>
\`\`\`

> [!IMPORTANT]
> Always use `--dry-run` first when running in production.
```

### Testing a skill before assigning it

1. Place the skill file in `.agentd/skills/<name>/SKILL.md`
2. Run `agent skill list` to verify it is discovered
3. Run `agent skill show <name>` to verify the content
4. Assign it to an agent template and run `agent apply .agentd/`
5. Verify the file was materialized: `ls <working_dir>/.claude/skills/<name>/`

## Troubleshooting

### Skill not found during `agent apply`

```
Agent 'worker' references unknown skill(s): my-skill.
Run 'agent skill list' to see available skills.
```

**Cause:** The skill name in the `skills:` list does not match any discovered
skill.

**Fix:** Check the skill name by running `agent skill list`. Ensure the skill
file exists in `.agentd/skills/` with the correct directory name or frontmatter
`name` field.

### Skill not appearing in `agent skill list`

1. Check the path: `.agentd/skills/<name>/SKILL.md` or `.agentd/skills/<name>.md`
2. Check that the file has a `.md` extension (other extensions are ignored)
3. Check that the directory contains a `SKILL.md` file (not `skill.md` — the
   filename is case-sensitive on Linux)
4. Run from the project root directory (`.agentd/skills/` is resolved relative
   to the orchestrator's CWD)

### Skill not loading in agent

1. Verify materialization: `ls <working_dir>/.claude/skills/`
2. Check orchestrator logs for `materialized skills` or `skill materialization
   failed` messages
3. If the file already exists, the agentd copy was skipped (agent-local takes
   precedence) — check the existing file at `.claude/skills/<name>/SKILL.md`
4. For worktree agents, skills are in the source working directory, not the
   worktree

### Worktree agents and skills

Skills are written to the source `working_dir` before launch. If the skill
does not appear available inside the worktree, ensure the project root is in
the agent's `additional_dirs` so Claude Code can discover skills from the
parent directory.

## Reference

### `Skill` struct

```rust
pub struct Skill {
    /// Skill identifier — from frontmatter `name` or directory/filename stem.
    pub name: String,
    /// Optional description from frontmatter.
    pub description: Option<String>,
    /// Full Markdown content including frontmatter.
    pub content: String,
}
```

### `MaterializeResult` struct

```rust
pub struct MaterializeResult {
    /// Skills successfully written to .claude/skills/.
    pub written: Vec<String>,
    /// Skills skipped because the target file already existed.
    pub skipped: Vec<String>,
    /// Skills requested but not found in the discovered skill set.
    pub not_found: Vec<String>,
}
```

### API endpoint

```
GET /skills
```

Returns a JSON array of all discovered skills (name + description only —
`source_path` is omitted from API responses):

```json
[
  {"name": "git-spice", "description": "Branch stacking..."},
  {"name": "agent-memory", "description": "Memory service..."}
]
```
