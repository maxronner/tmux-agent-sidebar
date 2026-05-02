use serde_json::{Map, Value};

use crate::event::{AgentEvent, EventAdapter};
use crate::tmux::PI_AGENT;
use crate::tool_name::CanonicalTool;

use super::{json_str, json_value_or_null, optional_str};

pub struct PiAdapter;

/// Pi built-in tool names are lowercase (`bash`, `read`, …), while the
/// sidebar's label extractor uses the same Claude-style PascalCase
/// vocabulary for every agent. Normalize once here so activity rendering
/// stays shared across adapters.
fn normalize_tool_name(raw: &str) -> String {
    let canonical = match raw {
        "bash" => CanonicalTool::Bash,
        "read" => CanonicalTool::Read,
        "write" => CanonicalTool::Write,
        "edit" => CanonicalTool::Edit,
        "grep" => CanonicalTool::Grep,
        "find" | "ls" => CanonicalTool::Glob,
        other => return other.to_string(),
    };
    canonical.as_str().to_string()
}

/// Translate Pi built-in tool argument names into the keys expected by the
/// shared label extractor. Keep original keys too for raw payload consumers.
fn normalize_tool_input(tool_name: &str, input: Value) -> Value {
    let Value::Object(mut map) = input else {
        return input;
    };
    let rewrites: &[(&str, &str)] = match tool_name {
        // Pi's built-in read/write/edit tools use `path`; Claude's label
        // extractor expects `file_path` for file-oriented tools.
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

impl EventAdapter for PiAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: PI_AGENT.into(),
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
                agent: PI_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                prompt: json_str(input, "prompt").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "notification" => Some(AgentEvent::Notification {
                agent: PI_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                wait_reason: json_str(input, "wait_reason").into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "stop" => Some(AgentEvent::Stop {
                agent: PI_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                last_message: json_str(input, "last_message").into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "stop-failure" => Some(AgentEvent::StopFailure {
                agent: PI_AGENT.into(),
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
    fn session_start_sets_pi_agent() {
        let event = PiAdapter
            .parse(
                "session-start",
                &json!({"cwd": "/repo", "session_id": "s1", "source": "startup"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: PI_AGENT.into(),
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
        let event = PiAdapter
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
    fn activity_log_maps_path_to_file_path() {
        let event = PiAdapter
            .parse(
                "activity-log",
                &json!({"tool_name": "read", "tool_input": {"path": "/repo/src/main.rs"}}),
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
    fn session_end_is_supported() {
        assert_eq!(
            PiAdapter.parse("session-end", &json!({"end_reason": "quit"})),
            Some(AgentEvent::SessionEnd {
                end_reason: "quit".into()
            })
        );
    }
}
