//! 셸 명령 분류(설계 7.4절). 정규식을 쓰지 않는다. 명령 해석은 보안 경계가 아니다.

use crate::model::{Dialect, Intent, Known};
use std::collections::BTreeMap;

/// 토큰 하나. `var`는 작은따옴표 밖에 `$`가 있었다는 뜻이다(리터럴이 아니다).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    text: String,
    var: bool,
}

/// 큰따옴표 안 `$(`의 짝 `)` 자리. here-doc 본문의 괄호는 세지 않는다(`git commit -m "$(cat <<'EOF' … EOF)"`의 본문은 글이다).
fn closing(chars: &[char], from: usize, dialect: Dialect) -> Option<usize> {
    let line_end = |from: usize| (from..chars.len()).find(|k| chars.get(*k) == Some(&'\n'));
    let (mut depth, mut j) = (1, from);
    loop {
        match *chars.get(j)? {
            '(' => depth += 1,
            ')' if depth == 1 => return Some(j),
            ')' => depth -= 1,
            '<' if dialect == Dialect::Bash && chars.get(j + 1) == Some(&'<') => {
                // 줄바꿈이 없거나(`$((1<<2))`) `<<<`면 here-doc이 아니다
                let here_string = chars.get(j + 2) == Some(&'<');
                if let Some(first) = line_end(j).filter(|_| !here_string) {
                    let head: String = chars.get(j + 2..first)?.iter().collect();
                    let word: String = head
                        .trim_start_matches(['-', ' ', '\t'])
                        .chars()
                        .take_while(|c| !c.is_whitespace())
                        .filter(|c| !matches!(c, '\'' | '"' | '\\'))
                        .collect();
                    let mut start = first + 1;
                    j = loop {
                        let end = line_end(start).unwrap_or(chars.len());
                        let line: String = chars.get(start..end)?.iter().collect();
                        if line.trim_end_matches('\r').trim_start_matches('\t') == word {
                            break end;
                        }
                        if end >= chars.len() {
                            return None;
                        }
                        start = end + 1;
                    };
                    continue;
                }
                // 남은 `<`를 다시 보지 않는다
                j += 1 + usize::from(here_string);
            }
            _ => {}
        }
        j += 1;
    }
}

