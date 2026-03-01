// agentd - UI Event Handlers

// Navigation handler: hides all pages, shows the matching page.
fn on_nav(component_id, event_data) {
    set_component_property("page_dashboard", "visible", false);
    set_component_property("page_agents", "visible", false);
    set_component_property("page_workflows", "visible", false);
    set_component_property("page_activity", "visible", false);

    let page_id = replace(component_id, "nav_", "page_");
    set_component_property(page_id, "visible", true);
}

// Create agent from form inputs.
fn on_create_agent(component_id, event_data) {
    let name = get_component_property("agent_name", "value");
    let working_dir = get_component_property("agent_working_dir", "value");
    let user = get_component_property("agent_user", "value");
    let prompt = get_component_property("agent_prompt", "value");
    let worktree = get_component_property("agent_worktree", "checked");

    if name == () || name == "" {
        name = "nemo-agent";
    }
    if working_dir == () || working_dir == "" {
        set_component_property("agent_form_status", "text", "Working directory is required.");
        return;
    }
    if worktree == () { worktree = false; }

    let body = build_agent_json(name, working_dir, user, prompt, worktree);

    set_component_property("agent_form_status", "text", "Creating agent...");

    let result = http_post("http://127.0.0.1:17006/agents", body);

    if "error" in result {
        set_component_property("agent_form_status", "text", "Error: " + result.error);
        return;
    }

    if result.ok {
        let agent = result.body;
        let id = if "id" in agent { agent.id } else { "?" };
        set_component_property("agent_form_status", "text", "Agent created: " + id);
    } else {
        let msg = get_error_msg(result);
        set_component_property("agent_form_status", "text", "Failed: " + msg);
    }
}

fn build_agent_json(name, working_dir, user, prompt, worktree) {
    let body = "{";
    body += json_field("name", name) + ",";
    body += json_field("working_dir", working_dir) + ",";
    body += json_field("shell", "zsh") + ",";
    body += "\"worktree\":" + worktree;
    if user != () && user != "" {
        body += "," + json_field("user", user);
    }
    if prompt != () && prompt != "" {
        body += "," + json_field("prompt", prompt);
    }
    body += "}";
    body
}

// Refresh agents list.
fn on_refresh_agents(component_id, event_data) {
    let result = http_get("http://127.0.0.1:17006/agents");
    if result.ok {
        let text = format_agent_detail(result.body);
        set_component_property("agents_detail_list", "text", text);
    } else {
        set_component_property("agents_detail_list", "text", "Failed to refresh");
    }
}

// Create workflow from form inputs.
fn on_create_workflow(component_id, event_data) {
    let name = get_component_property("wf_name", "value");
    let agent_val = get_component_property("wf_agent_select", "value");
    let owner = get_component_property("wf_owner", "value");
    let repo = get_component_property("wf_repo", "value");
    let labels_str = get_component_property("wf_labels", "value");
    let template = get_component_property("wf_template", "value");
    let interval_str = get_component_property("wf_interval", "value");

    if name == () || name == "" {
        set_component_property("wf_form_status", "text", "Name is required.");
        return;
    }
    if agent_val == () || agent_val == "" || agent_val == "(none)" {
        set_component_property("wf_form_status", "text", "Select an agent.");
        return;
    }
    if owner == () || owner == "" {
        set_component_property("wf_form_status", "text", "GitHub owner is required.");
        return;
    }
    if repo == () || repo == "" {
        set_component_property("wf_form_status", "text", "GitHub repo is required.");
        return;
    }
    if template == () || template == "" {
        template = "Fix issue #{{source_id}}: {{title}}\n\n{{body}}";
    }

    let agent_id = extract_agent_id(agent_val);
    if agent_id == "" {
        set_component_property("wf_form_status", "text", "Could not parse agent ID.");
        return;
    }

    let labels_json = build_labels_json(labels_str);

    let interval = 60;
    if interval_str != () && interval_str != "" {
        interval = parse_int(interval_str);
        if interval < 10 { interval = 10; }
    }

    let body = build_workflow_json(name, agent_id, owner, repo, labels_json, template, interval);

    set_component_property("wf_form_status", "text", "Creating workflow...");

    let result = http_post("http://127.0.0.1:17006/workflows", body);

    if "error" in result {
        set_component_property("wf_form_status", "text", "Error: " + result.error);
        return;
    }

    if result.ok {
        let wf = result.body;
        let id = if "id" in wf { wf.id } else { "?" };
        set_component_property("wf_form_status", "text", "Workflow created: " + id);
    } else {
        let msg = get_error_msg(result);
        set_component_property("wf_form_status", "text", "Failed: " + msg);
    }
}

fn build_workflow_json(name, agent_id, owner, repo, labels_json, template, interval) {
    let body = "{";
    body += json_field("name", name);
    body += "," + json_field("agent_id", agent_id);

    // source_config object
    let sc = "{";
    sc += json_field("type", "github_issues");
    sc += "," + json_field("owner", owner);
    sc += "," + json_field("repo", repo);
    sc += ",\"labels\":" + labels_json;
    sc += "," + json_field("state", "open");
    sc += "}";
    body += ",\"source_config\":" + sc;

    body += "," + json_field("prompt_template", template);
    body += ",\"poll_interval_secs\":" + interval;
    body += "}";
    body
}

fn build_labels_json(labels_str) {
    if labels_str == () || labels_str == "" {
        return "[]";
    }
    let items = [];
    let remaining = labels_str;
    loop {
        let idx = remaining.index_of(",");
        if idx < 0 {
            let t = trim(remaining);
            if t != "" { items.push(t); }
            break;
        }
        let part = remaining.sub_string(0, idx);
        let t = trim(part);
        if t != "" { items.push(t); }
        remaining = remaining.sub_string(idx + 1);
    }
    if items.len() == 0 { return "[]"; }
    let parts = [];
    for item in items {
        let escaped = esc(item);
        parts.push("\"" + escaped + "\"");
    }
    "[" + parts.reduce(|a, b| a + "," + b) + "]"
}

// Refresh workflows list.
fn on_refresh_workflows(component_id, event_data) {
    let result = http_get("http://127.0.0.1:17006/workflows");
    if result.ok {
        let text = format_workflow_detail(result.body);
        set_component_property("wf_detail_list", "text", text);
    } else {
        set_component_property("wf_detail_list", "text", "Failed to refresh");
    }
}

// -- Helpers --

// Build a JSON key-value pair: "key":"escaped_value"
fn json_field(key, value) {
    let v = esc(value);
    "\"" + key + "\":\"" + v + "\""
}

// Escape a string for JSON.
fn esc(s) {
    let r = replace(s, "\\", "\\\\");
    r = replace(r, "\"", "\\\"");
    r = replace(r, "\n", "\\n");
    r = replace(r, "\r", "\\r");
    r = replace(r, "\t", "\\t");
    r
}

// Extract agent_id from "name (uuid)" format.
fn extract_agent_id(val) {
    let start = val.index_of("(");
    let end = val.index_of(")");
    if start >= 0 && end > start {
        let len = end - start - 1;
        return val.sub_string(start + 1, len);
    }
    if val.len() == 36 {
        return val;
    }
    ""
}

// Get error message from an HTTP result.
fn get_error_msg(result) {
    if type_of(result.body) == "map" {
        if "error" in result.body {
            return result.body.error;
        }
    }
    "HTTP " + result.status
}
