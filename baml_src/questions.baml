// Question generation BAML functions for agentd ask service
//
// This module provides AI-powered question generation for the ask service,
// enabling contextual, intelligent questions based on system state and user behavior.

// ============================================================================
// Data Models
// ============================================================================

/// Generated system question with context and suggestions
class SystemQuestion {
  /// The question text to present to the user
  question_text string

  /// Suggested responses (e.g., ["yes", "no", "later"], or specific options)
  suggested_responses string[]

  /// Brief explanation of why this question is being asked
  reasoning string

  /// Urgency level: "low", "normal", "high", "urgent"
  urgency string

  /// Follow-up actions the system might take based on user response
  follow_up_actions map<string, string>

  /// Additional context or help text for the user
  help_text string?
}

/// Analysis of user's previous answer to improve future questions
class AnswerAnalysis {
  /// Interpreted meaning of the user's answer
  interpretation string

  /// Confidence in interpretation (0.0 - 1.0)
  confidence float

  /// Suggested system action based on this answer
  suggested_action string

  /// Whether a follow-up question might be needed
  needs_followup bool

  /// Optional follow-up question if needed
  followup_question string?
}

/// Question effectiveness feedback for learning
class QuestionFeedback {
  /// Whether the question was clear and actionable
  was_clear bool

  /// Whether the user found the question relevant
  was_relevant bool

  /// Suggestions for improving this question type
  improvement_suggestions string[]

  /// Optimal time to ask this question (if timing was an issue)
  better_timing string?
}

// ============================================================================
// Functions
// ============================================================================

/// Generate a context-aware system question
///
/// Creates intelligent questions based on system state, user context, and history.
/// Used by the ask service to replace hardcoded question templates.
///
/// # Arguments
/// - check_type: Type of system check (e.g., "tmux_sessions", "disk_space", "service_health")
/// - system_state: Current system state (e.g., "0 tmux sessions running", "80% disk usage")
/// - user_context: Information about user's current activity and preferences
/// - recent_history: Recent relevant events or user actions
///
/// # Returns
/// SystemQuestion with question text, suggested responses, and follow-up actions
function GenerateSystemQuestion(
  check_type: string,
  system_state: string,
  user_context: string,
  recent_history: string
) -> SystemQuestion {
  client AgentdPrimary
  prompt #"
    Generate a contextual question for the user based on system state.

    **Check Type:** {{ check_type }}

    **Current System State:**
    {{ system_state }}

    **User Context:**
    {{ user_context }}

    **Recent History:**
    {{ recent_history }}

    **Guidelines for Question Generation:**

    1. **Clarity**: Question should be specific and unambiguous
    2. **Actionability**: User should clearly understand what action to take
    3. **Context**: Reference relevant information from system state and history
    4. **Brevity**: Keep question concise, save details for help text
    5. **Suggestions**: Provide 2-5 suggested responses that cover common cases

    **Example Questions (for inspiration):**
    - For tmux check: "No tmux sessions are running. Would you like to start your usual development session?"
    - For disk space: "Disk usage is at 85%. Should I clean up old logs and temporary files?"
    - For service health: "The notification service hasn't responded in 5 minutes. Should I restart it?"

    **Follow-up Actions:**
    Map each suggested response to a concrete system action:
    - What should happen if user says "yes"?
    - What should happen if user says "no"?
    - What about other responses?

    Generate the question structure now.

    {{ ctx.output_format }}
  "#
}

/// Analyze user's answer to a question
///
/// Interprets the user's answer, extracting intent and determining next steps.
/// Handles various answer formats (yes/no, free-form text, etc.)
///
/// # Arguments
/// - original_question: The question that was asked
/// - user_answer: The user's response
/// - expected_response_type: Type of expected response (e.g., "yes_no", "free_form", "choice")
///
/// # Returns
/// AnswerAnalysis with interpretation, confidence, and suggested action
function AnalyzeAnswer(
  original_question: string,
  user_answer: string,
  expected_response_type: string
) -> AnswerAnalysis {
  client AgentdFast
  prompt #"
    Analyze the user's answer to determine intent and next steps.

    **Original Question:**
    {{ original_question }}

    **User's Answer:**
    "{{ user_answer }}"

    **Expected Response Type:** {{ expected_response_type }}

    **Analysis Guidelines:**

    1. **Interpretation**: What is the user's intent?
       - Handle variations (e.g., "yeah", "sure", "ok" all mean "yes")
       - Detect uncertainty (e.g., "maybe", "I'm not sure")
       - Recognize qualified answers (e.g., "yes, but only if...")

    2. **Confidence**: How confident are you in the interpretation?
       - High (0.9-1.0): Clear, unambiguous answer
       - Medium (0.6-0.9): Reasonable interpretation, some ambiguity
       - Low (0.0-0.6): Unclear answer, may need clarification

    3. **Action**: What should the system do next?
       - Be specific and actionable
       - Consider edge cases
       - Handle failure scenarios

    4. **Follow-up**: Does this answer require additional clarification?

    Analyze the answer now.

    {{ ctx.output_format }}
  "#
}

/// Generate a clarifying follow-up question
///
/// When a user's answer is ambiguous or unclear, generate a follow-up
/// question to clarify their intent.
///
/// # Arguments
/// - original_question: The original question
/// - original_answer: The user's ambiguous answer
/// - ambiguity_reason: Why the answer needs clarification
///
/// # Returns
/// SystemQuestion with clarifying question and updated context
function GenerateFollowUpQuestion(
  original_question: string,
  original_answer: string,
  ambiguity_reason: string
) -> SystemQuestion {
  client AgentdFast
  prompt #"
    Generate a clarifying follow-up question.

    **Original Question:**
    {{ original_question }}

    **User's Answer:**
    "{{ original_answer }}"

    **Why Clarification Needed:**
    {{ ambiguity_reason }}

    **Guidelines:**
    - Reference the original context
    - Be specific about what needs clarification
    - Offer concrete options
    - Keep it brief and focused

    **Example:**
    Original: "Should I restart the service?"
    Answer: "maybe"
    Follow-up: "I understand you're unsure. The service has been unresponsive for 10 minutes. Would you like me to: 1) Restart it now, 2) Wait 5 more minutes, or 3) Just alert you if it doesn't recover?"

    Generate the follow-up question now.

    {{ ctx.output_format }}
  "#
}

