# Additional Directories

By default, an agent can only read and write within its `working_dir`. The **additional directories** feature lets you grant an agent access to other paths on the filesystem. Each added path maps directly to Claude Code's `--add-dir` flag.

Common use cases:

- **Monorepo access** — agent works in one package but needs to read sibling packages
- **Shared config or libraries** — company-wide configs, shared scripts, or tooling directories
- **Reference repositories** — read a related repo without making it the working directory

---

## Configure via YAML template

Add `additional_dirs` to an agent template under `.agentd/agents/`:

```yaml
name: my-agent
working_dir: /path/to/project
additional_dirs:
  - ../shared-libraries      # relative: resolved relative to this YAML file
  - /opt/company/configs     # absolute: used as-is
  - ~/other-project          # tilde: expanded to the home directory
```

### Path resolution

| Path type | Example | Resolved to |
|-----------|---------|-------------|
| Relative | `../shared-libs` | Joined with the YAML file's directory, then canonicalized |
| Absolute | `/opt/configs` | Used as-is |
| Tilde | `~/other-project` | Expanded to home directory |

Relative paths are resolved at `agent apply` time, relative to the directory containing the YAML file — not relative to `working_dir` or `$PWD`.

```yaml
# .agentd/agents/worker.yml
# File is at: /home/user/myproject/.agentd/agents/worker.yml
# working_dir is: /home/user/myproject
additional_dirs:
  - ../../shared       # resolves to /home/user/shared
  - /opt/tools         # used as /opt/tools
```

Apply the template:

```bash
agent apply .agentd/agents/worker.yml
```

---

## Configure via CLI

### At creation time

Pass `--add-dir` one or more times when creating an agent:

```bash
agent orchestrator create-agent \
  --name my-agent \
  --add-dir /path/to/shared-libs \
  --add-dir /opt/company/configs
```

### Add a directory to an existing agent

```bash
agent orchestrator add-dir <AGENT_ID> /path/to/dir
```

The directory must exist on the filesystem at the time of the call. The change is persisted immediately but **takes effect on the next agent restart**.

**Example output:**

```
Agent ID: 550e8400-e29b-41d4-a716-446655440000
Additional Dirs: /path/to/dir
Note: restart the agent for directory changes to take effect.
```

### Remove a directory from an existing agent

```bash
agent orchestrator remove-dir <AGENT_ID> /path/to/dir
```

Removing a path that is not in the list is a no-op. The change **takes effect on the next agent restart**.

---

## Configure via API

### Add a directory

```
POST /agents/{id}/dirs
Content-Type: application/json
```

**Request body:**

```json
{ "path": "/path/to/dir" }
```

**Response:** `200 OK`

```json
{
  "agent_id": "550e8400-e29b-41d4-a716-446655440000",
  "additional_dirs": [
    "/path/to/dir"
  ],
  "requires_restart": true
}
```

**Errors:**
- `404` — agent not found
- `422` — path does not exist or is not a directory

The operation is **idempotent** — adding a path that is already present is a no-op.

**curl example:**

```bash
curl -X POST http://127.0.0.1:17006/agents/<ID>/dirs \
  -H "Content-Type: application/json" \
  -d '{"path": "/path/to/shared-libs"}'
```

---

### Remove a directory

```
DELETE /agents/{id}/dirs
Content-Type: application/json
```

**Request body:**

```json
{ "path": "/path/to/dir" }
```

**Response:** `200 OK`

```json
{
  "agent_id": "550e8400-e29b-41d4-a716-446655440000",
  "additional_dirs": [],
  "requires_restart": true
}
```

**Errors:**
- `404` — agent not found

The operation is **idempotent** — removing a path that is not in the list is a no-op.

**curl example:**

```bash
curl -X DELETE http://127.0.0.1:17006/agents/<ID>/dirs \
  -H "Content-Type: application/json" \
  -d '{"path": "/path/to/shared-libs"}'
```

---

## Restart behavior

Directory changes are persisted to the database immediately but the running agent process is not affected until it is restarted. The API and CLI always set `"requires_restart": true` in the response as a reminder.

To apply the change:

```bash
# Terminate the agent
agent orchestrator delete-agent <AGENT_ID>

# Re-create it (or use your YAML template)
agent apply .agentd/agents/my-agent.yml
```

!!! tip "Workflow agents"
    If the agent is managed by a workflow, the workflow will re-launch the agent automatically after it is terminated.

---

## Docker backend behavior

For agents running in Docker containers, each path in `additional_dirs` is automatically bind-mounted into the container at the same absolute path. The mount is read-write, so the agent can read and write to those directories.

```
Host: /opt/company/configs  →  Container: /opt/company/configs  (bind-mount, rw)
```

Ensure the paths exist on the **host** before starting the agent; the orchestrator validates their existence at creation and add time.

---

## Security considerations

!!! warning "Agents can read and write"
    An agent has full read and write access to every directory in `additional_dirs`. Only add directories that the agent legitimately needs.

- Validate that paths point to the intended directories — avoid accidentally adding sensitive parent directories (e.g., `/home/user` instead of `/home/user/project`).
- For agents with broad filesystem access, consider pairing with a restrictive [tool policy](tool-policies.md) that limits which tools the agent can use.
- In Docker environments the bind-mounts are read-write; there is no current support for read-only additional directories.
