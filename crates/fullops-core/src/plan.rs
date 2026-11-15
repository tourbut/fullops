//! `PLANS.md` 해석(설계 6.1.2절). 정규식을 쓰지 않는다.

use crate::model::{Plan, PlanRow, PlanStatus, RoleId, TaskKey};

/// `template`은 내장 `PLANS.md` 템플릿이다. 위임 범위에서 안내 문장을 빼는 데만 쓴다.
pub fn parse_plan(text: &str, template: &str) -> Plan {
    // 코드 펜스 안의 줄은 읽지 않는다. 줄 번호가 밀리지 않게 빈 줄로 바꾼다
    let mut fenced = false;
    let lines: Vec<&str> = text
        .lines()
        .map(|line| {
            let fence = line.trim_start().starts_with("```");
            let skip = fenced || fence;
            fenced ^= fence;
            if skip { "" } else { line }
        })
        .collect();
    let mut plan = Plan::default();
    let mut i = 0;
    while i + 1 < lines.len() {
        let Some(cols) = task_table(lines[i], lines[i + 1]) else {
            i += 1;
            continue;
        };
        i += 2;
        while let Some(cells) = lines.get(i).and_then(|line| cells(line)) {
            row(&cols, &cells, i as u32 + 1, &mut plan);
            i += 1;
        }
    }
    plan.delegation = delegation(&lines, template);
    plan
}

struct Columns {
    key: usize,
    status: usize,
    goal: Option<usize>,
    role: Option<usize>,
    source: Option<usize>,
}

/// 표의 한 줄을 칸으로 나눈다. `|`로 시작하지 않으면 표 줄이 아니다.
fn cells(line: &str) -> Option<Vec<String>> {
    // 파일 머리의 BOM은 뗀다(front matter 파서와 같다)
    let inner = line
        .trim_start_matches('\u{feff}')
        .trim()
        .strip_prefix('|')?;
    let mut out = vec![String::new()];
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                chars.next();
                out.last_mut()?.push('|');
            }
            '|' => out.push(String::new()),
            _ => out.last_mut()?.push(c),
        }
    }
    // 줄 끝의 `|` 뒤에 생긴 빈 칸은 칸이 아니다
    if out.last().is_some_and(|c| c.trim().is_empty()) {
        out.pop();
    }
    Some(out)
}

fn is_separator(line: &str) -> bool {
    cells(line).is_some_and(|cs| {
        !cs.is_empty()
            && cs.iter().all(|c| {
                c.contains('-') && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ' | '\t'))
            })
    })
}

/// 헤더 줄과 구분선이 과제 표를 이루면 열 위치를 돌려준다.
fn task_table(header: &str, separator: &str) -> Option<Columns> {
    if !is_separator(separator) {
        return None;
    }
    let heads: Vec<String> = cells(header)?.iter().map(|c| normalize(c)).collect();
    let mut taken = Vec::new();
    let mut find = |words: &[&str]| {
        let at = heads
            .iter()
            .enumerate()
            .position(|(i, h)| !taken.contains(&i) && words.iter().any(|w| h.contains(w)))?;
        taken.push(at);
        Some(at)
    };
    let key = find(&["과제", "키"])?;
    let status = find(&["상태"])?;
    Some(Columns {
        key,
        status,
        goal: find(&["목표", "결과", "다음", "내용"]),
        role: find(&["담당", "책임"]),
        source: find(&["지시서", "원천"]),
    })
}