/// Evaluate question effectiveness for continuous improvement
///
/// Analyzes how effective a question was based on user response patterns,
/// providing feedback for improving future questions.
///
/// # Arguments
/// - question_text: The question that was asked
/// - response_time_seconds: How long it took the user to respond
/// - user_answer: The user's answer
/// - system_outcome: What happened after the answer (success/failure)
///
/// # Returns
/// QuestionFeedback with evaluation and improvement suggestions
function EvaluateQuestionEffectiveness(
  question_text: string,
  response_time_seconds: int,
  user_answer: string,
  system_outcome: string
) -> QuestionFeedback {
  client AgentdFast
  prompt #"
    Evaluate how effective this question was.

    **Question:** {{ question_text }}

    **User Response Time:** {{ response_time_seconds }} seconds

    **User's Answer:** "{{ user_answer }}"

    **System Outcome:** {{ system_outcome }}

    **Evaluation Criteria:**

    1. **Clarity**: Was the question clear?
       - Very quick response (< 10s) suggests clarity
       - Very slow response (> 120s) might indicate confusion
       - Non-standard answers might indicate unclear options

    2. **Relevance**: Was the question timely and relevant?
       - Successful outcome suggests good relevance
       - Failed outcome might indicate poor timing or context

    3. **Improvements**: How could this question be better?
       - Different phrasing?
       - Better timing?
       - More/fewer response options?
       - Additional context?

    Provide honest, actionable feedback.

    {{ ctx.output_format }}
  "#
}

/// Generate a personalized question based on user preferences
///
/// Creates questions tailored to the user's communication style and preferences,
/// learned from previous interactions.
///
/// # Arguments
/// - base_question: The basic question content
/// - user_preferences: User's learned preferences (verbosity, formality, etc.)
/// - interaction_history: Summary of past interactions
///
/// # Returns
/// SystemQuestion adapted to user's preferences
function PersonalizeQuestion(
  base_question: string,
  user_preferences: string,
  interaction_history: string
) -> SystemQuestion {
  client AgentdFast
  prompt #"
    Personalize this question for the user.

    **Base Question:**
    {{ base_question }}

    **User Preferences:**
    {{ user_preferences }}

    **Interaction History:**
    {{ interaction_history }}

    **Personalization Guidelines:**

    1. **Tone Matching**:
       - Formal user → Use professional language
       - Casual user → Use conversational tone
       - Technical user → Include technical details

    2. **Verbosity**:
       - Prefers brief → Short question, minimal help text
       - Prefers detailed → Comprehensive context and options

    3. **Response Style**:
       - Quick decider → Binary choices (yes/no)
       - Thoughtful → Multiple options with context

    4. **Learning from History**:
       - What question patterns work well for this user?
       - What response types do they prefer?

    Adapt the question now.

    {{ ctx.output_format }}
  "#
}

// ============================================================================
// Tests
// ============================================================================

test generate_tmux_session_question {
  functions [GenerateSystemQuestion]
  args {
    check_type "tmux_sessions"
    system_state "0 tmux sessions running, tmux is installed"
    user_context "User is in terminal, last tmux session ended 2 hours ago"
    recent_history "User typically runs 2-3 tmux sessions during work hours. Previous session was named 'dev-main'."
  }
}

test generate_service_health_question {
  functions [GenerateSystemQuestion]
  args {
    check_type "service_health"
    system_state "agentd-notify service not responding, last heartbeat 5 minutes ago"
    user_context "User is actively using the system, notifications are enabled"
    recent_history "Service has been stable for 3 days. No recent configuration changes."
  }
}

test analyze_clear_yes_answer {
  functions [AnalyzeAnswer]
  args {
    original_question "Should I restart the notification service?"
    user_answer "yes"
    expected_response_type "yes_no"
  }
}

test analyze_ambiguous_answer {
  functions [AnalyzeAnswer]
  args {
    original_question "Would you like to start a new tmux session?"
    user_answer "maybe later"
    expected_response_type "yes_no"
  }
}

test generate_followup_for_ambiguous {
  functions [GenerateFollowUpQuestion]
  args {
    original_question "Should I clean up old log files? Disk is 85% full."
    original_answer "depends how old"
    ambiguity_reason "User's answer doesn't clearly indicate yes/no, needs age threshold"
  }
}

test evaluate_effective_question {
  functions [EvaluateQuestionEffectiveness]
  args {
    question_text "Notification service is down. Restart it now?"
    response_time_seconds 3
    user_answer "yes"
    system_outcome "Service restarted successfully and is responding normally"
  }
}

test evaluate_ineffective_question {
  functions [EvaluateQuestionEffectiveness]
  args {
    question_text "Something might be wrong. What should I do?"
    response_time_seconds 45
    user_answer "I don't know, what's wrong?"
    system_outcome "No action taken, user confused about the situation"
  }
}

test personalize_for_technical_user {
  functions [PersonalizeQuestion]
  args {
    base_question "A service is not responding. Should I restart it?"
    user_preferences "Technical user, prefers detailed information, uses technical terminology"
    interaction_history "Usually asks for more details before deciding. Prefers seeing error codes and logs."
  }
}
