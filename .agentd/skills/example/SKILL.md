---
name: example
description: A minimal example skill demonstrating the skill file format. Copy and adapt this template to create your own skills.
---

# Example Skill

This is a minimal example skill. Replace this content with instructions or
reference material for the agent.

## What Skills Are For

Skills are Markdown files that are injected into an agent's `.claude/skills/`
directory at spawn time. Claude Code discovers and can invoke them via the
`/skill` command.

## Skill Format

A skill file consists of:
1. An optional YAML frontmatter block (between `---` delimiters)
2. Markdown content with instructions, examples, or reference material

### Frontmatter Fields

- `name` — override the skill name (defaults to directory/filename stem)
- `description` — shown in `agent skill list` output

## Tips

- Write skills as instructions to the agent, not documentation for humans
- Use code blocks for commands the agent should run
- Keep skills focused on a single domain or tool
- Use `> [!IMPORTANT]` callouts for critical constraints
