// CLI natural language processing BAML functions for agentd
//
// This module enables natural language interaction with the agent CLI,
// allowing users to express commands in conversational language instead
// of remembering exact command syntax.

// ============================================================================
// Data Models
// ============================================================================

/// Parsed command intent from natural language input
class CommandIntent {
  /// The identified action: "create_notification", "list_notifications",
  /// "get_notification", "delete_notification", "respond_to_notification",
  /// "trigger_ask_check", "answer_question", "get_service_status"
  action string

  /// Extracted parameters for the command (e.g., {"title": "Meeting", "priority": "high"})
  parameters map<string, string>

  /// Confidence in the intent parsing (0.0 - 1.0)
  confidence float

  /// Whether the command should ask for confirmation before executing
  requires_confirmation bool

  /// Human-readable explanation of what will be executed
  execution_summary string

  /// Warning messages if the command might have unintended consequences
  warnings string[]?
}

/// Suggestion for command completion or correction
class CommandSuggestion {
  /// Suggested command to run
  suggested_command string

  /// Explanation of what this command does
  explanation string

  /// Confidence that this is what the user intended
  confidence float

  /// Alternative suggestions
  alternatives string[]?
}

/// Help information for natural language query
class HelpResponse {
  /// Direct answer to the user's question
  answer string

  /// Relevant examples showing how to accomplish the task
  examples string[]

  /// Related commands or topics
  related_topics string[]

  /// Whether this requires additional context from user
  needs_more_info bool

  /// Follow-up questions to clarify user's intent
  followup_questions string[]?
}

// ============================================================================
// Functions
// ============================================================================

/// Parse natural language input into a structured command
///
/// Interprets user's natural language input and maps it to specific
/// agent CLI commands with appropriate parameters.
///
/// # Arguments
/// - user_input: The natural language command from the user
/// - current_context: Current working context (e.g., current directory, recent commands)
///
/// # Returns
/// CommandIntent with parsed action, parameters, and confidence
function ParseNaturalLanguageCommand(
  user_input: string,
  current_context: string
) -> CommandIntent {
  client AgentdFast
  prompt #"
    Parse this natural language command for the agent CLI:

    **User Input:** "{{ user_input }}"

    **Current Context:** {{ current_context }}

    **Available Actions:**

    1. **create_notification**: Create a new notification
       Parameters: title, message, priority (low/normal/high/urgent), lifetime (ephemeral/persistent)
       Examples: "remind me to deploy", "create urgent notification about database"

    2. **list_notifications**: List notifications
       Parameters: filter (all/actionable/urgent), status (pending/responded/dismissed)
       Examples: "show me urgent notifications", "list all pending notifications"

    3. **get_notification**: Get details of a specific notification
       Parameters: notification_id or description
       Examples: "show notification about deployment", "get details of last notification"

    4. **delete_notification**: Delete a notification
       Parameters: notification_id or description
       Examples: "delete the database notification", "remove all old notifications"

    5. **respond_to_notification**: Respond to a notification that requires response
       Parameters: notification_id, response_text
       Examples: "answer the deployment question with yes", "respond to last notification: approved"

    6. **trigger_ask_check**: Trigger a system check
       Parameters: check_type (optional, e.g., "tmux_sessions")
       Examples: "check tmux", "run system checks"

    7. **answer_question**: Answer a pending question
       Parameters: question_id or description, answer
       Examples: "answer the tmux question with yes", "respond to system question: no"

    8. **get_service_status**: Check status of services
       Parameters: service_name (optional)
       Examples: "are services running", "check notify service status"

    **Parsing Guidelines:**

    1. **Extract Intent**: What is the user trying to do?
    2. **Extract Parameters**: Pull out relevant values (titles, priorities, filters, etc.)
    3. **Handle Ambiguity**:
       - If unclear, set confidence lower and requires_confirmation true
       - Provide clear execution summary
    4. **Safety Checks**:
       - Deletions should require confirmation
       - Batch operations should warn user
    5. **Context Usage**:
       - "last notification", "recent alert" → use context to resolve
       - Relative references → interpret based on current state

    **Confidence Levels:**
    - High (0.9-1.0): Clear, unambiguous command
    - Medium (0.6-0.9): Reasonable interpretation, slight ambiguity
    - Low (0.0-0.6): Multiple interpretations possible, needs confirmation

    Parse the command now.

    {{ ctx.output_format }}
  "#
}

