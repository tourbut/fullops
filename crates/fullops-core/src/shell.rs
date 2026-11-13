//! 셸 명령 분류(설계 7.4절). 정규식을 쓰지 않는다. 명령 해석은 보안 경계가 아니다.

use crate::model::{Dialect, Intent, Known};
use std::collections::BTreeMap;

/// 토큰 하나. `var`는 작은따옴표 밖에 `$`가 있었다는 뜻이다(리터럴이 아니다).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    text: String,
    var: bool,
}

/// 명령 문자열을 명령(토큰 목록)들로 나눈다. 따옴표나 here-string이 닫히지 않으면 `None`이다.
fn split(command: &str, dialect: Dialect) -> Option<Vec<Vec<Token>>> {
    let escape = match dialect {
        Dialect::Bash => '\\',
        Dialect::PowerShell => '`',
    };
    let chars: Vec<char> = command.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let mut commands: Vec<Vec<Token>> = vec![Vec::new()];
    let mut current: Option<Token> = None;
    // 이 줄이 끝난 뒤 건너뛸 here-doc의 끝 표식(`<<-`면 앞 탭을 봐준다)
    let mut heredocs: Vec<(String, bool)> = Vec::new();
    let mut i = 0;

    macro_rules! flush {
        () => {
            if let Some(token) = current.take()
                && let Some(last) = commands.last_mut()
            {
                last.push(token);
            }
        };
    }
    macro_rules! next_command {
        () => {
            flush!();
            if commands.last().is_some_and(|c| !c.is_empty()) {
                commands.push(Vec::new());
            }
        };
    }
    macro_rules! push {
        ($c:expr) => {
            current
                .get_or_insert(Token {
                    text: String::new(),
                    var: false,
                })
                .text
                .push($c)
        };
    }

    while let Some(c) = at(i) {
        match c {
            '\n' => {
                next_command!();
                i += 1;
                // here-doc 본문은 한 덩어리의 데이터다: 끝 표식 줄까지 건너뛴다
                for (word, tabs) in heredocs.drain(..) {
                    loop {
                        let end = (i..chars.len())
                            .find(|j| chars[*j] == '\n')
                            .unwrap_or(chars.len());
                        let line: String = chars[i..end].iter().collect();
                        let line = line.trim_end_matches('\r');
                        let line = if tabs {
                            line.trim_start_matches('\t')
                        } else {
                            line
                        };
                        let last = end >= chars.len();
                        i = (end + 1).min(chars.len());
                        if line == word || last {
                            break;
                        }
                    }
                }
                continue;
            }
            ' ' | '\t' | '\r' => flush!(),
            ';' => {
                next_command!();
            }
            '|' => {
                next_command!();
                if at(i + 1) == Some('|') {
                    i += 1;
                }
            }
            '&' => {
                if at(i + 1) == Some('&') {
                    next_command!();
                    i += 1;
                } else if at(i + 1) == Some('>') || (i > 0 && at(i - 1) == Some('>')) {
                    // fd 복제(`2>&1`, `>&2`, `&>`)의 `&`는 구분자가 아니다
                    push!(c);
                } else if current.is_none() && commands.last().is_some_and(|c| c.is_empty()) {
                    // PowerShell의 호출 연산자
                } else {
                    next_command!();
                }
            }
            '#' if current.is_none() => {
                while at(i).is_some_and(|c| c != '\n') {
                    i += 1;
                }
                continue;
            }
            '\'' => {
                let token = current.get_or_insert(Token {
                    text: String::new(),
                    var: false,
                });
                i += 1;
                loop {
                    match at(i)? {
                        '\'' if dialect == Dialect::PowerShell && at(i + 1) == Some('\'') => {
                            token.text.push('\'');
                            i += 1;
                        }
                        '\'' => break,
                        other => token.text.push(other),
                    }
                    i += 1;
                }
            }
            '"' => {
                let token = current.get_or_insert(Token {
                    text: String::new(),
                    var: false,
                });
                i += 1;
                loop {
                    match at(i)? {
                        '"' if dialect == Dialect::PowerShell && at(i + 1) == Some('"') => {
                            token.text.push('"');
                            i += 1;
                        }
                        '"' => break,
                        e if e == escape => {
                            i += 1;
                            token.text.push(at(i)?);
                        }
                        other => {
                            token.var |= other == '$';
                            token.text.push(other);
                        }
                    }
                    i += 1;
                }
            }
            '@' if dialect == Dialect::PowerShell
                && current.is_none()
                && matches!(at(i + 1), Some('\'' | '"')) =>
            {
                // here-string: `@'` 줄바꿈 … 줄바꿈 `'@`
                let quote = at(i + 1)?;
                let mut j = i + 2;
                while matches!(at(j), Some(' ' | '\t' | '\r')) {
                    j += 1;
                }
                if at(j) != Some('\n') {
                    push!(c);
                    i += 1;
                    continue;
                }
                let mut text = String::new();
                j += 1;
                loop {
                    if at(j)? == quote && at(j + 1) == Some('@') {
                        break;
                    }
                    let end = (j..chars.len()).find(|k| chars[*k] == '\n')?;
                    text.extend(&chars[j..=end]);
                    j = end + 1;
                }
                if let Some(last) = commands.last_mut() {
                    last.push(Token { text, var: false });
                }
                i = j + 2;
                continue;
            }
            '<' if dialect == Dialect::Bash && at(i + 1) == Some('<') && at(i + 2) != Some('<') => {
                flush!();
                let mut j = i + 2;
                let tabs = at(j) == Some('-');
                if tabs {
                    j += 1;
                }
                while matches!(at(j), Some(' ' | '\t')) {
                    j += 1;
                }
                let mut word = String::new();
                while let Some(w) = at(j) {
                    if w.is_whitespace() || matches!(w, ';' | '|' | '&' | '<' | '>') {
                        break;
                    }
                    if !matches!(w, '\'' | '"' | '\\') {
                        word.push(w);
                    }
                    j += 1;
                }
                if !word.is_empty() {
                    heredocs.push((word, tabs));
                }
                i = j;
                continue;
            }
            e if e == escape => {
                i += 1;
                match at(i) {
                    Some('\n') | None => {}
                    Some(other) => push!(other),
                }
            }
            other => {
                let token = current.get_or_insert(Token {
                    text: String::new(),
                    var: false,
                });
                token.var |= other == '$';
                token.text.push(other);
            }
        }
        i += 1;
    }
    flush!();
    commands.retain(|c| !c.is_empty());
    Some(commands)
}

