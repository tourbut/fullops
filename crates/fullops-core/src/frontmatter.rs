//! front matter 해석(설계 6.1.3절). 문법은 `이름: 값`과 `이름: [a, b]`뿐이다.

use crate::model::{Inbox, RoleId, RouteClass, Sha, SubagentLevel, TaskKey, TestLevel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FmValue {
    Text(String),
    List(Vec<String>),
}

/// 적힌 순서를 지키는 필드 목록.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrontMatter {
    pub fields: Vec<(String, FmValue)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontMatterError {
    /// 파일이 `---` 줄로 시작하지 않는다.
    Missing,
    /// 닫는 `---` 줄이 없다.
    Unclosed,
    /// `이름: 값` 꼴이 아닌 줄(1부터 세는 줄 번호).
    BadLine(u32),
}

impl FrontMatter {
    /// 글 값. 목록이거나 없으면 `None`.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, v)| match v {
                FmValue::Text(t) => Some(t.as_str()),
                FmValue::List(_) => None,
            })
    }

    /// 목록 값. 글이거나 없으면 `None`.
    pub fn list(&self, name: &str) -> Option<&[String]> {
        self.fields
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, v)| match v {
                FmValue::List(l) => Some(l.as_slice()),
                FmValue::Text(_) => None,
            })
    }

    /// 있으면 바꾸고 없으면 끝에 더한다.
    pub fn set(&mut self, name: &str, value: FmValue) {
        match self.fields.iter_mut().find(|(n, _)| n == name) {
            Some(field) => field.1 = value,
            None => self.fields.push((name.to_string(), value)),
        }
    }

    /// `---` 줄을 포함한 front matter 글. 끝에 줄바꿈이 있다.
    pub fn render(&self) -> String {
        let mut out = String::from("---\n");
        for (name, value) in &self.fields {
            match value {
                FmValue::Text(t) => out.push_str(&format!("{name}: {t}\n")),
                FmValue::List(l) => out.push_str(&format!("{name}: [{}]\n", l.join(", "))),
            }
        }
        out.push_str("---\n");
        out
    }
}

/// front matter와 본문이 시작하는 바이트 위치를 돌려준다.
pub fn parse_front_matter(text: &str) -> Result<(FrontMatter, usize), FrontMatterError> {
    let bom = if text.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let mut offset = bom;
    let mut fm = FrontMatter::default();
    for (index, raw) in text[bom..].split_inclusive('\n').enumerate() {
        offset += raw.len();
        let line = raw.trim_end_matches(['\n', '\r']).trim_end();
        if index == 0 {
            if line != "---" {
                return Err(FrontMatterError::Missing);
            }
            continue;
        }
        if line == "---" {
            return Ok((fm, offset));
        }
        if line.trim().is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or(FrontMatterError::BadLine(index as u32 + 1))?;
        let name = name.trim();
        if name.is_empty()
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err(FrontMatterError::BadLine(index as u32 + 1));
        }
        let value = value.trim();
        let parsed = match value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            Some(inner) => FmValue::List(
                inner
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect(),
            ),
            None => FmValue::Text(value.to_string()),
        };
        fm.fields.push((name.to_string(), parsed));
    }
    Err(if offset == bom {
        FrontMatterError::Missing
    } else {
        FrontMatterError::Unclosed
    })
}

/// 인박스 전용 필드를 읽는다. 하나라도 없거나 문법에 안 맞으면 `None`이다.
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
        subagent_level: SubagentLevel::parse(fm.get("subagent_level")?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const INBOX: &str = "---\nid: handover-dev\ntitle: PAY-12 — 결제: 재시도 버튼\nstatus: draft\ntasks: [PAY-12, PAY-13]\n\
key: PAY-12\nrole: dev\ngoal: 결제 재시도 버튼\nbase: 4f0c1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b\nroute: direct\nfrom: lead\n\
round: 0\nbudget: 2\ntest_level: lite\nsubagent_level: lite\n---\n# 본문\n";

    #[test]
    fn reads_fields_and_body_offset() {
        let (fm, body) = parse_front_matter(INBOX).unwrap();
        assert_eq!(fm.get("title"), Some("PAY-12 — 결제: 재시도 버튼"));
        assert_eq!(fm.list("tasks").unwrap(), ["PAY-12", "PAY-13"]);
        assert_eq!(&INBOX[body..], "# 본문\n");
        let inbox = inbox_of(&fm).unwrap();
        assert_eq!(
            (inbox.key.as_str(), inbox.round, inbox.from.as_str()),
            ("PAY-12", 0, "lead")
        );
        assert_eq!(parse_front_matter(&fm.render()).unwrap().0, fm);
    }

    #[test]
    fn rejects_broken_front_matter() {
        assert_eq!(
            parse_front_matter("# 제목\n"),
            Err(FrontMatterError::Missing)
        );
        assert_eq!(parse_front_matter(""), Err(FrontMatterError::Missing));
        assert_eq!(
            parse_front_matter("---\nid: a\n"),
            Err(FrontMatterError::Unclosed)
        );
        assert_eq!(
            parse_front_matter("---\nid: a\n- 목록\n---\n"),
            Err(FrontMatterError::BadLine(3))
        );
        let (fm, _) = parse_front_matter("\u{feff}---\r\nkey: A-1\r\n---\r\n").unwrap();
        assert_eq!(fm.get("key"), Some("A-1"));
        assert!(inbox_of(&fm).is_none());
    }
}
