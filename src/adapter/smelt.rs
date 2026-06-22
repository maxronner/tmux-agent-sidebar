use serde_json::{Map, Value};

use crate::event::{AgentEvent, EventAdapter};
use crate::tmux::SMELT_AGENT;
use crate::tool_name::CanonicalTool;

use super::{json_str, json_value_or_null, optional_str};

pub struct SmeltAdapter;

/// Smelt built-in tool names are lowercase (`bash`, `read_file`, …),
/// while the sidebar's label extractor uses the same Claude-style
/// PascalCase vocabulary for every agent. Normalize once here so
/// activity rendering stays shared across adapters.
fn normalize_tool_name(raw: &str) -> String {
    let canonical = match raw {
        "bash" => CanonicalTool::Bash,
        "read_file" | "read" => CanonicalTool::Read,
        "write_file" | "write" => CanonicalTool::Write,
        "edit_file" | "edit" => CanonicalTool::Edit,
        "grep" => CanonicalTool::Grep,
        "glob" | "find" | "ls" => CanonicalTool::Glob,
        "web_fetch" => CanonicalTool::WebFetch,
        "web_search" => CanonicalTool::WebSearch,
        "tool_search" => CanonicalTool::ToolSearch,
        "skill" => CanonicalTool::Skill,
        "notebook_edit" => CanonicalTool::NotebookEdit,
        "notebook_edit_async" => CanonicalTool::NotebookEdit,
        "ask_user_question" => CanonicalTool::AskUserQuestion,
        "todo_write" => CanonicalTool::TodoWrite,
        "task_create" => CanonicalTool::TaskCreate,
        "task_update" => CanonicalTool::TaskUpdate,
        "task_get" => CanonicalTool::TaskGet,
        "task_stop" => CanonicalTool::TaskStop,
        "task_output" => CanonicalTool::TaskOutput,
        other => return other.to_string(),
    };
    canonical.as_str().to_string()
}

/// Translate smelt built-in tool argument names into the keys expected
/// by the shared label extractor.
fn normalize_tool_input(tool_name: &str, input: Value) -> Value {
    let Value::Object(mut map) = input else {
        return input;
    };
    let rewrites: &[(&str, &str)] = match tool_name {
        // Smelt uses `path` in read/write tools for the file argument;
        // the label extractor expects `file_path`.
        "Read" | "Write" | "Edit" => &[("path", "file_path")],
        _ => &[],
    };
    copy_keys(&mut map, rewrites);
    Value::Object(map)
}

fn copy_keys(map: &mut Map<String, Value>, pairs: &[(&str, &str)]) {
    for (src, dst) in pairs {
        if map.contains_key(*dst) {
            continue;
        }
        if let Some(value) = map.get(*src).cloned() {
            map.insert((*dst).to_string(), value);
        }
    }
}

impl EventAdapter for SmeltAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: SMELT_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                source: json_str(input, "source").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "session-end" => Some(AgentEvent::SessionEnd {
                end_reason: json_str(input, "end_reason").into(),
            }),
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: SMELT_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                prompt: json_str(input, "prompt").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "notification" => Some(AgentEvent::Notification {
                agent: SMELT_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                wait_reason: json_str(input, "wait_reason").into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "stop" => Some(AgentEvent::Stop {
                agent: SMELT_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                last_message: json_str(input, "last_message").into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "stop-failure" => Some(AgentEvent::StopFailure {
                agent: SMELT_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                error: json_str(input, "error").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "activity-log" => {
                let raw_name = json_str(input, "tool_name");
                if raw_name.is_empty() {
                    return None;
                }
                let tool_name = normalize_tool_name(raw_name);
                let tool_input =
                    normalize_tool_input(&tool_name, json_value_or_null(input, "tool_input"));
                Some(AgentEvent::ActivityLog {
                    tool_name,
                    tool_input,
                    tool_response: json_value_or_null(input, "tool_response"),
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn session_start_sets_smelt_agent() {
        let event = SmeltAdapter
            .parse(
                "session-start",
                &json!({"cwd": "/repo", "session_id": "s1", "source": "startup"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: SMELT_AGENT.into(),
                cwd: "/repo".into(),
                permission_mode: "".into(),
                source: "startup".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("s1".into()),
            }
        );
    }

    #[test]
    fn activity_log_normalizes_bash() {
        let event = SmeltAdapter
            .parse(
                "activity-log",
                &json!({
                    "tool_name": "bash",
                    "tool_input": {"command": "cargo test"},
                    "tool_response": {"stdout": "ok"}
                }),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                tool_response,
            } => {
                assert_eq!(tool_name, "Bash");
                assert_eq!(tool_input["command"], "cargo test");
                assert_eq!(tool_response["stdout"], "ok");
            }
            other => panic!("expected ActivityLog, got {other:?}"),
        }
    }

    #[test]
    fn activity_log_normalizes_read_file() {
        let event = SmeltAdapter
            .parse(
                "activity-log",
                &json!({"tool_name": "read_file", "tool_input": {"path": "/repo/src/main.rs"}}),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                ..
            } => {
                assert_eq!(tool_name, "Read");
                assert_eq!(tool_input["path"], "/repo/src/main.rs");
                assert_eq!(tool_input["file_path"], "/repo/src/main.rs");
            }
            other => panic!("expected ActivityLog, got {other:?}"),
        }
    }

    #[test]
    fn activity_log_unrecognized_tool_passes_through() {
        let event = SmeltAdapter
            .parse(
                "activity-log",
                &json!({"tool_name": "custom_tool", "tool_input": {"key": "val"}}),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                ..
            } => {
                assert_eq!(tool_name, "custom_tool");
                assert_eq!(tool_input["key"], "val");
            }
            other => panic!("expected ActivityLog, got {other:?}"),
        }
    }

    #[test]
    fn activity_log_empty_tool_name_returns_none() {
        assert!(SmeltAdapter
            .parse("activity-log", &json!({"tool_name": ""}))
            .is_none(),);
    }

    #[test]
    fn session_end_is_supported() {
        assert_eq!(
            SmeltAdapter.parse("session-end", &json!({"end_reason": "quit"})),
            Some(AgentEvent::SessionEnd {
                end_reason: "quit".into()
            })
        );
    }

    #[test]
    fn notification_is_supported() {
        let event = SmeltAdapter
            .parse(
                "notification",
                &json!({"cwd": "/repo", "wait_reason": "permission", "session_id": "s1"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::Notification {
                agent: SMELT_AGENT.into(),
                cwd: "/repo".into(),
                permission_mode: "".into(),
                wait_reason: "permission".into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: Some("s1".into()),
            }
        );
    }

    #[test]
    fn stop_is_supported() {
        let event = SmeltAdapter
            .parse(
                "stop",
                &json!({"cwd": "/repo", "last_message": "done", "session_id": "s1"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: SMELT_AGENT.into(),
                cwd: "/repo".into(),
                permission_mode: "".into(),
                last_message: "done".into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: Some("s1".into()),
            }
        );
    }

    #[test]
    fn stop_failure_is_supported() {
        let event = SmeltAdapter
            .parse(
                "stop-failure",
                &json!({"cwd": "/repo", "error": "API error"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::StopFailure {
                agent: SMELT_AGENT.into(),
                cwd: "/repo".into(),
                permission_mode: "".into(),
                error: "API error".into(),
                worktree: None,
                agent_id: None,
                session_id: None,
            }
        );
    }

    #[test]
    fn unknown_event_returns_none() {
        assert!(SmeltAdapter.parse("unknown-event", &json!({})).is_none());
    }
}
