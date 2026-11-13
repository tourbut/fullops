//! 가짜 호스트 CLI와 스킬 설치 도구. `claude`·`codex`·`grok`·`agy`·`npx`라는 이름으로 PATH 앞에 놓인다(`Sandbox::install_fakes`).
//! 실제로는 아무것도 설치하지 않는다. 설계 9.5절의 목록 조회에는 각본의 JSON을 답하고 받은 호출을 남긴다.
//!
//! `FAKE_HOST_DIR` 폴더의 각본 규약은 `fullops_testkit::fake`에 있다. `when`은 `[<실행 파일 이름>, <인자>…]`와 맞춘다
//! (예: `["claude","plugin","marketplace","list"]`). 기록에는 `"program"`이 더 남는다.
//! 각본에 없으면 `--version`에는 `<이름> 0.0.0-fake`를, 그 밖에는 빈 출력을 내고 종료 코드 0이다.

use fullops_testkit::fake::{Reply, finish, script_dir, scripted};
use serde_json::{Value, json};
use std::process::ExitCode;

fn main() -> ExitCode {
    let dir = match script_dir("FAKE_HOST_DIR") {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    let program = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let named: Vec<String> = std::iter::once(program.clone())
        .chain(argv.iter().cloned())
        .collect();
    let reply = scripted(&dir, &named).unwrap_or_else(|| {
        let version = argv.first().is_some_and(|a| a == "--version");
        let stdout = if version {
            json!(format!("{program} 0.0.0-fake\n"))
        } else {
            Value::Null
        };
        Reply::plain(stdout, 0)
    });
    finish(&dir, &argv, reply, json!({ "program": program }))
}