/// 명령 문자열을 명령(토큰 목록)들로 나눈다. 따옴표나 here-string이 닫히지 않으면 `None`이다.
/// 둘째 값은 큰따옴표 안의 명령 치환(`$(…)`, Bash의 백틱) 본문이다: 따옴표 안에서도 실행된다.
/// 괄호, 명령을 여닫는 중괄호, 따옴표 밖의 백틱은 명령 구분자로 본다(서브셸·묶음·치환의 안쪽이 명령으로 나뉜다).
fn split(command: &str, dialect: Dialect) -> Option<(Vec<Vec<Token>>, Vec<String>)> {
    let escape = match dialect {
        Dialect::Bash => '\\',
        Dialect::PowerShell => '`',
    };
    let chars: Vec<char> = command.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let mut commands: Vec<Vec<Token>> = vec![Vec::new()];
    let mut current: Option<Token> = None;
    let mut subs: Vec<String> = Vec::new();
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
            '(' | ')' => {
                // `$(`·`@(`의 머리 글자는 명령의 일부가 아니다
                if let Some(token) = &mut current
                    && token.text.ends_with(['$', '@'])
                {
                    token.text.pop();
                }
                current = current.filter(|t| !t.text.is_empty());
                next_command!();
            }
            '{' if current.is_none() => {
                next_command!();
            }
            // `${이름}`과 `{a,b}`의 닫는 중괄호는 토큰의 일부다
            '}' if current.as_ref().is_none_or(|t| !t.text.contains('{')) => {
                next_command!();
            }
            '`' if dialect == Dialect::Bash => {
                next_command!();
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
                        '$' if at(i + 1) == Some('(') => {
                            let end = closing(&chars, i + 2, dialect)?;
                            subs.push(chars.get(i + 2..end)?.iter().collect());
                            token.var = true;
                            token.text.extend(chars.get(i..=end)?);
                            i = end;
                        }
                        '`' if dialect == Dialect::Bash => {
                            let end = (i + 1..chars.len()).find(|j| at(*j) == Some('`'))?;
                            subs.push(chars.get(i + 1..end)?.iter().collect());
                            token.var = true;
                            token.text.extend(chars.get(i..=end)?);
                            i = end;
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
    Some((commands, subs))
}

/// 명령 이름: 마지막 경로 부분에서 `.exe`·`.cmd`·`.bat`·`.ps1`을 떼고 소문자로 바꾼 것.
pub fn command_name(token: &str) -> String {
    let base = token.rsplit(['/', '\\']).next().unwrap_or(token);
    let lower = base.to_lowercase();
    [".exe", ".cmd", ".bat", ".ps1"]
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
    // Orca는 전역 플래그(`--json` 등)를 명령 경로 앞·가운데에도 받는다: 플래그를 건너뛰고 무리와 하위 명령을 찾는다
    let words: Vec<&str> = args
        .iter()
        .map(|t| t.text.as_str())
        .filter(|w| !w.starts_with('-'))
        .collect();
    let path = words
        .iter()
        .position(|w| *w == "orchestration" || *w == "terminal")
        .map_or(("", ""), |i| {
            let subs: &[&str] = if words[i] == "terminal" {
                &["send"]
            } else {
                &["worker-start", "task-create", "dispatch", "check"]
            };
            let sub = words[i + 1..].iter().find(|w| subs.contains(w));
            (words[i], sub.copied().unwrap_or(""))
        });
    match path {
        ("orchestration", "worker-start") => vec![Intent::OrcaWorkerStart],
        ("orchestration", "task-create") => {
            let spec = match option(args, "--spec") {
                Some(token) => resolve(&token, vars).map_or(Known::Unknown("spec_var"), Known::Yes),
                None => Known::Unknown("spec_file"),
            };
            vec![Intent::OrcaTaskCreate { spec }]
        }
        ("orchestration", "dispatch") if has("--inject") => vec![Intent::OrcaInject],
        // 날것 배정은 worker-start와 같다(DF-1): 배정은 `fullops dispatch`로만 한다. 미리보기만 통과
        ("orchestration", "dispatch") if !has("--dry-run") => vec![Intent::OrcaWorkerStart],
        ("orchestration", "check") => vec![Intent::OrcaCheck {
            peek: has("--peek"),
            run: option(args, "--run").map(|t| t.text),
        }],
        ("terminal", "send") => {
            let text = option(args, "--text").map(|t| t.text).unwrap_or_default();
            let mut out = vec![Intent::OrcaTerminalSend {
                chars: text.chars().count(),
            }];
            // 인박스 경로를 터미널에 보내는 것은 지시서 붙여 넣기와 같다
            if text.contains("inbox/to_") || text.contains("inbox\\to_") {
                out.push(Intent::OrcaInject);
            }
            out
        }
        _ => vec![Intent::Other],
    }
}

/// 작업 위치와 무관하게 레포 전체를 뜻하는 pathspec(`:/`). `.`은 hook의 cwd로 풀려 하위 폴더가 된다.
const WHOLE_REPO: &str = ":/";

fn git(args: &[Token]) -> Intent {
    let mut args = args.iter();
    let mut moved = false;
    let sub = loop {
        match args.next() {
            // `-C <경로>`와 `-c <설정>`은 값을 하나 먹는다
            Some(t) if t.text == "-C" || t.text == "-c" => {
                moved |= t.text == "-C";
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
                // 값을 받는 옵션(`-m<글>` 등) 뒤는 그 값이다: 그 앞의 `a`만 센다
                let before_value = |s: &str| {
                    s.chars()
                        .take_while(|c| !"mFCctuS".contains(*c))
                        .any(|c| c == 'a')
                };
                t.text == "--all" || short.is_some_and(before_value)
            }),
        },
        // `stage`는 `add`의 별칭이다
        "add" | "stage" => {
            let (mut paths, mut update, mut options) = (Vec::new(), false, true);
            for t in args {
                let text = t.text.as_str();
                let short = text.strip_prefix('-').filter(|s| !s.starts_with('-'));
                match text {
                    "--" if options => options = false,
                    _ if options && (text == "--all" || short.is_some_and(|s| s.contains('A'))) => {
                        paths.push(WHOLE_REPO.to_string())
                    }
                    _ if options
                        && (text == "--update" || short.is_some_and(|s| s.contains('u'))) =>
                    {
                        update = true
                    }
                    // 작업 위치를 따라가지 않고 레포 전체로 본다(넓게 보는 쪽이 안전하다)
                    _ if options && text.starts_with("--pathspec-from-file") => {
                        paths.push(WHOLE_REPO.to_string())
                    }
                    _ if options && text.starts_with('-') => {}
                    // 풀리지 않은 확장(변수·명령 치환)은 어디를 가리킬지 모른다
                    _ if t.var || text.contains(['$', '`', '%']) => {
                        paths.push(WHOLE_REPO.to_string())
                    }
                    _ => paths.push(text.to_string()),
                }
            }
            // `-C`로 옮겼거나 경로가 따로 오면(`$(…)`·xargs) 어디인지 모른다
            if (moved && !paths.is_empty()) || (paths.is_empty() && !update) {
                paths = vec![WHOLE_REPO.to_string()];
            }
            Intent::GitAdd { paths, update }
        }
        _ => Intent::Other,
    }
}

/// 명령 앞에 붙는 제어 키워드. 뒤따르는 것이 실제 명령이다.
const KEYWORDS: [&str; 8] = ["then", "do", "else", "elif", "!", "if", "while", "until"];
/// 앞붙이 명령과, 값을 따로 받는 그 짧은 옵션 글자. 옵션을 건너뛴 다음 토큰이 실제 명령이다.
const PREFIXES: [(&str, &str); 8] = [
    ("env", "uCS"),
    ("command", ""),
    ("exec", "a"),
    ("nohup", ""),
    ("time", "fo"),
    ("sudo", "ughpCDRTU"),
    ("xargs", "nILPsdEa"),
    ("timeout", "ks"),
];
/// 셸 안의 셸과 명령 치환을 따라 들어가는 깊이.
const DEPTH: u8 = 3;

fn is_assignment(token: &Token) -> bool {
    token.text.split_once('=').is_some_and(|(n, _)| is_name(n))
}

/// 앞붙이 명령의 옵션과 `NAME=value`를 건너뛴 나머지.
fn after_options<'a>(mut args: &'a [Token], valued: &str) -> &'a [Token] {
    while let Some((first, rest)) = args.split_first() {
        if is_assignment(first) {
            args = rest;
        } else if first.text == "--" {
            return rest;
        } else if let Some(flag) = first.text.strip_prefix('-').filter(|f| !f.is_empty()) {
            let takes_value = flag.len() == 1 && valued.contains(flag);
            args = rest.get(usize::from(takes_value)..).unwrap_or_default();
        } else {
            break;
        }
    }
    args
}

