//! 가짜 curl. `curl`이라는 이름으로 PATH 앞에 놓여(`Sandbox::install_fakes`) Jev 요청에 각본의 응답을 낸다(설계 9.3절, 12.1절).
//! 네트워크로 나가지 않는다.
//!
//! `FAKE_CURL_DIR` 폴더의 각본 규약은 `fullops_testkit::fake`에 있다. 기록에는 `"stdin"`(`curl --config -`로 받은 글 전부)이
//! 더 남는다. 시험은 이 글로 요청 본문에 든 것을 본다. 각본에 없으면 종료 코드 7(연결 실패)이다.

use fullops_testkit::fake::{Reply, finish, script_dir, scripted};
use serde_json::{Value, json};
use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let dir = match script_dir("FAKE_CURL_DIR") {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut stdin = String::new();
    let _ = std::io::stdin().read_to_string(&mut stdin);
    let reply = scripted(&dir, &argv).unwrap_or_else(|| {
        let mut reply = Reply::plain(Value::Null, 7);
        reply.stderr = "curl: (7) fake-curl: 각본에 없는 요청\n".into();
        reply
    });
    finish(&dir, &argv, reply, json!({ "stdin": stdin }))
}
