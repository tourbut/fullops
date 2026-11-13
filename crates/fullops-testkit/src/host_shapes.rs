//! 호스트별 hook 입력의 모양, 등록 인자, 출력 읽기(설계 7.2절). 표본은 `scenarios/fixtures/host/<호스트>/`에 있고
//! 구동기는 이 파일로만 호스트 차이를 다룬다. 계약이 바뀌면 표본과 이 파일을 함께 고친다.

use serde_json::{Value, json};
use std::path::Path;

/// 구동기가 아는 호스트. `[setup] host`의 값이다.
pub const HOSTS: [&str; 4] = ["claude", "codex", "grok", "agy"];

/// hook 이벤트 하나. 호스트에 없는 이벤트는 `hook_call`이 `None`을 돌려준다.
pub enum Event<'a> {
    /// `source`는 fullops의 어휘(`startup`·`clear`·`resume`·`compact`·`fork`)로 준다.
    SessionStart {
        source: &'a str,
    },
    UserPromptSubmit {
        prompt: &'a str,
    },
    PreToolUse(Tool<'a>),
    /// `reason`은 호스트의 종료 이유 값이다(grok `end_turn` 등, agy `NO_TOOL_CALL` 등). 없으면 정상 종료다.
    Stop {
        reason: Option<&'a str>,
    },
    /// 턴이 API 오류로 끝났다. `error`는 오류 글이다.
    StopFailure {
        error: &'a str,
    },
}

/// 도구 호출 하나. 경로는 절대 경로다.
pub enum Tool<'a> {
    Shell {
        command: &'a str,
        powershell: bool,
    },
    Write {
        path: &'a Path,
        content: &'a str,
    },
    Edit {
        path: &'a Path,
        old: &'a str,
        new: &'a str,
    },
    SubAgent {
        prompt: &'a str,
    },
    /// Claude Code의 Workflow 도구. 다른 호스트에는 없다.
    Workflow {
        script: &'a str,
    },
    /// 호스트의 도구 이름과 인자를 그대로 준다(Codex의 `apply_patch` 등).
    Raw {
        name: &'a str,
        input: Value,
    },
}

/// `fullops hook` 뒤에 붙일 인자와 stdin에 넣을 JSON.
pub struct HookCall {
    pub args: Vec<String>,
    pub input: Value,
}

fn tool_shape(host: &str, tool: &Tool) -> Option<(String, Value)> {
    let named = |name: &str, input: Value| Some((name.to_string(), input));
    match (host, tool) {
        (_, Tool::Raw { name, input }) => named(name, input.clone()),
        (
            "claude",
            Tool::Shell {
                command,
                powershell,
            },
        ) => named(
            if *powershell { "PowerShell" } else { "Bash" },
            json!({ "command": command }),
        ),
        ("codex", Tool::Shell { command, .. }) => named("Bash", json!({ "command": command })),
        ("grok", Tool::Shell { command, .. }) => {
            named("run_terminal_command", json!({ "command": command }))
        }
        ("agy", Tool::Shell { command, .. }) => {
            named("run_command", json!({ "CommandLine": command }))
        }
        ("claude", Tool::Write { path, content }) => {
            named("Write", json!({ "file_path": path, "content": content }))
        }
        ("grok", Tool::Write { path, content }) => {
            named("write", json!({ "file_path": path, "content": content }))
        }
        ("agy", Tool::Write { path, content }) => named(
            "write_to_file",
            json!({ "TargetFile": path, "CodeContent": content }),
        ),
        ("claude", Tool::Edit { path, old, new }) => named(
            "Edit",
            json!({ "file_path": path, "old_string": old, "new_string": new }),
        ),
        ("grok", Tool::Edit { path, old, new }) => named(
            "search_replace",
            json!({ "file_path": path, "old_string": old, "new_string": new, "replace_all": false }),
        ),
        ("agy", Tool::Edit { path, old, new }) => named(
            "replace_file_content",
            json!({ "TargetFile": path, "TargetContent": old, "ReplacementContent": new }),
        ),
        ("claude", Tool::SubAgent { prompt }) => named("Agent", json!({ "prompt": prompt })),
        ("codex", Tool::SubAgent { prompt }) => named("spawn_agent", json!({ "message": prompt })),
        ("grok", Tool::SubAgent { prompt }) => named("spawn_subagent", json!({ "prompt": prompt })),
        ("agy", Tool::SubAgent { prompt }) => named("invoke_subagent", json!({ "Prompt": prompt })),
        ("claude", Tool::Workflow { script }) => named("Workflow", json!({ "script": script })),
        // Codex의 파일 편집은 apply_patch다(구동기가 Raw로 준다). Workflow는 Claude Code에만 있다
        _ => None,
    }
}