/// 셸에 넘기는 스크립트: `flag`에 맞는 첫 토큰의 뒤. `whole`이면 남은 토큰 전부를 공백으로 잇고 아니면 하나만 본다.
fn script(args: &[Token], flag: impl Fn(&str) -> bool, whole: bool) -> Option<String> {
    let rest = args.get(args.iter().position(|t| flag(&t.text))? + 1..)?;
    let take = if whole { rest.len() } else { 1 };
    let words: Vec<&str> = rest.iter().take(take).map(|t| t.text.as_str()).collect();
    (!words.is_empty()).then(|| words.join(" "))
}

#[derive(Clone, Copy)]
struct Scope<'a> {
    dialect: Dialect,
    orca_names: &'a [String],
    vars: &'a BTreeMap<String, Option<String>>,
    depth: u8,
}

/// 명령 하나를 Intent로 옮긴다. 완벽한 셸 해석이 아니다: 놓치는 꼴은 통과한다.
fn command(tokens: &[Token], scope: Scope, out: &mut Vec<Intent>) {
    let mut rest = tokens;
    // 앞의 `NAME=value` 대입, 제어 키워드, 앞붙이 명령을 떼고 실제 명령을 찾는다
    let (name, args) = loop {
        let Some((first, args)) = rest.split_first() else {
            return;
        };
        if is_assignment(first) {
            rest = args;
            continue;
        }
        if args.first().is_some_and(|t| t.text == "=") {
            return;
        }
        let Some(first) = resolve(first, scope.vars) else {
            out.push(Intent::Unparsed);
            return;
        };
        let name = command_name(&first);
        if KEYWORDS.contains(&name.as_str()) {
            rest = args;
        } else if let Some((_, valued)) = PREFIXES.iter().find(|(p, _)| *p == name) {
            rest = after_options(args, valued);
            // `timeout <시간> 명령`의 시간
            if name == "timeout" {
                rest = rest.get(1..).unwrap_or_default();
            }
        } else {
            break (name, args);
        }
    };
    let any = |flags: &'static [&'static str]| {
        move |text: &str| flags.iter().any(|f| text.eq_ignore_ascii_case(f))
    };
    // `-c`, `-lc`, `-ec`처럼 `c`로 끝나는 짧은 옵션 묶음
    let dash_c = |text: &str| {
        text.strip_prefix('-')
            .is_some_and(|f| f.ends_with('c') && f.chars().all(|c| c.is_ascii_lowercase()))
    };
    let inner = match name.as_str() {
        "bash" | "sh" | "zsh" | "dash" | "ksh" => {
            script(args, dash_c, false).map(|s| (s, Dialect::Bash))
        }
        "pwsh" | "powershell" => {
            script(args, any(&["-c", "-command"]), true).map(|s| (s, Dialect::PowerShell))
        }
        // cmd는 `\`가 경로 구분자라 PowerShell 방언으로 나눈다
        "cmd" => script(args, any(&["/c", "/k"]), true).map(|s| (s, Dialect::PowerShell)),
        "eval" => {
            let words: Vec<&str> = args.iter().map(|t| t.text.as_str()).collect();
            Some((words.join(" "), scope.dialect))
        }
        "invoke-expression" | "iex" => args
            .iter()
            .find(|t| !t.text.starts_with('-'))
            .map(|t| (t.text.clone(), Dialect::PowerShell)),
        _ => None,
    };
    if let Some((text, dialect)) = inner {
        if scope.depth < DEPTH {
            out.extend(classify(&text, dialect, scope.orca_names, scope.depth + 1));
        } else {
            out.push(Intent::Other);
        }
        return;
    }
    match name.as_str() {
        // 매개변수 이름은 버리고 파일과 인자 목록(쉼표나 공백으로 나뉜다)을 한 명령으로 본다
        "start-process" | "saps" | "start" if scope.depth < DEPTH => {
            let words: Vec<Token> = args
                .iter()
                .filter(|t| !t.text.starts_with('-'))
                .flat_map(|t| {
                    let word = |w: &str| Token {
                        text: w.to_string(),
                        var: t.var,
                    };
                    t.text.split([',', ' ']).filter(|w| !w.is_empty()).map(word)
                })
                .collect();
            if words.is_empty() {
                out.push(Intent::Other);
            } else {
                let depth = scope.depth + 1;
                command(&words, Scope { depth, ..scope }, out);
            }
        }
        "orca" | "orca-dev" | "orca-ide" => out.extend(orca(args, scope.vars)),
        n if scope.orca_names.iter().any(|o| command_name(o) == n) => {
            out.extend(orca(args, scope.vars))
        }
        "git" => out.push(git(args)),
        "fullops" => out.push(Intent::Fullops {
            sub: args.first().map(|t| t.text.clone()).unwrap_or_default(),
        }),
        _ => out.push(Intent::Other),
    }
}

