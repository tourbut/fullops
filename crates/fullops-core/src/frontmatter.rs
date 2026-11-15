//! front matter 읽기와 쓰기(설계 6.1.3절). 문법은 1.24.0의 `deliverables.py`와 같다: 기존 문서를 고치지 않고 같은 값으로 읽는다.
//! 인박스와 산출물 문서가 이 파서 하나를 쓴다.

use crate::model::{Inbox, Parallel, RoleId, RouteClass, Sha, TaskKey, TestLevel};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FmValue {
    Text(String),
    List(Vec<String>),
    /// JSON 객체. 쓰는 곳은 산출물 문서의 `statuses`(산출물 ID → 상태) 하나다.
    Map(BTreeMap<String, String>),
}

/// 파일에 적힌 순서를 지키는 필드 목록.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrontMatter {
    pub fields: Vec<(String, FmValue)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontMatterError {
    /// 파일이 `---` 줄로 시작하지 않거나 닫는 `---` 줄이 없다. front matter가 없는 문서다.
    Missing,
    /// JSON으로 적은 값(`"…"`, `{…}`)을 풀지 못했다(1부터 세는 줄 번호). front matter 전체가 문법 오류다.
    Syntax(u32),
}

impl FrontMatter {
    fn value(&self, name: &str) -> Option<&FmValue> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    /// 글 값. 목록·객체이거나 없으면 `None`.
    pub fn get(&self, name: &str) -> Option<&str> {
        match self.value(name)? {
            FmValue::Text(t) => Some(t),
            _ => None,
        }
    }

    /// 목록 값. 글·객체이거나 없으면 `None`.
    pub fn list(&self, name: &str) -> Option<&[String]> {
        match self.value(name)? {
            FmValue::List(l) => Some(l),
            _ => None,
        }
    }

    /// 객체 값. 글·목록이거나 없으면 `None`.
    pub fn map(&self, name: &str) -> Option<&BTreeMap<String, String>> {
        match self.value(name)? {
            FmValue::Map(m) => Some(m),
            _ => None,
        }
    }

    /// 있으면 그 자리에서 바꾸고 없으면 끝에 더한다.
    pub fn set(&mut self, name: &str, value: FmValue) {
        match self.fields.iter_mut().find(|(n, _)| n == name) {
            Some(field) => field.1 = value,
            None => self.fields.push((name.to_string(), value)),
        }
    }

    /// `---` 줄을 포함한 front matter 글(1.24.0의 `render`와 같은 표기). 끝에 줄바꿈이 있다. 필드는 `fields`의 순서대로 쓴다.
    pub fn render(&self) -> String {
        let mut out = String::from("---\n");
        for (name, value) in &self.fields {
            let text = match value {
                FmValue::Text(t) => scalar(t),
                FmValue::List(items) if items.iter().any(|i| needs_json(i)) => {
                    format!("[{}]", join(items.iter().map(|i| json(i))))
                }
                FmValue::List(items) => format!("[{}]", items.join(", ")),
                // BTreeMap이라 키가 정렬돼 있다
                FmValue::Map(map) => format!(
                    "{{{}}}",
                    join(map.iter().map(|(k, v)| format!("{}: {}", json(k), json(v))))
                ),
            };
            out.push_str(&format!("{name}: {text}\n"));
        }
        out.push_str("---\n");
        out
    }
}

fn json(text: &str) -> String {
    serde_json::Value::from(text).to_string()
}

// Python의 json.dumps와 같게 원소 사이에 ", "를 둔다(1.24.0이 쓴 문서와 같은 글이 되게)
fn join(items: impl Iterator<Item = String>) -> String {
    items.collect::<Vec<_>>().join(", ")
}

/// 글 값을 그대로 쓰면 다시 읽을 때 다른 값이 되는가.
fn needs_json(value: &str) -> bool {
    value.is_empty()
        || value.contains([':', '#', '[', ']', '{', '}', ',', '\n', '\r', '"', '\''])
        || value.starts_with(['-', '?', '&', '*', '!', '|', '>', '@', '`', '%'])
        || value.trim() != value
        || ["true", "false", "yes", "no", "null", "~"]
            .iter()
            .any(|word| value.eq_ignore_ascii_case(word))
}

fn scalar(value: &str) -> String {
    if needs_json(value) {
        json(value)
    } else {
        value.to_string()
    }
}

fn unquote(value: &str) -> &str {
    value.trim_matches(['\'', '"'])
}