/// 명령 이름: 마지막 경로 부분에서 `.exe`·`.cmd`·`.bat`를 떼고 소문자로 바꾼 것.
pub fn command_name(token: &str) -> String {
    let base = token.rsplit(['/', '\\']).next().unwrap_or(token);
    let lower = base.to_lowercase();
    [".exe", ".cmd", ".bat"]
        .iter()
        .find_map(|ext| lower.strip_suffix(ext))
        .unwrap_or(&lower)
        .to_string()
}

fn var_name(text: &str) -> Option<&str> {
    let name = text.strip_prefix('$')?;
    let name = name
        .strip_prefix('{')
        .and_then(|n| n.strip_suffix('}'))
        .unwrap_or(name);
    let ok = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_');
    ok.then_some(name)
}

fn is_name(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
        && text.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// 같은 명령 문자열 안의 리터럴 대입. 두 번 대입된 이름은 `None`이다(단일 대입만 푼다).
fn assignments(commands: &[Vec<Token>]) -> BTreeMap<String, Option<String>> {
    let mut found: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut put = |name: &str, value: &Token| {
        let value = (!value.var).then(|| value.text.clone());
        found
            .entry(name.to_string())
            .and_modify(|v| *v = None)
            .or_insert(value);
    };
    for tokens in commands {
        // PowerShell: `$이름 = '값'`
        if let [name, eq, value] = tokens.as_slice()
            && eq.text == "="
            && let Some(name) = var_name(&name.text)
        {
            put(name, value);
            continue;
        }
        for token in tokens {
            let Some((name, value)) = token.text.split_once('=') else {
                break;
            };
            let name = name.strip_prefix('$').unwrap_or(name);
            if !is_name(name) {
                break;
            }
            let value = Token {
                text: value.to_string(),
                var: value.contains('$') && token.var,
            };
            put(name, &value);
        }
    }
    found
}

fn resolve(token: &Token, vars: &BTreeMap<String, Option<String>>) -> Option<String> {
    if !token.var {
        return Some(token.text.clone());
    }
    vars.get(var_name(&token.text)?)?.clone()
}

/// `--이름 값` 또는 `--이름=값`의 값 토큰.
fn option(args: &[Token], name: &str) -> Option<Token> {
    let prefix = format!("{name}=");
    args.iter().enumerate().find_map(|(i, arg)| {
        if arg.text == name {
            args.get(i + 1).cloned()
        } else {
            arg.text.strip_prefix(&prefix).map(|value| Token {
                text: value.to_string(),
                var: arg.var,
            })
        }
    })
}

fn orca(args: &[Token], vars: &BTreeMap<String, Option<String>>) -> Vec<Intent> {
    let has = |flag: &str| args.iter().any(|a| a.text == flag);
    if has("--help") || has("-h") {
        return vec![Intent::Other];
    }
    let word = |i: usize| args.get(i).map_or("", |t| t.text.as_str());
    match (word(0), word(1)) {
        ("orchestration", "worker-start") => vec![Intent::OrcaWorkerStart],
        ("orchestration", "task-create") => {
            let spec = match option(args, "--spec") {
                Some(token) => resolve(&token, vars).map_or(Known::Unknown("spec_var"), Known::Yes),
                None => Known::Unknown("spec_file"),
            };
            vec![Intent::OrcaTaskCreate { spec }]
        }
        ("orchestration", "dispatch") if has("--inject") => vec![Intent::OrcaInject],
        ("orchestration", "check") => vec![Intent::OrcaCheck {
            peek: has("--peek"),
            run: option(args, "--run").map(|t| t.text),
        }],
        ("terminal", "send") => {
            let text = option(args, "--text").map(|t| t.text).unwrap_or_default();
            let mut out = vec![Intent::OrcaTerminalSend {
                chars: text.chars().count(),
            }];
            if text.contains("handovers/") || text.contains("handovers\\") {
                out.push(Intent::OrcaInject);
            }
            out
        }
        _ => vec![Intent::Other],
    }
}

fn git(args: &[Token]) -> Intent {
    let mut args = args.iter();
    let sub = loop {
        match args.next() {
            // `-C <경로>`와 `-c <설정>`은 값을 하나 먹는다
            Some(t) if t.text == "-C" || t.text == "-c" => {
                args.next();
            }
            Some(t) if t.text.starts_with('-') => {}
            Some(t) => break t.text.as_str(),
            None => return Intent::Other,
        }
    };
    match sub {
        "commit" => Intent::GitCommit {
            all: args.any(|t| {
                let short = t.text.strip_prefix('-').filter(|s| !s.starts_with('-'));
                t.text == "--all"
                    || short.is_some_and(|s| {
                        s.contains('a') && s.chars().all(|c| c.is_ascii_alphabetic())
                    })
            }),
        },
        "add" => Intent::GitAdd,
        _ => Intent::Other,
    }
}

fn classify(command: &str, dialect: Dialect, orca_names: &[String], nested: bool) -> Vec<Intent> {
    let Some(commands) = split(command, dialect) else {
        return vec![Intent::Unparsed];
    };
    let vars = assignments(&commands);
    let mut out = Vec::new();
    for tokens in &commands {
        // 명령 앞의 `NAME=value` 대입을 뗀다
        let skip = tokens
            .iter()
            .take_while(|t| t.text.split_once('=').is_some_and(|(n, _)| is_name(n)))
            .count();
        let Some(first) = tokens.get(skip) else {
            continue;
        };
        let args = tokens.get(skip + 1..).unwrap_or_default();
        if args.first().is_some_and(|t| t.text == "=") {
            continue;
        }
        let Some(first) = resolve(first, &vars) else {
            out.push(Intent::Unparsed);
            continue;
        };
        let name = command_name(&first);
        let inner = |flags: &[&str]| {
            args.iter()
                .position(|t| flags.iter().any(|f| t.text.eq_ignore_ascii_case(f)))
                .and_then(|i| args.get(i + 1))
        };
        let shell = match name.as_str() {
            "bash" | "sh" | "zsh" => inner(&["-c", "-lc", "-ic"]).map(|t| (t, Dialect::Bash)),
            "pwsh" | "powershell" => inner(&["-c", "-command"]).map(|t| (t, Dialect::PowerShell)),
            // cmd는 `\`가 경로 구분자라 PowerShell 방언으로 나눈다
            "cmd" => inner(&["/c"]).map(|t| (t, Dialect::PowerShell)),
            _ => None,
        };
        match (shell, name.as_str()) {
            (Some((script, inner_dialect)), _) if !nested => {
                out.extend(classify(&script.text, inner_dialect, orca_names, true));
            }
            (_, "orca" | "orca-dev" | "orca-ide") => out.extend(orca(args, &vars)),
            (_, n) if orca_names.iter().any(|o| command_name(o) == n) => {
                out.extend(orca(args, &vars))
            }
            (_, "git") => out.push(git(args)),
            (_, "fullops") => out.push(Intent::Fullops {
                sub: args.first().map(|t| t.text.clone()).unwrap_or_default(),
            }),
            _ => out.push(Intent::Other),
        }
    }
    out
}

/// 명령 문자열을 Intent 목록으로 나눈다. `orca_names`는 Orca 실행 파일로 볼 이름(`ORCA_CLI_COMMAND`의 파일 이름)이다.
pub fn classify_shell(command: &str, dialect: Dialect, orca_names: &[String]) -> Vec<Intent> {
    classify(command, dialect, orca_names, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Dialect::{Bash, PowerShell};

    fn of(command: &str, dialect: Dialect) -> Vec<Intent> {
        classify_shell(command, dialect, &["C:/tools/fake-orca.exe".to_string()])
    }

    #[test]
    fn classifies_the_case_table() {
        let send = |chars| Intent::OrcaTerminalSend { chars };
        let spec = |s: &str| Intent::OrcaTaskCreate {
            spec: Known::Yes(s.to_string()),
        };
        let check = |peek, run: Option<&str>| Intent::OrcaCheck {
            peek,
            run: run.map(str::to_string),
        };
        let fullops = |sub: &str| Intent::Fullops { sub: sub.into() };
        let cases: Vec<(&str, Dialect, Vec<Intent>)> = vec![
            ("ls -la", Bash, vec![Intent::Other]),
            (
                "orca orchestration worker-start --spec x",
                Bash,
                vec![Intent::OrcaWorkerStart],
            ),
            (
                "cd a && orca-dev orchestration worker-start --run r 2>&1 | tee log",
                Bash,
                vec![Intent::Other, Intent::OrcaWorkerStart, Intent::Other],
            ),
            (
                "FOO=1 orca orchestration worker-start",
                Bash,
                vec![Intent::OrcaWorkerStart],
            ),
            (
                "orca orchestration worker-start --help",
                Bash,
                vec![Intent::Other],
            ),
            (
                "fake-orca orchestration worker-start",
                Bash,
                vec![Intent::OrcaWorkerStart],
            ),
            (
                r#"& "C:\Program Files\Orca\orca.exe" orchestration worker-start --spec x"#,
                PowerShell,
                vec![Intent::OrcaWorkerStart],
            ),
            (
                r#"$o = 'C:\bin\orca.exe'; & $o orchestration worker-start"#,
                PowerShell,
                vec![Intent::OrcaWorkerStart],
            ),
            (
                "O=orca; $O orchestration worker-start",
                Bash,
                vec![Intent::OrcaWorkerStart],
            ),
            (
                "$UNKNOWN orchestration worker-start",
                Bash,
                vec![Intent::Unparsed],
            ),
            (
                r#"bash -lc "orca orchestration worker-start --spec x""#,
                Bash,
                vec![Intent::OrcaWorkerStart],
            ),
            (
                r#"pwsh -Command "orca orchestration dispatch --inject""#,
                Bash,
                vec![Intent::OrcaInject],
            ),
            (
                r#"cmd /c "orca orchestration check --run r1""#,
                PowerShell,
                vec![check(false, Some("r1"))],
            ),
            (
                "orca orchestration dispatch --task t",
                Bash,
                vec![Intent::Other],
            ),
            (
                "orca orchestration check --peek",
                Bash,
                vec![check(true, None)],
            ),
            (
                r#"orca orchestration task-create --spec "Task key: A-1""#,
                Bash,
                vec![spec("Task key: A-1")],
            ),
            (
                r#"S='Task key: A-1'; orca orchestration task-create --spec "$S""#,
                Bash,
                vec![spec("Task key: A-1")],
            ),
            (
                "orca orchestration task-create --spec-file s.txt",
                Bash,
                vec![Intent::OrcaTaskCreate {
                    spec: Known::Unknown("spec_file"),
                }],
            ),
            (
                r#"orca orchestration task-create --spec "$S""#,
                Bash,
                vec![Intent::OrcaTaskCreate {
                    spec: Known::Unknown("spec_var"),
                }],
            ),
            (
                "orca terminal send --terminal t --text '12345678901234567890'",
                Bash,
                vec![send(20)],
            ),
            (
                "orca terminal send --text=가나다 --enter",
                Bash,
                vec![send(3)],
            ),
            ("orca terminal send --enter", Bash, vec![send(0)]),
            (
                r#"& orca.exe terminal send --text "handovers/to_dev.md 를 읽어 주세요" --enter"#,
                PowerShell,
                vec![send(28), Intent::OrcaInject],
            ),
            (
                "git commit -m msg",
                Bash,
                vec![Intent::GitCommit { all: false }],
            ),
            (
                "git -C sub -c user.name=x commit -am msg",
                Bash,
                vec![Intent::GitCommit { all: true }],
            ),
            (
                "git commit --amend",
                Bash,
                vec![Intent::GitCommit { all: false }],
            ),
            (
                "git add -A; git commit --all",
                Bash,
                vec![Intent::GitAdd, Intent::GitCommit { all: true }],
            ),
            ("git status", Bash, vec![Intent::Other]),
            (
                "fullops dispatch A-1 --role dev",
                Bash,
                vec![fullops("dispatch")],
            ),
            (
                r#"C:\Users\a\.fullops\bin\fullops.exe doctor"#,
                PowerShell,
                vec![fullops("doctor")],
            ),
            (
                "echo 'orca orchestration worker-start'",
                Bash,
                vec![Intent::Other],
            ),
            (
                "cat <<EOF > a.txt\norca orchestration worker-start\nEOF\nfullops status",
                Bash,
                vec![Intent::Other, fullops("status")],
            ),
            (
                "$t = @'\norca orchestration worker-start\n'@\nfullops why",
                PowerShell,
                vec![fullops("why")],
            ),
            ("echo \"unterminated", Bash, vec![Intent::Unparsed]),
            (
                "# orca orchestration worker-start\nls",
                Bash,
                vec![Intent::Other],
            ),
            (
                "echo a\\ b; orca orchestration worker-start",
                Bash,
                vec![Intent::Other, Intent::OrcaWorkerStart],
            ),
        ];
        for (command, dialect, expected) in cases {
            assert_eq!(of(command, dialect), expected, "{dialect:?}: {command}");
        }
    }
}