fn classify(line: &str, dialect: Dialect, orca_names: &[String], depth: u8) -> Vec<Intent> {
    let Some((commands, subs)) = split(line, dialect) else {
        return vec![Intent::Unparsed];
    };
    let vars = assignments(&commands);
    let scope = Scope {
        dialect,
        orca_names,
        vars: &vars,
        depth,
    };
    let mut out = Vec::new();
    for tokens in &commands {
        command(tokens, scope, &mut out);
    }
    if depth < DEPTH {
        for sub in &subs {
            out.extend(classify(sub, dialect, orca_names, depth + 1));
        }
    }
    out
}

/// 명령 문자열을 Intent 목록으로 나눈다. `orca_names`는 Orca 실행 파일로 볼 이름(`ORCA_CLI_COMMAND`의 파일 이름)이다.
pub fn classify_shell(command: &str, dialect: Dialect, orca_names: &[String]) -> Vec<Intent> {
    classify(command, dialect, orca_names, 0)
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
        let add = |paths: &[&str], update| Intent::GitAdd {
            paths: paths.iter().map(|p| p.to_string()).collect(),
            update,
        };
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
                r#"& orca.exe terminal send --text "inbox/to_dev.md 를 읽어 주세요" --enter"#,
                PowerShell,
                vec![send(24), Intent::OrcaInject],
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
                vec![add(&[":/"], false), Intent::GitCommit { all: true }],
            ),
            (
                "git add -u src/a.png -- -x.png && git commit -m x",
                Bash,
                vec![
                    add(&["src/a.png", "-x.png"], true),
                    Intent::GitCommit { all: false },
                ],
            ),
            ("git -C sub add a.png", Bash, vec![add(&[":/"], false)]),
            ("git stage --all", Bash, vec![add(&[":/"], false)]),
            (
                "git add --pathspec-from-file=l.txt",
                Bash,
                vec![add(&[":/"], false)],
            ),
            ("D=.fullops; git add $D", Bash, vec![add(&[":/"], false)]),
            (
                r#"git add "$(git rev-parse --show-toplevel)""#,
                Bash,
                vec![add(&[":/"], false), Intent::Other],
            ),
            ("git add %D%", PowerShell, vec![add(&[":/"], false)]),
            ("git add -u", Bash, vec![add(&[], true)]),
            ("git add src", Bash, vec![add(&["src"], false)]),
            (
                "orca orchestration dispatch --task t --to term",
                Bash,
                vec![Intent::OrcaWorkerStart],
            ),
            (
                "orca orchestration dispatch --task t --to term --dry-run",
                Bash,
                vec![Intent::Other],
            ),
            (
                "orca --json orchestration dispatch --task t --to term",
                Bash,
                vec![Intent::OrcaWorkerStart],
            ),
            (
                "orca orchestration --json worker-start --run r --spec x",
                Bash,
                vec![Intent::OrcaWorkerStart],
            ),
            (
                "orca --json terminal send --terminal t --text 'inbox/to_dev.md'",
                Bash,
                vec![send(15), Intent::OrcaInject],
            ),
            (
                "orca worktree create --name orchestration",
                Bash,
                vec![Intent::Other],
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

    /// 감싼 꼴(서브셸, 제어 키워드, 앞붙이 명령, 명령 치환, 중첩 셸)도 같은 Intent가 된다.
    #[test]
    fn sees_through_wrappers() {
        let bash = [
            "(orca orchestration worker-start --spec x)",
            "{ orca orchestration worker-start; }",
            "if true; then orca orchestration worker-start; fi",
            "while true; do orca orchestration worker-start; done",
            "true || ! orca orchestration worker-start",
            "f() { orca orchestration worker-start; }; f",
            "env orca orchestration worker-start",
            "env -u HOME ORCA_X=1 orca orchestration worker-start",
            "command orca orchestration worker-start",
            "exec orca orchestration worker-start",
            "nohup orca orchestration worker-start &",
            "time -p orca orchestration worker-start",
            "sudo -u dev -E orca orchestration worker-start",
            "echo x | xargs -n 1 orca orchestration worker-start",
            "timeout -k 5 60 orca orchestration worker-start",
            "echo $(orca orchestration worker-start)",
            "x=`orca orchestration worker-start`",
            "echo \"started: $(orca orchestration worker-start)\"",
            "git commit -m \"run `orca orchestration worker-start`\"",
            "sh -ec 'orca orchestration worker-start'",
            "bash -c \"bash -c 'orca orchestration worker-start'\"",
            "eval \"orca orchestration worker-start\"",
            "/usr/local/bin/orca orchestration worker-start",
        ];
        let powershell = [
            "Start-Process orca -ArgumentList 'orchestration','worker-start'",
            "Start-Process -FilePath orca.exe -ArgumentList \"orchestration worker-start --spec x\" -Wait",
            "Invoke-Expression 'orca orchestration worker-start'",
            "iex \"orca orchestration worker-start\"",
            "& { orca orchestration worker-start }",
            "& {orca orchestration worker-start}",
            "if ($true) { & orca orchestration worker-start }",
            "$out = $(orca orchestration worker-start)",
            "Write-Host \"$(orca orchestration worker-start)\"",
            "orca.ps1 orchestration worker-start",
            "orca.cmd orchestration worker-start",
            r"& 'C:\Program Files\Orca\bin\orca.cmd' orchestration worker-start",
            "cmd /c orca orchestration worker-start",
            "pwsh -NoProfile -Command orca orchestration worker-start",
        ];
        for (commands, dialect) in [(&bash[..], Bash), (&powershell[..], PowerShell)] {
            for command in commands {
                assert!(
                    of(command, dialect).contains(&Intent::OrcaWorkerStart),
                    "{dialect:?}: {command} -> {:?}",
                    of(command, dialect)
                );
            }
        }
        // 터미널 주입도 같다
        assert!(
            of(
                "(orca terminal send --terminal t --text 'read .fullops/inbox/to_dev.md')",
                Bash
            )
            .contains(&Intent::OrcaInject)
        );
    }

    /// 명령이 글로만 들어 있으면 걸리지 않는다(과잉 차단 방지).
    #[test]
    fn leaves_plain_text_alone() {
        let hit = |i: &Intent| {
            matches!(
                i,
                Intent::OrcaWorkerStart | Intent::OrcaInject | Intent::OrcaTerminalSend { .. }
            )
        };
        let bash = [
            "echo 'orca orchestration worker-start'",
            "echo \"orca orchestration worker-start (금지)\"",
            "echo orca orchestration worker-start",
            "grep -rn 'orca orchestration worker-start' docs",
            "rg \"orca terminal send --text inbox/to_\" crates",
            "git commit -m \"fix(hook): orca orchestration worker-start 막기\"",
            "git commit -m 'T-01: `orca orchestration worker-start` 거부'",
            "git commit -m \"T-01: \\`orca orchestration worker-start\\` 거부\"",
            "git log --grep='orca orchestration worker-start' | head -5",
            "cat <<'EOF' > notes.md\n`orca orchestration worker-start`는 쓰지 않는다\n$(orca orchestration worker-start)\nEOF",
            "printf '%s\\n' '(orca orchestration worker-start)' '{ orca orchestration worker-start; }'",
            "command -v orca",
            "env | grep ORCA",
            "sudo echo 'orca orchestration worker-start'",
            "timeout 60 echo orca orchestration worker-start",
            "bash -c \"echo 'orca orchestration worker-start'\"",
            "test -f x && echo \"$(cat x) orca orchestration worker-start\"",
            // 커밋 글을 here-doc으로 넘기는 흔한 꼴: 본문의 괄호·따옴표·백틱은 글이다
            "git commit -m \"$(cat <<'EOF'\n1) 첫째 2) 둘째: `orca orchestration worker-start`를 막는다\n\"따옴표\" $(orca orchestration worker-start)\nEOF\n)\"",
            "echo \"$((1<<2)) orca orchestration worker-start\"",
            "# (orca orchestration worker-start)\nls",
        ];
        let powershell = [
            "Write-Host 'orca orchestration worker-start'",
            "Write-Host \"orca orchestration worker-start (금지)\"",
            "Select-String -Path docs/*.md -Pattern 'orca orchestration worker-start'",
            "git commit -m \"fix(hook): orca orchestration worker-start 막기\"",
            "$t = @'\n(orca orchestration worker-start)\n'@\nSet-Content notes.md $t",
            "Get-Command orca | ForEach-Object { $_.Source }",
            "Start-Process notepad 'orca orchestration worker-start'",
            "if (Test-Path x) { Write-Host 'orca orchestration worker-start' }",
        ];
        for (commands, dialect) in [(&bash[..], Bash), (&powershell[..], PowerShell)] {
            for command in commands {
                let got = of(command, dialect);
                assert!(!got.iter().any(hit), "{dialect:?}: {command} -> {got:?}");
            }
        }
    }

    #[test]
    fn commit_all_reads_only_flags_before_a_value() {
        let all = |command: &str| of(command, Bash) == vec![Intent::GitCommit { all: true }];
        for command in ["git commit -a", "git commit -qam msg", "git commit -m x -a"] {
            assert!(all(command), "{command}");
        }
        for command in [
            "git commit -mall",
            "git commit -mfeat",
            "git commit -m\"add a\"",
            "git commit -uall -m x",
            "git commit -Fa.txt",
        ] {
            assert!(!all(command), "{command}");
        }
    }
}