/// `이름: 값` 줄을 나눈다. 이름은 줄 맨 앞의 `[A-Za-z_][A-Za-z0-9_-]*`다.
fn name_value(line: &str) -> Option<(&str, &str)> {
    let end = line
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(line.len());
    let (name, rest) = line.split_at(end);
    let first = name.bytes().next()?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }
    Some((name, rest.trim_start().strip_prefix(':')?.trim()))
}

fn parse_value(value: &str) -> Option<FmValue> {
    if value.starts_with('{') {
        return serde_json::from_str(value).ok().map(FmValue::Map);
    }
    if value.starts_with('[') && value.ends_with(']') {
        // [57]은 숫자가 아니라 과제 키 '57'이다. 문자열 배열이 아니면 쉼표로 나눈다
        return Some(FmValue::List(serde_json::from_str(value).unwrap_or_else(
            |_| {
                value[1..value.len() - 1]
                    .split(',')
                    .filter(|item| !item.trim().is_empty())
                    .map(|item| unquote(item.trim()).to_string())
                    .collect()
            },
        )));
    }
    if value.starts_with('"') && value.ends_with('"') {
        return serde_json::from_str(value).ok().map(FmValue::Text);
    }
    if value.starts_with(['"', '[']) {
        return Some(FmValue::Text(unquote(value).to_string()));
    }
    // 공백 뒤의 `#`부터 줄 끝까지는 주석이다
    let mut cut = value.len();
    let mut after_space = false;
    for (at, c) in value.char_indices() {
        if c == '#' && after_space {
            cut = at;
            break;
        }
        after_space = c.is_whitespace();
    }
    Some(FmValue::Text(unquote(value[..cut].trim_end()).to_string()))
}

/// front matter와 본문이 시작하는 바이트 위치를 돌려준다. 파일 머리의 BOM은 뗀다.
/// `이름: 값` 꼴이 아닌 줄은 건너뛰고, 같은 이름이 두 번 나오면 뒤의 것이 이긴다.
pub fn parse_front_matter(text: &str) -> Result<(FrontMatter, usize), FrontMatterError> {
    let mut offset = text.len() - text.trim_start_matches('\u{feff}').len();
    let mut fm = FrontMatter::default();
    for (index, raw) in text[offset..].split_inclusive('\n').enumerate() {
        offset += raw.len();
        let line = raw.trim_end_matches(['\n', '\r']);
        if index == 0 {
            if line.trim() != "---" {
                return Err(FrontMatterError::Missing);
            }
            continue;
        }
        if line.trim() == "---" {
            return Ok((fm, offset));
        }
        if let Some((name, value)) = name_value(line) {
            let value = parse_value(value).ok_or(FrontMatterError::Syntax(index as u32 + 1))?;
            fm.set(name, value);
        }
    }
    Err(FrontMatterError::Missing)
}