/// 그 호스트가 그 이벤트에 주는 입력과 등록 인자. `last_prompt`는 agy가 모든 이벤트에 싣는 `lastUserInput`이다.
/// 호스트에 그 이벤트(또는 그 도구)가 없으면 `None`이다: Codex의 StopFailure, agy의 UserPromptSubmit, Claude Code 밖의 Workflow.
pub fn hook_call(
    host: &str,
    sid: &str,
    cwd: &Path,
    last_prompt: Option<&str>,
    event: &Event,
) -> Option<HookCall> {
    let name = match event {
        Event::SessionStart { .. } => "SessionStart",
        Event::UserPromptSubmit { .. } => "UserPromptSubmit",
        Event::PreToolUse(_) => "PreToolUse",
        Event::Stop { .. } => "Stop",
        Event::StopFailure { .. } => "StopFailure",
    };
    let mut args = vec![host.to_string()];
    let mut extra = serde_json::Map::new();
    let mut put = |key: &str, value: Value| {
        extra.insert(key.to_string(), value);
    };
    let mut input = match host {
        "claude" | "codex" => json!({ "hook_event_name": name, "session_id": sid, "cwd": cwd }),
        // grok은 camelCase와 Claude Code의 snake_case 별칭을 함께 준다
        "grok" => {
            let snake = name.chars().fold(String::new(), |mut out, c| {
                if c.is_ascii_uppercase() && !out.is_empty() {
                    out.push('_');
                }
                out.push(c.to_ascii_lowercase());
                out
            });
            json!({ "hookEventName": snake, "hook_event_name": name, "sessionId": sid, "session_id": sid, "cwd": cwd })
        }
        // agy는 입력에 이벤트 이름이 없어 등록 문자열이 --event를 준다
        "agy" => {
            let event_arg = if name == "StopFailure" { "Stop" } else { name };
            args.extend(["--event".to_string(), event_arg.to_string()]);
            let mut input = json!({ "conversationId": sid, "workspacePaths": [cwd] });
            if let (Some(prompt), false) = (last_prompt, name == "SessionStart") {
                input["lastUserInput"] = json!(prompt);
            }
            input
        }
        _ => return None,
    };
    match (host, event) {
        ("agy", Event::SessionStart { .. }) => {}
        ("grok", Event::SessionStart { source }) => put(
            "source",
            json!(match *source {
                "startup" => "new",
                "resume" => "load",
                other => other,
            }),
        ),
        (_, Event::SessionStart { source }) => put("source", json!(source)),
        ("agy", Event::UserPromptSubmit { .. }) => return None,
        (_, Event::UserPromptSubmit { prompt }) => put("prompt", json!(prompt)),
        (_, Event::PreToolUse(tool)) => {
            let (tool_name, tool_input) = tool_shape(host, tool)?;
            match host {
                "agy" => put("toolCall", json!({ "name": tool_name, "args": tool_input })),
                "grok" => {
                    put("toolName", json!(tool_name));
                    put("tool_name", json!(tool_name));
                    put("toolInput", tool_input.clone());
                    put("tool_input", tool_input);
                    put("toolInputTruncated", json!(false));
                }
                _ => {
                    put("tool_name", json!(tool_name));
                    put("tool_input", tool_input);
                }
            }
        }
        ("agy", Event::Stop { reason }) => {
            put("terminationReason", json!(reason.unwrap_or("NO_TOOL_CALL")));
            put("error", json!(""));
        }
        ("grok", Event::Stop { reason }) => put("reason", json!(reason.unwrap_or("end_turn"))),
        // Claude Code와 Codex의 Stop 입력에는 종료 이유가 없다
        (_, Event::Stop { .. }) => {}
        ("codex", Event::StopFailure { .. }) => return None,
        ("agy", Event::StopFailure { error }) => {
            put("terminationReason", json!("ERROR"));
            put("error", json!(error));
        }
        (_, Event::StopFailure { error }) => {
            put("error", json!("rate_limit"));
            put("error_details", json!(error));
        }
    }
    input.as_object_mut()?.extend(extra);
    Some(HookCall { args, input })
}

/// hook의 stdout을 시나리오의 `verdict` 어휘로 옮긴다: `pass`·`context`·`warn`·`deny`·`block`. JSON 객체가 아니면 `error`다.
pub fn verdict_of(host: &str, stdout: &str) -> &'static str {
    let out: Value = serde_json::from_str(stdout).unwrap_or(Value::Null);
    if !out.is_object() {
        return "error";
    }
    if host == "agy" {
        let step = &out["injectSteps"][0];
        return match out["decision"].as_str() {
            Some("deny") => "deny",
            Some("continue") => "block",
            _ if step["errorMessage"].is_string() => "warn",
            _ if step["systemMessage"].is_object() => "context",
            _ => "pass",
        };
    }
    if out["hookSpecificOutput"]["permissionDecision"] == "deny" {
        "deny"
    } else if out["decision"] == "block" {
        "block"
    } else if out["systemMessage"].is_string() {
        "warn"
    } else if out["hookSpecificOutput"]["additionalContext"].is_string() {
        "context"
    } else {
        "pass"
    }
}

/// 그 호스트가 그 이벤트에서 "통과"로 읽는 글(설계 7.1절의 `Host::pass`). `event`는 fullops의 이벤트 이름이다.
pub fn pass_output(host: &str, event: &str) -> &'static str {
    match (host, event) {
        ("agy", "PreToolUse") => r#"{"decision":"ask"}"#,
        ("agy", "Stop" | "StopFailure") => r#"{"decision":""}"#,
        _ => "{}",
    }
}