/// Suggest command corrections or completions
///
/// When user input is unclear or potentially incorrect, suggest
/// what they might have meant to type.
///
/// # Arguments
/// - user_input: The user's attempted command (possibly malformed)
/// - error_message: Error message from attempted execution (if any)
///
/// # Returns
/// CommandSuggestion with corrected command and explanation
function SuggestCommandCorrection(
  user_input: string,
  error_message: string
) -> CommandSuggestion {
  client AgentdFast
  prompt #"
    The user tried this command which failed:

    **Input:** "{{ user_input }}"

    **Error:** {{ error_message }}

    **Your Task:** Suggest what the user likely meant to do.

    **Common Issues:**

    1. **Typos**: "notif" → "notification", "lst" → "list"
    2. **Wrong Action**: "show notifications" when they meant "list notifications"
    3. **Missing Parameters**: "create notification" without title/message
    4. **Invalid Parameter Values**: priority "super high" → "urgent"
    5. **Wrong Command Format**: Using flags that don't exist

    **Suggestion Guidelines:**

    - Suggest the corrected command
    - Explain what it does
    - Provide 1-2 alternatives if multiple interpretations exist
    - High confidence if fix is obvious, lower if uncertain

    Provide suggestion now.

    {{ ctx.output_format }}
  "#
}

/// Provide help for natural language queries
///
/// Answers user questions about how to use the CLI or accomplish
/// specific tasks.
///
/// # Arguments
/// - user_question: The user's question
/// - available_commands: List of available commands with descriptions
///
/// # Returns
/// HelpResponse with answer, examples, and related topics
function ProvideNaturalLanguageHelp(
  user_question: string,
  available_commands: string
) -> HelpResponse {
  client AgentdPrimary
  prompt #"
    Answer this user's question about the agent CLI:

    **Question:** "{{ user_question }}"

    **Available Commands:**
    {{ available_commands }}

    **Help Guidelines:**

    1. **Direct Answer**: Concisely answer the question
    2. **Examples**: Provide 2-3 practical examples
    3. **Related Topics**: Suggest related commands or features
    4. **Clarification**: If question is vague, ask follow-up questions

    **Types of Questions:**

    - **How do I...?** → Provide command examples
    - **What is...?** → Explain concept/feature
    - **Can I...?** → Yes/no + how to do it if possible
    - **Why...?** → Explain reasoning or design decision

    **Example Response Style:**

    Question: "How do I create an urgent notification?"
    Answer: "You can create an urgent notification using the `create` command with the `--priority urgent` flag..."
    Examples:
    - `agent notify create --title "Critical" --message "Server down" --priority urgent`
    - `agent "create urgent notification about database failure"`
    Related: notification priorities, notification lifetimes

    Provide helpful response now.

    {{ ctx.output_format }}
  "#
}

/// Explain what a command will do before execution
///
/// Generates a human-readable explanation of a command's effects,
/// useful for confirmation prompts or educational purposes.
///
/// # Arguments
/// - command_string: The command to explain
/// - current_context: Current system state for context-specific explanations
///
/// # Returns
/// String with clear explanation of command effects
function ExplainCommand(
  command_string: string,
  current_context: string
) -> string {
  client AgentdFast
  prompt #"
    Explain what this command will do:

    **Command:** `{{ command_string }}`

    **Current Context:** {{ current_context }}

    **Explanation Guidelines:**

    1. **What**: What action will be performed?
    2. **Where**: What objects/resources will be affected?
    3. **Impact**: What will change in the system?
    4. **Reversibility**: Can this be undone? If so, how?

    **Example Explanations:**

    Command: `agent notify delete abc123`
    Explanation: "This will permanently delete the notification with ID abc123 titled 'Database backup completed'. This action cannot be undone."

    Command: `agent ask trigger`
    Explanation: "This will run all registered system checks and create notifications for any questions that need answering. Currently enabled checks: tmux sessions."

    Provide clear, concise explanation in 1-2 sentences.

    {{ ctx.output_format }}
  "#
}