/// 인박스 전용 필드 열 개를 읽는다. 하나라도 없거나 타입·어휘가 맞지 않으면 `None`이다(인박스 문법 오류).
pub fn inbox_of(fm: &FrontMatter) -> Option<Inbox> {
    let route = RouteClass::parse(fm.get("route")?)?;
    if !matches!(
        route,
        RouteClass::Direct | RouteClass::Design | RouteClass::Product
    ) {
        return None;
    }
    Some(Inbox {
        key: TaskKey::new(fm.get("key")?).ok()?,
        role: RoleId::new(fm.get("role")?).ok()?,
        goal: fm.get("goal")?.to_string(),
        base: Sha::new(fm.get("base")?).ok()?,
        route,
        from: fm.get("from")?.to_string(),
        round: fm.get("round")?.parse().ok()?,
        budget: fm.get("budget")?.parse().ok()?,
        test_level: TestLevel::parse(fm.get("test_level")?)?,
        parallel: Parallel::parse(fm.get("parallel")?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // 설계 6.1.3절의 예시(base만 40자로 채웠다). `#` 뒤 글은 주석이다
    const INBOX: &str = "---\nid: handover-dev\ntitle: PAY-12 — 결제 재시도 버튼\nstatus: draft\nupdated: 2026-11-13\nowner: dev\n\
summary: 결제 재시도 버튼\nkey: PAY-12\nrole: dev\ngoal: 결제 재시도 버튼\nbase: 4f0c1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b\n\
route: direct          # direct | design | product\nfrom: lead             # lead | <작성한 역할 ID>\nround: 0               # task reopen마다 1 증가\n\
budget: 2              # 반복 예산\ntest_level: lite\nparallel: on\n---\n# PAY-12 — 결제 재시도 버튼\n";

    #[test]
    fn reads_design_inbox_example() {
        let (fm, body) = parse_front_matter(INBOX).unwrap();
        assert_eq!(&INBOX[body..], "# PAY-12 — 결제 재시도 버튼\n");
        let inbox = inbox_of(&fm).unwrap();
        assert_eq!(
            (
                inbox.key.as_str(),
                inbox.round,
                inbox.budget,
                inbox.from.as_str()
            ),
            ("PAY-12", 0, 2, "lead")
        );
        assert_eq!(
            (inbox.route, inbox.parallel),
            (RouteClass::Direct, Parallel::On)
        );
        // 다시 쓰고 다시 읽으면 같은 값이다
        assert_eq!(parse_front_matter(&fm.render()).unwrap().0, fm);
        // 인박스 전용 필드가 하나라도 없으면 문법 오류다
        let (fm, _) = parse_front_matter(&INBOX.replace("parallel: on\n", "")).unwrap();
        assert!(inbox_of(&fm).is_none());
    }

    #[test]
    fn reads_the_1_24_0_grammar() {
        let text = "\u{feff}---\r\nid: [D06, 'D07', \"D09\"]\r\ntitle: \"결제: 재시도 \\\"버튼\\\"\"\r\nstatus: 'draft'\r\n\
statuses: {\"D06\": \"review\", \"D07\": \"draft\"}\r\ntasks: [\"A-1\", \"57\"]\r\nnums: [57]\r\nempty: []\r\n\
\x20indented: 건너뛴다\r\n- 목록 줄도 건너뛴다\r\n한글이름: 건너뛴다\r\nurl: http://x/#y # 주석\r\nstatus: review\r\n---\r\n본문\r\n";
        let (fm, body) = parse_front_matter(text).unwrap();
        assert_eq!(&text[body..], "본문\r\n");
        assert_eq!(fm.list("id").unwrap(), ["D06", "D07", "D09"]);
        assert_eq!(fm.get("title"), Some("결제: 재시도 \"버튼\""));
        assert_eq!(fm.map("statuses").unwrap()["D06"], "review");
        assert_eq!(fm.list("tasks").unwrap(), ["A-1", "57"]);
        assert_eq!(fm.list("nums").unwrap(), ["57"]);
        assert!(fm.list("empty").unwrap().is_empty());
        assert_eq!(fm.get("url"), Some("http://x/#y"));
        // 같은 이름은 뒤의 것이 이기고 자리는 처음 그대로다
        assert_eq!(fm.get("status"), Some("review"));
        assert_eq!(fm.fields[2].0, "status");
        assert_eq!(fm.fields.len(), 8);

        assert_eq!(
            parse_front_matter("# 제목\n"),
            Err(FrontMatterError::Missing)
        );
        assert_eq!(parse_front_matter(""), Err(FrontMatterError::Missing));
        assert_eq!(
            parse_front_matter("---\nid: a\n"),
            Err(FrontMatterError::Missing)
        );
        assert_eq!(
            parse_front_matter("---\nid: a\ntitle: \"닫히지 \\x 않음\"\n---\n"),
            Err(FrontMatterError::Syntax(3))
        );
        assert_eq!(
            parse_front_matter("---\nstatuses: {D06: review}\n---\n"),
            Err(FrontMatterError::Syntax(2))
        );
    }

    #[test]
    fn writes_like_1_24_0() {
        let fm = FrontMatter {
            fields: vec![
                ("id".into(), FmValue::Text("D03".into())),
                ("title".into(), FmValue::Text("결제: 재시도".into())),
                ("empty".into(), FmValue::Text(String::new())),
                ("flag".into(), FmValue::Text("True".into())),
                ("dash".into(), FmValue::Text("-x".into())),
                (
                    "tasks".into(),
                    FmValue::List(vec!["A-1".into(), "B-2".into()]),
                ),
                ("odd".into(), FmValue::List(vec!["a, b".into(), "c".into()])),
                (
                    "statuses".into(),
                    FmValue::Map(
                        [("D07", "draft"), ("D06", "review")]
                            .map(|(k, v)| (k.into(), v.into()))
                            .into(),
                    ),
                ),
            ],
        };
        let text = fm.render();
        assert_eq!(
            text,
            "---\nid: D03\ntitle: \"결제: 재시도\"\nempty: \"\"\nflag: \"True\"\ndash: \"-x\"\ntasks: [A-1, B-2]\n\
odd: [\"a, b\", \"c\"]\nstatuses: {\"D06\": \"review\", \"D07\": \"draft\"}\n---\n"
        );
        assert_eq!(parse_front_matter(&text).unwrap().0, fm);
    }
}