/// `[글](주소)`는 `글`로 바꾸고 `*`와 `` ` ``를 지운 뒤 앞뒤 공백을 뗀다.
fn normalize(cell: &str) -> String {
    let mut out = String::new();
    let mut rest = cell;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let link = after
            .find(']')
            .filter(|close| after[close + 1..].starts_with('('))
            .and_then(|close| {
                after[close + 2..]
                    .find(')')
                    .map(|end| (close, close + 2 + end))
            });
        match link {
            Some((close, end)) => {
                out.push_str(&rest[..open]);
                out.push_str(&after[..close]);
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.replace(['*', '`'], "").trim().to_string()
}

fn row(cols: &Columns, cells: &[String], line: u32, plan: &mut Plan) {
    let cell = |at: Option<usize>| {
        at.and_then(|i| cells.get(i))
            .map(|c| normalize(c))
            .unwrap_or_default()
    };
    let key = cell(Some(cols.key));
    if key.is_empty() {
        return;
    }
    let Ok(key) = TaskKey::new(key) else {
        plan.parse_errors.push(line);
        return;
    };
    let status = cell(Some(cols.status));
    // 사람이 쓰는 표라 담당 칸의 대소문자를 봐준다
    let role = cell(cols.role).to_ascii_lowercase();
    let role_end = role
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(role.len());
    plan.rows.push(PlanRow {
        key,
        goal: cell(cols.goal),
        role: RoleId::new(&role[..role_end]).ok(),
        after: after(&status),
        status: plan_status(&status),
        source: cell(cols.source),
        line,
    });
}

fn plan_status(cell: &str) -> PlanStatus {
    if let Some(rest) = cell.strip_prefix("보류(") {
        return PlanStatus::Held(inside(rest));
    }
    if let Some(rest) = cell.strip_prefix("대기") {
        let rest = rest.trim_start();
        return match rest.strip_prefix('(').or_else(|| rest.strip_prefix('（')) {
            Some(inner) => PlanStatus::WaitHuman(inside(inner)),
            None => PlanStatus::Ready,
        };
    }
    PlanStatus::Other(cell.to_string())
}

/// 여는 괄호 바로 뒤부터 짝이 맞는 닫는 괄호 전까지. 닫히지 않으면 끝까지.
fn inside(rest: &str) -> String {
    let mut depth = 0;
    for (at, c) in rest.char_indices() {
        match c {
            '(' | '（' => depth += 1,
            ')' | '）' if depth == 0 => return rest[..at].trim().to_string(),
            ')' | '）' => depth -= 1,
            _ => {}
        }
    }
    rest.trim().to_string()
}

fn after(status: &str) -> Vec<TaskKey> {
    let Some((_, list)) = status.split_once("선행:") else {
        return Vec::new();
    };
    list.split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|token| TaskKey::new(token.trim_end_matches(['.', ')', '）'])).ok())
        .collect()
}

fn delegation(lines: &[&str], template: &str) -> Vec<String> {
    let guide: Vec<&str> = template
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lines
        .iter()
        .skip_while(|line| line.trim() != "## 위임 범위")
        .skip(1)
        .take_while(|line| !(line.starts_with("# ") || line.starts_with("## ")))
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !guide.contains(line))
        .take(10)
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = "# 현재 작업\n\n| 과제 키 | 목표 | 담당 | 상태 | 지시서·원천 링크 |\n|---|---|---|---|---|\n\n\
## 위임 범위\n\n사용자가 묻지 않고 이어 가도 된다고 한 범위를 적는다.\n";

    const PLAN: &str = "# 현재 작업\n\n\
| 과제 키 | 목표 | 담당 | 상태 | 지시서·원천 링크 |\n\
|---|---|---|---|---|\n\
| **PAY-12** | 결제 재시도 | dev (서버) | 대기 | [요청](docs/requests/pay-12.md) |\n\
| `PAY-13` | 영수증 | art | 대기 선행: PAY-12, PAY-11. | 사용자 지시 11-13 |\n\
| PAY-14 | 배포 | dev | 대기(사람: 배포) | 승인 |\n\
| PAY-15 | 정리 | DEV | 대기（사람: 결정 D003） | 승인 |\n\
| PAY-16 | 실험 | | 보류(범위 미정 (다음 분기), 선행: PAY-12) | |\n\
| PAY-17 | 끝남 | dev | 완료(4f0c1a2) | 승인 |\n\
| 결제 17 | 문법 오류 | dev | 대기 | 승인 |\n\
|  | 빈 키 | dev | 대기 | 승인 |\n\
\n\
| 이름 | 값 |\n|---|---|\n| X-1 | 과제 표가 아니다 |\n\
\n\
## 위임 범위\n\n\
사용자가 묻지 않고 이어 가도 된다고 한 범위를 적는다.\n\
- 2026-11-13 \"결제 묶음은 묻지 말고 끝까지\"\n\n\
## 완료 보고 후속 요청\n\n- [ ] tester: 독립 QA (PAY-12)\n";

    #[test]
    fn reads_task_table() {
        let plan = parse_plan(PLAN, TEMPLATE);
        let keys: Vec<&str> = plan.rows.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(
            keys,
            ["PAY-12", "PAY-13", "PAY-14", "PAY-15", "PAY-16", "PAY-17"]
        );
        assert_eq!(plan.parse_errors, [11]);

        let row = &plan.rows[0];
        assert_eq!(
            (row.goal.as_str(), row.source.as_str(), row.line),
            ("결제 재시도", "요청", 5)
        );
        assert_eq!(row.role.as_ref().map(RoleId::as_str), Some("dev"));
        assert_eq!(row.status, PlanStatus::Ready);

        let after: Vec<&str> = plan.rows[1].after.iter().map(TaskKey::as_str).collect();
        assert_eq!(
            (plan.rows[1].status.clone(), after),
            (PlanStatus::Ready, vec!["PAY-12", "PAY-11"])
        );
        assert_eq!(
            plan.rows[2].status,
            PlanStatus::WaitHuman("사람: 배포".into())
        );
        assert_eq!(
            plan.rows[3].status,
            PlanStatus::WaitHuman("사람: 결정 D003".into())
        );
        assert_eq!(plan.rows[3].role.as_ref().map(RoleId::as_str), Some("dev"));
        assert_eq!(
            plan.rows[4].status,
            PlanStatus::Held("범위 미정 (다음 분기), 선행: PAY-12".into())
        );
        // 괄호 안에 적은 선행의 마지막 키가 버려지지 않는다
        assert_eq!(plan.rows[4].after.len(), 1);
        assert_eq!(plan.rows[4].source, "");
        assert_eq!(
            plan.rows[5].status,
            PlanStatus::Other("완료(4f0c1a2)".into())
        );
    }

    #[test]
    fn reads_delegation_without_template_lines() {
        let plan = parse_plan(PLAN, TEMPLATE);
        assert_eq!(
            plan.delegation,
            ["- 2026-11-13 \"결제 묶음은 묻지 말고 끝까지\""]
        );
        assert!(parse_plan("", TEMPLATE).rows.is_empty());
        // 코드 펜스 안의 표는 읽지 않고 줄 번호는 그대로다
        let fenced = format!("```\n{PLAN}```\n| 과제 | 상태 |\n|---|---|\n| Z-1 | 대기 |\n");
        let plan = parse_plan(&fenced, TEMPLATE);
        assert_eq!(plan.rows.len(), 1);
        assert_eq!(plan.rows[0].line as usize, fenced.lines().count());
        assert!(parse_plan("| 과제 | 상태 |\n", TEMPLATE).rows.is_empty());
    }
}
