// agentd - Data Transform Functions

fn format_status(value) {
    if value == () { return "disconnected"; }
    if type_of(value) == "map" {
        if "status" in value { return value.status; }
    }
    "connected"
}

fn format_health_status(value) {
    if value == () { return "--"; }
    if type_of(value) == "map" {
        if "status" in value { return value.status; }
    }
    "unknown"
}

fn format_active_count(value) {
    if value == () { return "0"; }
    if type_of(value) == "map" {
        if "agents_active" in value { return "" + value.agents_active; }
    }
    "0"
}

fn format_list_count(value) {
    if value == () { return "0"; }
    if type_of(value) == "array" { return "" + value.len(); }
    "0"
}

fn format_uptime(value) {
    let seconds = value;
    if seconds == () { seconds = 0; }
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    let result = "";
    if hours > 0 { result += "" + hours + "h "; }
    if minutes > 0 || hours > 0 { result += "" + minutes + "m "; }
    result += "" + secs + "s";
    result
}

fn format_agent_table(value) {
    if value == () { return "No agents"; }
    if type_of(value) != "array" { return "No agents"; }
    if value.len() == 0 { return "No agents"; }
    let lines = [];
    for agent in value {
        let name = if "name" in agent { agent.name } else { "unnamed" };
        let status = if "status" in agent { agent.status } else { "unknown" };
        let id = if "id" in agent { agent.id } else { "?" };
        let line = "[" + status + "]  " + name;
        line += "  (" + id + ")";
        lines.push(line);
    }
    lines.reduce(|a, b| a + "\n" + b)
}

fn format_agent_detail(value) {
    if value == () { return "No agents found."; }
    if type_of(value) != "array" { return "No agents found."; }
    if value.len() == 0 { return "No agents found."; }
    let lines = [];
    for agent in value {
        let name = if "name" in agent { agent.name } else { "unnamed" };
        let status = if "status" in agent { agent.status } else { "unknown" };
        let id = if "id" in agent { agent.id } else { "?" };
        let line = name + " [" + status + "]";
        line += "\n  ID: " + id;
        if "config" in agent {
            if "working_dir" in agent.config {
                line += "\n  Dir: " + agent.config.working_dir;
            }
        }
        lines.push(line);
    }
    lines.reduce(|a, b| a + "\n\n" + b)
}

fn format_workflow_table(value) {
    if value == () { return "No workflows"; }
    if type_of(value) != "array" { return "No workflows"; }
    if value.len() == 0 { return "No workflows"; }
    let lines = [];
    for wf in value {
        let name = if "name" in wf { wf.name } else { "unnamed" };
        let enabled = if "enabled" in wf { wf.enabled } else { false };
        let status = if enabled { "enabled" } else { "disabled" };
        lines.push("[" + status + "]  " + name);
    }
    lines.reduce(|a, b| a + "\n" + b)
}

fn format_workflow_detail(value) {
    if value == () { return "No workflows found."; }
    if type_of(value) != "array" { return "No workflows found."; }
    if value.len() == 0 { return "No workflows found."; }
    let lines = [];
    for wf in value {
        let name = if "name" in wf { wf.name } else { "unnamed" };
        let id = if "id" in wf { wf.id } else { "?" };
        let enabled = if "enabled" in wf { wf.enabled } else { false };
        let status = if enabled { "enabled" } else { "disabled" };
        let agent = if "agent_id" in wf { wf.agent_id } else { "?" };
        let interval = if "poll_interval_secs" in wf { wf.poll_interval_secs } else { "?" };
        let line = name + " [" + status + "]";
        line += "\n  ID: " + id;
        line += "\n  Agent: " + agent;
        line += "\n  Poll: " + interval + "s";
        lines.push(line);
    }
    lines.reduce(|a, b| a + "\n\n" + b)
}

fn agent_options(value) {
    if value == () { return ["(no agents)"]; }
    if type_of(value) != "array" { return ["(no agents)"]; }
    let opts = [];
    for agent in value {
        let status = agent_field(agent, "status");
        if status != "running" { continue; }
        let name = agent_field(agent, "name");
        let id = agent_field(agent, "id");
        opts.push(name + " (" + id + ")");
    }
    if opts.len() == 0 { return ["(no running agents)"]; }
    opts
}

fn agent_field(agent, field) {
    if field in agent { return agent[field]; }
    "?"
}

fn format_stream_message(value) {
    if value == () { return "Waiting for agent activity..."; }
    if type_of(value) != "map" { return "" + value; }
    let agent = if "agent_id" in value { value.agent_id } else { "?" };
    let msg_type = if "type" in value { value.type } else { "unknown" };
    if msg_type == "assistant" {
        let content = if "content" in value { value.content } else { "" };
        return "[" + agent + "] " + content;
    }
    if msg_type == "result" {
        let is_error = if "is_error" in value { value.is_error } else { false };
        let s = if is_error { "error" } else { "done" };
        return "[" + agent + "] Task " + s;
    }
    if msg_type == "system" {
        let subtype = if "subtype" in value { value.subtype } else { "" };
        return "[" + agent + "] system: " + subtype;
    }
    "[" + agent + "] " + msg_type
}