/// Generate command aliases or shortcuts
///
/// Suggests memorable aliases for frequently used commands based
/// on user's command history and patterns.
///
/// # Arguments
/// - command_history: List of recent commands the user has run
/// - frequency_map: Map of commands to how often they're used
///
/// # Returns
/// Map of suggested aliases to their full commands with explanations
function SuggestCommandAliases(
  command_history: string[],
  frequency_map: map<string, int>
) -> map<string, string> {
  client AgentdFast
  prompt #"
    Analyze this command history and suggest useful aliases:

    **Recent Commands:**
    {% for cmd in command_history %}
    - {{ cmd }}
    {% endfor %}

    **Frequency Data:**
    {{ frequency_map }}

    **Alias Suggestion Guidelines:**

    1. **Frequency**: Prioritize frequently used commands
    2. **Length**: Longer commands benefit more from aliases
    3. **Memorability**: Aliases should be intuitive and short
    4. **Patterns**: Look for common parameter combinations

    **Example Aliases:**

    - `agent notify list --actionable` → `agent pending` or `agent todo`
    - `agent notify create --priority urgent` → `agent urgent-notify`
    - `agent ask trigger` → `agent check`

    Return a map of alias → full command with explanation.
    Format: "alias-name": "full command | Explanation: why this is useful"

    Maximum 5 suggestions, only for commands used 3+ times.

    {{ ctx.output_format }}
  "#
}

// ============================================================================
// Tests
// ============================================================================

test parse_simple_create_notification {
  functions [ParseNaturalLanguageCommand]
  args {
    user_input "remind me to deploy the application tomorrow"
    current_context "Current time: 2024-01-15 14:00, working directory: /projects/myapp"
  }
}

test parse_list_notifications {
  functions [ParseNaturalLanguageCommand]
  args {
    user_input "show me all urgent notifications"
    current_context "User has 15 total notifications, 3 are urgent"
  }
}

test parse_ambiguous_delete {
  functions [ParseNaturalLanguageCommand]
  args {
    user_input "delete old notifications"
    current_context "User has 10 notifications from last week, 5 from yesterday"
  }
}

test suggest_correction_typo {
  functions [SuggestCommandCorrection]
  args {
    user_input "notif list"
    error_message "Unknown command 'notif'. Did you mean 'notify'?"
  }
}

test suggest_correction_missing_param {
  functions [SuggestCommandCorrection]
  args {
    user_input "agent notify create"
    error_message "Missing required argument '--title'"
  }
}

test help_how_to_create {
  functions [ProvideNaturalLanguageHelp]
  args {
    user_question "How do I create a high priority notification that persists?"
    available_commands "notify create [--title] [--message] [--priority low|normal|high|urgent] [--lifetime ephemeral|persistent]"
  }
}

test help_what_is {
  functions [ProvideNaturalLanguageHelp]
  args {
    user_question "What is an ephemeral notification?"
    available_commands "notify create [--lifetime ephemeral|persistent] - ephemeral notifications auto-dismiss after a timeout"
  }
}

test explain_delete_command {
  functions [ExplainCommand]
  args {
    command_string "agent notify delete --all"
    current_context "User has 25 notifications (3 urgent, 12 normal, 10 low)"
  }
}

test explain_create_command {
  functions [ExplainCommand]
  args {
    command_string "agent notify create --title 'Deploy' --priority urgent --lifetime persistent"
    current_context "User currently has 5 notifications, none about deployment"
  }
}

test suggest_aliases {
  functions [SuggestCommandAliases]
  args {
    command_history [
      "agent notify list --actionable",
      "agent notify list --actionable",
      "agent notify list --actionable",
      "agent notify list --actionable",
      "agent notify create --priority urgent --lifetime persistent",
      "agent notify create --priority urgent --lifetime persistent",
      "agent notify create --priority urgent --lifetime persistent",
      "agent ask trigger",
      "agent ask trigger"
    ]
    frequency_map {
      "notify list --actionable": 4,
      "notify create --priority urgent --lifetime persistent": 3,
      "ask trigger": 2
    }
  }
}
