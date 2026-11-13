# fullops 설계 (Rust 재구축, 기준 기능: Fullops-Squad-plugin v1.24.0)

- 상태: 구현 기준 문서. 2026-11-13 작성.
- 읽는 사람: 레포 주인, 구현 에이전트.
- 근거: 원본 v1.24.0의 문서·스킬·hook 등록 파일·템플릿과 `flow_gate.py`·`done_gate.py`·`integration.py`·`board.py`·`policy.py`·`review.py` 일부를 읽어 기능을 확인했다. 실행한 것은 없다. 되돌린 이전 Rust 코드는 읽지 않았다.
- 표기: "미정-Mn"은 이 문서가 정하지 못한 곳이다. 16절에 누가 언제 정하는지 적었다. "DF-n"은 1.24.0과 일부러 다르게 한 판정이다(12.3절).
- 이 README의 '재구축 방식'·'이식 순서' 절과 이슈 #1~#18의 이식 전제는 이 문서로 대체한다. 이슈 수정은 사람이 한다(17절).

## 1. 요지

1. 실행 파일은 `fullops` 하나다. 호스트 hook과 CLI가 같은 바이너리다. hook은 이벤트당 프로세스 하나이고 종료 코드는 항상 0이며 판정은 stdout JSON으로만 낸다. 죽거나 시간이 넘으면 통과가 된다.
2. 배정·대기·완료 준비·병합은 `fullops` 명령이 Orca와 git을 감싸고, 같은 명령이 상태를 쓴다. hook은 감싼 경로 밖의 배정(날것 `worker-start`, 터미널 주입)만 막는다. worker의 완료 전송은 Orca preamble의 `worker_done` 명령을 그대로 쓴다. 날것 `worker_done`과 날것 `git merge`·`git push`는 인정한다.
3. 등록하는 hook은 SessionStart, UserPromptSubmit, PreToolUse, Stop, StopFailure(Claude Code만) 다섯이다. PostToolUse·PostToolUseFailure·SessionEnd는 등록하지 않는다. 도구 호출당 프로세스는 최대 1개다.
4. 사람이 읽고 고치는 레포 문서(`PLANS.md`, 인박스, 규칙, 산출물)는 1.24.0의 자리와 어휘를 유지한다. 기계 상태는 Git 공용 디렉터리의 JSON 파일 하나(`state.json`)에 잠금과 원자 교체로 쓴다. 과제 단계와 통합 여부는 저장하지 않고 계산한다.
5. 활성 표식은 `.fullops-squad/fullops.toml`이다. Python판(`fullops.json`)과 표식이 달라 한 PC에 두 플러그인이 함께 있어도 서로 간섭하지 않는다. 전환은 레포 단위로 표식을 바꾸는 커밋 하나다.
6. 판정 규칙은 정적 표 하나가 정본이다(7.5절). 모르는 사실은 통과다. 막는 예외는 정책 오류 하나다.
7. v1에서 빼는 것: 이슈 자동 모드, 탐색(find·context·패킷·색인), 섀도 판정, 긴 출력 줄이기, HTML 현황판, 웹·Unity·모션 도구, OCR CLI, caveman 주입, grok·agy.

## 2. 목표와 비목표

### 2.1 목표

| # | 목표 | 확인 방법 |
|---|---|---|
| G1 | 1.24.0과 같은 상황에서 같은 판정(통과·안내·경고·거부·차단)을 내고, 흐름이 사람 없이 같거나 더 멀리 이어진다 | 12절 비교 |
| G2 | hook 오류·시간 초과·잠금 실패·Orca 장애가 작업을 막지 않는다 | 장애 주입 시나리오 |
| G3 | 무관한 도구 호출은 git·Orca 프로세스 0개, 파일 쓰기 0회로 끝난다(세션에서 그 이벤트의 첫 호출이 남기는 실행 표식 하나는 예외) | `cargo xtask bench`의 프로세스 수 |
| G4 | 실행 중 Python·Node가 필요 없다 | 배포본에 `.py`·`.js`·`.cjs`·`.mjs` 0개(Orca 패널 제외) |
| G5 | 어떤 차단이든 규칙 ID와 탈출구가 문구에 있고, 사후에 `fullops why`로 확인할 수 있다 | 시나리오 단언 |
| G6 | jevpet 전환이 명령 하나, 되돌리기가 명령 하나다 | 이행 왕복 시나리오 |

### 2.2 성능 목표

Windows 11, 릴리스 빌드, p95 기준이다. 수치는 측정 전 목표이며 묶음 G의 첫 측정 뒤 확정한다(미정-M9).

| 경로 | 목표 | 1.24.0 기준선(1.17.0 실측, 한 PC) |
|---|---|---|
| 비활성 레포의 모든 이벤트 | 15ms | - |
| PreToolUse, 규칙 후보가 없는 호출 | 30ms, git 0·Orca 0·쓰기 0 | 약 500ms (Pre) + 약 500ms (Post) |
| PreToolUse, 규칙을 평가하는 호출 | 80ms, git 1 이하 | 약 500ms |
| Stop, lead, 통합 대기 0건 | 150ms, git 6 이하·Orca 0 | 약 730ms (프로세스 2개) |
| Stop, dispatch 받은 세션, 미정산 | Orca 응답 시간 + 100ms | - |
| SessionStart | 250ms | - |

### 2.3 비목표

- 1.24.0과의 상태 파일·출력 글자·명령 이름 호환.
- 셸 우회 방지. 명령 해석은 보안 경계가 아니다.
- 1.24.0에 없던 hook 차단 추가. 감싼 명령이 자기 입력을 거절하는 것은 12.3절에 적은 것뿐이다.
- 판정에 LLM을 쓰는 것, hook 경로의 네트워크 사용.

## 3. 유지할 결정 여덟 가지가 지켜지는 곳

| 결정 | 설계의 위치 |
|---|---|
| 리뷰는 위험 기반. 테스트·리뷰는 최소·표적. LLM 호출을 늘리지 않음 | `review need`는 기준 시점 정책(400줄·15파일·always)으로만 판정한다(8.9절). 불필요 판정이면 `merge`가 리뷰 없이 진행한다. hook·`wait`·`merge`는 모델을 부르지 않는다. `wait`가 heartbeat·status를 흡수한다(8.6절). 병합 뒤 lint는 트리가 달라졌을 때만 한 번 돈다(M-07) |
| 되돌릴 수 있는 선택과 버그 수정 여부는 자율 판단 | SessionStart 안내 문구(S-01)와 스킬 문서. 결정 기록(`decision add`)을 요구하는 규칙이 없다. 사람에게 올리는 곳은 8.8절 M-04(대규모 리뷰), D-07(한도 재배정 초과), `task reopen`의 예산 알림뿐이다 |
| 운영 배포·부하 테스트·backfill·운영 데이터 정리는 항상 사람 승인 | S-01의 lead 안내에 매 세션 넣는다. `PLANS.md`의 `대기(사람: …)` 행은 준비된 행에서 빠지고 뒤 행을 막지 않는다(6.6절) |
| 제품이 정한 사용자 선검토 유지 | 같은 `대기(사람: …)` 어휘와 `decision`의 SHA 고정 결정(8.10절) |
| 이슈 모드가 꺼져 있으면 보류 잡이 무관한 worker를 Orca 조회에 묶지 않음 | v1에는 이슈 모드 코드가 없다. hook의 Orca 조회는 X-03·T-11·F-01 세 곳뿐이고 모두 이슈 상태와 무관하다. 이슈 모드를 넣을 때의 조건을 18.3절에 고정했다 |
| 탐색 패킷은 선택. 패킷이 있는 과제만 검사 | v1에는 패킷이 없다. 배정·완료·리뷰 검사에 패킷 항목이 없다. 나중에 넣을 때도 "패킷 파일이 있을 때만 검사"로 넣는다(18.3절) |
| 한도로 worker가 끝나면 같은 모델을 기다리고 재배정은 과제당 2회. 다른 모델·호스트로 자동 전환하지 않음 | F-01이 escalation을 dispatch당 한 번 보낸다. `dispatch --reason limit`은 직전과 같은 tuple만 받고 3회째를 거절한다(D-07). `dispatch`에는 다른 호스트로 넘어가는 경로가 없다. Orca 기동 실패는 종료 코드 4로 끝난다 |
| 지표는 과제당 사람 턴 수, 사람 없이 이어진 과제 수. 본문은 저장하지 않음 | P-02가 `turns.jsonl`에 시각·세션 ID·분류만 쓴다(6.2절). 계산식은 6.7절. 프롬프트·명령·메시지 본문은 어떤 상태 파일에도 쓰지 않는다 |

## 4. 용어

| 용어 | 뜻 |
|---|---|
| 활성 레포 | `.fullops-squad/fullops.toml`이 있는 Git 레포 |
| lead | 흐름을 책임지는 세션. coor 모드의 coordinator, dev 모드의 주 담당. dispatch를 받은 세션은 lead가 아니다 |
| 역할 인박스 | `.fullops-squad/handovers/to_<역할>.md`. 역할당 하나. 파일이 없으면 빈 인박스다 |
| 완료 보고(Completion) | 구현·설계 dispatch의 `worker_done` 한 건. 과제 키와 SHA 하나를 가진다 |
| 통합 기준 ref | `git.remote`가 있으면 `refs/remotes/<remote>/<base>`, 없으면 `refs/heads/<base>` |
| 통합됨 | 완료 SHA가 통합 기준 ref의 조상이다 |
| 준비 커밋 / 완료 기록 커밋 / 병합 기록 커밋 | 과제당 기록 커밋 세 개. 각각 `dispatch`, `done`, `merge`가 만든다 |
| 기준 시점 설정 | merge-base 커밋의 `fullops.toml`·`lint/lint.json` 내용 |
| 턴 | 세션에서 UserPromptSubmit과 다음 UserPromptSubmit 사이 |

## 5. 도메인 모델

타입은 `fullops-core::model`에 둔다. 식별자는 newtype이고 생성할 때 검증한다.

```rust
pub struct TaskKey(String);   // ^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$
pub struct RoleId(String);    // ^[a-z][a-z0-9_-]{0,31}$
pub struct Sha(String);       // 40자 소문자 hex
pub struct SessionId(String); // 호스트 값에서 [A-Za-z0-9_-] 밖 글자를 '_'로 바꾸고 100자로 자른 것
pub struct RunId(String); pub struct DispatchId(String); pub struct MessageId(String);
pub struct DecisionId(String); // D001 형식

pub enum Mode { Coor, Dev }
pub enum RoleKind { Worker, Design, Tester, Coordinator }
pub enum Routing { Legacy, Split }
pub enum TestLevel { Minimal, Lite, Standard, Full, Exhaustive }
pub enum SubagentLevel { Off, Lite, Standard, Full }
pub struct ModelTuple { pub agent: String, pub model: String, pub effort: String }

pub struct Config {              // fullops.toml (6.1절)
    pub schema: u32, pub applied_version: String, pub mode: Mode,
    pub primary_role: Option<RoleId>, pub primary_branch: Option<String>,
    pub routing: Routing, pub tech_plan_role: Option<RoleId>,
    pub test_level: TestLevel, pub subagent_level: SubagentLevel,
    pub base: String, pub remote: Option<String>,
    pub roles: BTreeMap<RoleId, Role>,      // Role { branch, kind, about }
    pub review: ReviewPolicy,               // min_code_lines=400, min_code_files=15, always=[], auto=false
    pub code: CodeDef,                      // extensions
    pub wait: WaitConfig,                   // check_minutes=60, ready_timeout_seconds=90, log_limit=30
}

pub enum Seat { Lead, Role { id: RoleId, kind: RoleKind }, Reviewer, Other }
pub struct Actor { pub seat: Seat, pub dispatch: Option<DispatchId>, pub direct_dev: bool }
// is_lead() = seat == Lead && dispatch.is_none();  direct_dev = mode == Dev && is_lead()

pub enum RouteClass { Direct, Design, Product, Unresolved, Untracked }
// 1.24.0의 simple·implementation = Direct, design = Design, product = Product
pub enum Purpose { Build, Fix, Review, Research }
pub enum Stage { None, Routed, Assigned, Reported, Reviewing, Merged, Done }   // 순서 있음, 유도값
pub enum Landing { MergeLeft, PushLeft, Landed }                              // 유도값

pub enum PlanStatus { Ready, WaitHuman(String), Held(String), Other(String) }
pub struct PlanRow { pub key: TaskKey, pub goal: String, pub role: Option<RoleId>,
                     pub status: PlanStatus, pub after: Vec<TaskKey>, pub source: String, pub line: u32 }
pub struct Plan { pub rows: Vec<PlanRow>, pub delegation: Vec<String>, pub parse_errors: Vec<u32> }

pub struct Need { pub required: bool, pub action: NeedAction /* Skip|Ask|Review */,
                  pub reason: String, pub code_files: u32, pub code_lines: u32, pub always: Vec<String>,
                  pub base: Sha, pub head: Sha, pub merge_base: Sha }

pub enum Known<T> { Yes(T), Unknown(&'static str) }   // 모든 바깥 사실의 형태
pub enum Verdict { Pass, Context(String), Warn(String),
                   Deny { rule: &'static str, reason: String },
                   Block { rule: &'static str, reason: String, also: Vec<&'static str> } }
```

불변식은 다음과 같고, 지키는 주체는 CLI와 `fullops-store`다.

1. 역할당 인박스는 하나다. 인박스 파일은 `task new`·`task reopen`만 만들고 `done`만 없앤다.
2. `Dispatch`는 `dispatch`만 만들고 반드시 `Run`을 가진다.
3. `Completion`은 목적이 Build·Fix인 dispatch(설계 역할의 dispatch 포함)의 성공 보고에서만 생긴다. Review·Research 보고에서는 생기지 않는다. Orca에 ack하기 전에 저장한다.
4. 보류(hold)는 사유·담당·재개 조건 셋을 다 가진다. 통합되면 지우지 않고 `released_hold`로 옮긴다.
5. 리뷰·면제·보류 사유 기록은 덮어쓰지 않는다. 같은 과제를 다시 리뷰하면 SHA가 달라 폴더가 달라진다.
6. 본문(프롬프트, 메시지, 명령 문자열)은 어떤 상태 파일에도 쓰지 않는다.

## 6. 상태 저장

### 6.1 커밋되는 파일 (`.fullops-squad/` 아래)

| 경로 | 형식 | 쓰는 주체 | 내용 |
|---|---|---|---|
| `fullops.toml` | TOML | `init`·`mode`·`upgrade`·`migrate`, 사람 | 활성 표식이자 `Config` |
| `PLANS.md` | Markdown, 6.1.2 문법 | lead, `plan`, `merge` | 대기 목록, 위임 범위, 후속 요청, 배정 기록 |
| `handovers/to_<역할>.md` | front matter + Markdown | `task new\|reopen`이 만들고 본문은 사람·에이전트 | 현재 지시서, 진행 체크, 완료 보고 |
| `handovers/logs/<KEY>.<역할>.r<차수>.md` | 같음 | `done`(`git mv`) | 끝난 과제의 지시서와 완료 보고 전문. 1.24.0의 날짜별 로그는 그 자리에 읽기 전용으로 남는다 |
| `routes/<KEY>.json` | JSON | `route`, `dispatch`(tuple 기록) | 분류 기록 |
| `reviews/<KEY>/<head 12자>/` | JSON·Markdown | `review`, 리뷰어 | `targets.json`, `result.json`, `report.md`, `lint.json`, `accepted.json`, `waiver.json`, `abandoned.json`, `superseded.json` |
| `lint/lint.json` | JSON (1.24.0 형식 그대로) | 사람 | lint 명령·크기·제외·정규식 규칙. 형식을 유지하는 이유: 레포 규칙을 변환 없이 읽어 결과 비교를 단순하게 한다 |
| `review/rule.json` | JSON (1.24.0 형식 그대로) | 사람 | 리뷰 대상 include·exclude와 경로별 규칙 문장 |
| `docs/…`, `rules/…`, `FULLOPS.md`, `project.md`, `orca-agents.md`, `contexts/<역할>.md` | Markdown | 사람·에이전트 | 기계는 `docs/deliverables/README.md`(인덱스)와 문서 front matter만 읽는다 |
| `.env*`, `local/`, `**/raw/` | - | 사람 | Git 제외 |

#### 6.1.1 `fullops.toml`

```toml
schema = 2
applied_version = "2.0.0"
mode = "coor"                 # "coor" | "dev"
primary_role = "dev"          # dev 모드에서 필수. 등록 역할이어야 한다
primary_branch = "main"       # dev 모드에서 필수
routing = "legacy"            # "legacy" | "split"
tech_plan_role = "dev"        # routing = "split"일 때 필수. design 역할과 달라야 한다
test_level = "lite"           # minimal|lite|standard|full|exhaustive
subagent_level = "lite"       # off|lite|standard|full

[git]
base = "main"
remote = "origin"             # 없으면 로컬 전용

[roles.dev]
branch = "fullops/dev"        # 역할마다 고유
kind = "worker"               # worker|design|tester|coordinator
about = "서버·도구 코드 구현과 테스트"   # 분류 질문에 쓰는 설명. 생략 가능

[roles.architecture]
branch = "fullops/architecture"
kind = "design"               # legacy에서는 설계 역할, split에서는 제품 기획 역할. 최대 1개

[review]                      # 생략하면 아래 기본값
min_code_lines = 400
min_code_files = 15
always = []
auto = false

[code]                        # 생략하면 내장 목록(8.11절)
extensions = [".py", ".ts"]

[wait]                        # 생략하면 아래 기본값
check_minutes = 60
ready_timeout_seconds = 90
log_limit = 30
```

검증 규칙: `schema == 2`, 역할 브랜치 중복 없음, `kind = "design"`·`"coordinator"`·`"tester"`는 각각 최대 1개, dev 모드의 `primary_*` 존재, split의 `tech_plan_role` 존재. 하나라도 어기면 정책 오류다(7.8절). 모르는 키는 무시한다.

읽는 시점은 두 가지다.

- 신원에 쓰는 값(`mode`, `primary_*`, `roles`, `routing`, `test_level`, `subagent_level`, `git`)은 작업 트리의 파일에서 읽는다.
- 검사 기준에 쓰는 값(`[review]`, `[code]`, lint 명령 선택에 쓰는 `test_level`, `lint/lint.json` 전체)은 merge-base 커밋의 blob에서 읽는다. 그 커밋에 `fullops.toml`이 없으면 내장 기본값을 쓴다. 쓰는 곳은 `lint`, `review need`, Stop의 X-01·X-02, `merge`다.

#### 6.1.2 `PLANS.md` 문법

1.24.0 템플릿의 열과 상태 어휘를 그대로 쓴다. 해석은 정규식 없이 다음 규칙으로 한다.

- 과제 표: 헤더 행 다음 줄이 구분선(`|---|…`)인 표 가운데, 헤더에 `과제` 또는 `키`가 든 열과 `상태`가 든 열이 모두 있는 표다. 목표 열은 `목표`·`결과`·`다음`·`내용` 중 하나가 든 열, 담당 열은 `담당`·`책임`, 원천 열은 `지시서`·`원천`이 든 열이다. 과제 표가 여럿이면 모두 읽고 파일 순서를 유지한다.
- 셀 정규화: `[글](주소)`는 `글`로 바꾸고 `*`와 `` ` ``를 지운 뒤 앞뒤 공백을 뗀다. 키 셀이 빈 행은 버린다. 키 셀이 `TaskKey` 문법에 안 맞는 행은 `parse_errors`에 줄 번호를 넣고 버린다.
- 상태 칸:
  - `보류(`로 시작하면 `Held(괄호 안 글)`.
  - `대기`로 시작하고 그 뒤 첫 비공백 글자가 `(` 또는 `（`이면 `WaitHuman(괄호 안 글)`.
  - `대기`로 시작하고 위에 해당하지 않으면 `Ready`.
  - 그 밖은 `Other(원문)`.
- 선행: 상태 칸에 `선행:`이 있으면 그 뒤를 쉼표와 공백으로 나눠 `TaskKey` 문법에 맞는 토큰만 `after`에 넣는다. 토큰 끝의 마침표는 뗀다.
- 담당: 담당 칸 맨 앞의 `[A-Za-z0-9_-]+`가 역할 ID다.
- `## 위임 범위`: 그 제목 다음부터 다음 `#`·`##` 제목 전까지의 비어 있지 않은 줄 가운데 템플릿 안내 문장(바이너리에 내장한 템플릿의 같은 줄)이 아닌 것. 최대 10줄.
- `## 완료 보고 후속 요청`: `- [ ] <역할>: <글> (<KEY>)` 줄. `merge`가 덧붙인다.
- `## 배정 기록`: `- <YYYY-MM-DD HH:MM> <KEY> → <역할> | <agent>/<model>/<effort> | <fresh|retain> | run <id> | 재배정 <n> | <이유>` 줄. `merge`가 병합 기록 커밋에서 덧붙인다. 사람이 배정 근거를 보는 자리다.

`fullops plan check`가 `parse_errors`를 줄 번호와 함께 낸다. hook은 문법에 안 맞는 행을 준비되지 않은 것으로 보고 lead에게 세션당 한 번 알린다.

#### 6.1.3 인박스 front matter

front matter는 파일 맨 앞의 `---` 줄과 다음 `---` 줄 사이다. 문법은 `이름: 값`(값은 줄 끝까지, 앞뒤 공백 제거)과 `이름: [a, b]` 목록뿐이다. YAML의 다른 문법은 지원하지 않는다. 같은 파서(`fullops-core::frontmatter`)로 산출물 문서도 읽는다.

```
---
id: handover-dev
title: PAY-12 — 결제 재시도 버튼
status: draft
updated: 2026-11-13
owner: dev
summary: 결제 재시도 버튼
key: PAY-12
role: dev
goal: 결제 재시도 버튼
base: 4f0c…(40자)
route: direct          # direct | design | product
from: lead             # lead | <작성한 역할 ID>
round: 0               # task reopen마다 1 증가
budget: 2              # 반복 예산
test_level: lite
subagent_level: lite
---
# PAY-12 — 결제 재시도 버튼
(템플릿 본문: 현재 상황, 적용 기준, 먼저 읽을 문서, 해야 할 일, 완료 기준, 갱신할 산출물, 제약, ## 완료 보고)
```

`id`~`summary` 여섯은 문서 공통 필드(8.12절)이고 `key` 이하는 인박스 전용이다. 인박스 전용 필드는 `task new`·`task reopen`만 쓴다.

#### 6.1.4 `routes/<KEY>.json`

```json
{ "v": 1, "key": "PAY-12", "class": "direct", "role": "dev",
  "deliverables": ["D03"], "origin": "user", "decided_by": "jev",
  "reason": "", "request_file": "docs/requests/pay-12.md", "request_sha256": "…",
  "probabilities": { "scope": 0.91, "role": 0.77 },
  "thresholds": { "scope": 0.8, "role": 0.6, "deliverable": 0.5 },
  "tuples": { "dev": { "agent": "claude", "model": "…", "effort": "medium", "why": "기존 패턴 구현" } },
  "created_at": 1794500000,
  "history": [] }
```

- `class`: `direct`·`design`·`product`·`unresolved`·`untracked`. `untracked`는 분류 없이 tuple만 기록한 경우(dev 직접 작업의 리뷰 키)다.
- `origin`: `user`·`bot`. `decided_by`: `jev`·`fallback`·`manual`.
- `tuples`의 키는 역할 ID 또는 `reviewer`다. 기록일 뿐이고 다음 과제의 기본값으로 쓰지 않는다.
- 재분류하면 이전 객체(자기 `history` 제외)를 `history` 끝에 넣는다.

### 6.2 Git 공용 디렉터리 (`<git-common-dir>/fullops/`)

모든 워크트리와 세션이 공유한다. 전제: 역할 워크트리는 한 레포의 `git worktree`다(별도 clone은 지원하지 않는다).

| 파일 | 형식 | 쓰는 주체 | 수명 |
|---|---|---|---|
| `state.json` | JSON | CLI, lead의 Stop·T-11 효과 | 열린 것만 담는다 |
| `state.json.lock` | 빈 파일 | 잠금용 | - |
| `archive.jsonl` | JSONL, 추가만 | `merge`·`settle`·`gc` | 영구. 줄당 과제 하나 |
| `turns.jsonl` | JSONL, 추가만 | P-02 | 영구 |
| `waiters/<run>.json` | JSON | `wait` | 대기 중. 30초마다 갱신 |
| `status.json` | JSON | `state.json`을 쓰는 쪽이 같은 잠금 안에서 다시 만든다 | 덮어쓴다 |
| `py-backup/` | 디렉터리 | `migrate` | 전환 기록 |

#### 6.2.1 `state.json`

```json
{ "v": 1, "seq": 42,
  "runs": [ { "id": "run_ab12", "owner": "lead", "opened_at": 1794500000, "closed_at": null } ],
  "dispatches": [ {
      "id": "dsp_01", "task_id": "task_9", "run": "run_ab12", "key": "PAY-12", "role": "dev",
      "purpose": "build", "tuple": { "agent": "claude", "model": "…", "effort": "medium" },
      "terminal": "term_7", "worktree": "C:/…/dev", "session_mode": "fresh", "reason": null,
      "started_at": 1794500100, "bound_session": null,
      "reported": { "branch": "fullops/dev", "sha": "…", "lint_head": "…",
                    "followups": [ { "role": "tester", "text": "독립 QA" } ], "at": 1794503000 },
      "settled": { "how": "done", "outcome": "succeeded", "message": "msg_3", "at": 1794503050 },
      "limit_escalated": false } ],
  "completions": [ {
      "id": "msg_3", "dispatch": "dsp_01", "key": "PAY-12", "role": "dev", "branch": "fullops/dev",
      "sha": "…", "sha_source": "done", "direct": false, "outcome": "succeeded",
      "received_at": 1794503050, "hold": null, "released_hold": null,
      "landed_at": null, "drift": null } ],
  "messages": [ { "id": "msg_3", "delivery": "dlv_8", "run": "run_ab12", "type": "worker_done",
                  "dispatch": "dsp_01", "received_at": 1794503050, "acked_at": null } ],
  "tasks": { "PAY-12": { "rounds": 0, "budget": 2, "limit_redispatch": 0,
                         "last_tuple": { "agent": "claude", "model": "…", "effort": "medium" } } },
  "sync_pending": { "art": "<기본 브랜치 SHA>" },
  "snapshots": [ { "review": "PAY-12@4f0c1a2b3c4d", "path": "C:/…/.fullops-review-7f3a", "head": "…",
                   "base": "…", "state": "active", "implementer_session": "…", "reviewer_dispatch": null,
                   "owner": "lead", "created_at": 1794504000, "hold": null } ],
  "decisions": [ { "id": "D001", "status": "open", "key": "PAY-12", "question": "…", "options": ["진행", "생략"],
                   "blocks": ["PAY-12"], "asked_at": 1794504100, "origin": "user", "sha": "…",
                   "risks": [], "allow": [], "expires_at": 1794590500,
                   "answer": null, "decided_by": null, "via": null, "closed_at": null } ],
  "next_decision": 2 }
```

필드 규칙:

- `runs[].owner`: `"lead"` 또는 `"dispatch:<DispatchId>"`(dispatch 받은 세션이 만든 하위 Run).
- `dispatches[].purpose`: `build`·`fix`·`review`·`research`. `session_mode`: `fresh`·`retain`. `reason`: `null`·`"limit"`.
- `dispatches[].settled.how`: `done`(worker_done 수신 또는 Orca 조회로 확인), `escalated`(escalation 수신), `lost`(worker 사망 감지).
- `completions[].sha_source`: `done`(`fullops done` 기록), `body`(메시지 본문), `branch_tip`(역할 브랜치 로컬 ref), `manual`(`completion set-sha`). `sha`가 `null`이면 통합 대기로 센다.
- `completions[].direct`: dev 주 담당 직접 작업의 기록. 통합 차단(X-06, D-03)에서는 빼고 단계와 지표에만 쓴다.
- `completions[].landed_at`: 통합이 처음 확인된 시각. 한 번 쓰면 다시 계산하지 않는다(조상 관계는 되돌려지지 않는다).
- `completions[].drift`: 통합 시점에 기본 브랜치가 보고 SHA 뒤의 역할 브랜치 커밋(기록 전용 제외)을 함께 담았으면 그 커밋 수.
- `messages`: 처리할 메시지(worker_done·question·escalation)만 넣는다. 본문은 넣지 않는다. `acked_at`이 `null`이면 미처리다.
- `snapshots[].state`: `creating`·`active`·`removed`·`abandoned`·`missing`.
- `decisions[]`: 필드 뜻은 8.10절.
- `v`가 바이너리가 아는 값보다 크면 읽는 쪽은 `Unknown`으로 본다. 모르는 필드는 보존한다(`serde(flatten)`으로 받아 다시 쓴다).

통합되고 열린 dispatch가 없는 과제는 `merge`·`settle`·`gc`가 `archive.jsonl`로 옮긴다. 줄 형식은 `{"key","role","sha","received_at","landed_at","rounds","dispatches":[{"purpose","started_at","reason"}],"direct":bool}`이다.

#### 6.2.2 `turns.jsonl`

`{"at":1794500000,"sid":"<SessionId>","kind":"human"}` 한 줄. 기계 알림으로 분류한 입력은 `"kind":"machine:orca-message"` 또는 `"machine:task-notification"`으로 남겨 오분류를 나중에 확인할 수 있게 한다. 줄은 200바이트 이하이고 `O_APPEND`로 `write` 한 번에 쓴다. 잠그지 않는다.

#### 6.2.3 `status.json` (Orca 패널과 봇이 읽는 요약)

```json
{ "v": 1, "generated_at": 1794504200, "mode": "coor", "version": "2.0.0",
  "tasks": [ { "key": "PAY-12", "role": "dev", "stage": "reported", "rounds": 0, "budget": 2, "sha": "4f0c1a2", "held": false } ],
  "dispatches": [ { "key": "PAY-12", "role": "dev", "purpose": "build", "agent": "claude", "model": "…", "started_at": 1794500100, "settled": true } ],
  "pending": [ { "key": "PAY-12", "sha": "4f0c1a2", "landing": "merge_left", "hold_reason": null } ],
  "decisions": [ { "id": "D001", "key": "PAY-12", "question": "…", "options": ["진행", "생략"], "asked_at": 1794504100, "expires_at": 1794590500 } ],
  "ready": ["PAY-13"], "sync_pending": { "art": "9c1e0aa" },
  "metrics": { "since": 1794000000, "human_turns": 12, "done_tasks": 5, "turns_per_task": 2.4,
               "unattended_dispatches": 7, "unattended_tasks": 4 },
  "issue": null }
```

token, 이슈 본문, 댓글, 프롬프트는 넣지 않는다. `ready`는 lead 문맥(lead의 hook이나 lead가 실행한 CLI)에서 쓸 때만 다시 계산하고, 그 밖에는 이전 값을 유지한다. 같은 잠금 안에서 쓰므로 오래된 요약이 새 요약을 덮지 않는다.

### 6.3 작업 트리별 Git 디렉터리 (`<git-dir>/fullops/`)

| 파일 | 형식 | 쓰는 주체 | 내용 |
|---|---|---|---|
| `sessions/<sid>.json` | JSON | hook, `route` | 아래 |
| `sessions/<sid>.seen/<이벤트>` | 빈 파일 | hook | 그 세션에서 이벤트 종류가 처음 실행됐다는 표식 |
| `current.json` | JSON | PreToolUse(명령에 `fullops`가 있을 때) | `{"sid","terminal","at"}`. CLI가 자기 세션을 찾는 데 쓴다 |
| `trace/<sid>.jsonl` | JSONL | hook | 통과가 아닌 판정, 오류, 시간 초과. `FULLOPS_TRACE=1`이면 모든 호출 |
| `lint/result.json`, `lint/pass.json`, `lint/logs/<명령>.log`, `lint/lock` | JSON·텍스트 | `lint` | 8.11절 |

세션 파일:

```json
{ "v": 1, "sid": "…", "host": "claude", "started_at": 1794500000, "last_seen": 1794504000,
  "terminal": "term_3", "mode_at_start": ["coor", null, null], "anchor": "<첫 SessionStart의 HEAD>",
  "turn": 3,
  "dispatch": { "id": "dsp_01", "task": "task_9", "bound_by": "preamble" },
  "routed": ["PAY-12"],
  "blocks": { "X-07": { "fp": "9c1e…", "turn": 3 } },
  "noticed": ["hook-error:io", "plan-parse"] }
```

- `anchor`는 세션 파일을 처음 만들 때 한 번만 쓴다. resume·compact에서 유지한다.
- `mode_at_start`는 `[mode, primary_role, primary_branch]`다.
- `dispatch.bound_by`: `preamble`·`terminal`.
- 새 dispatch가 묶이면 `blocks`를 비운다.

추적 줄: `{"t":1794504000,"v":1,"bin":"2.0.0","host":"claude","ev":"PreToolUse","actor":"lead","intents":["OrcaWorkerStart"],"rules":[{"id":"T-01","verdict":"deny","fp":"…"}],"git":0,"orca":0,"ms":7,"err":null}`. 명령 문자열·프롬프트·파일 내용·환경 변수 값은 쓰지 않는다.

### 6.4 사용자 홈

| 경로 | 내용 |
|---|---|
| `~/.fullops/bin/fullops[.exe]` | 고정 위치 실행본 |
| `~/.fullops/bin/fullops.old-<시각>[.exe]` | 교체 전 실행본. `install --prune`만 지운다 |
| `~/.fullops/install.json` | `{"version","sha256","source","at","previous"}` |
| `~/.fullops/cache/jev/<요청 sha256>.json` | 분류 응답 캐시 |

홈은 Windows에서 `USERPROFILE`, 그 밖에서는 `HOME`이다.

### 6.5 동시 접근과 원자성

- `state.json` 쓰기는 `fullops_store::update_state` 하나로만 한다. 순서: `state.json.lock`을 열고 `File::try_lock`을 10ms 간격으로 시도 → 파일을 다시 읽음 → 수정 → `state.json.tmp.<pid>`에 쓰고 `sync_all` → `rename` → `status.json`을 같은 방법으로 교체 → 잠금 해제. OS 잠금이라 프로세스가 죽으면 풀린다.
- 잠금 대기 한도: hook 300ms, CLI 10초. hook이 못 얻으면 쓰기를 건너뛰고 추적에 `lock_busy`를 남긴다. 그 쓰기에 기대는 한 번 막기 규칙은 그 호출에서 차단 대신 경고를 낸다(7.6절).
- 읽기는 잠그지 않는다. Windows에서 `rename`이 실패하면 20ms 간격으로 세 번 다시 한다.
- 바깥 호출(Orca, git push·merge, curl)은 잠금 밖에서 한다. 호출 뒤 잠금 안에서 전제를 다시 확인하고 쓴다.
- 세션 파일은 같은 방법(잠금 파일 `sessions/<sid>.json.lock`)으로 쓴다. `blocks`·`noticed`·`routed`는 합치기 연산이라 병렬 호출 순서와 무관하다.
- `state.json`이 JSON으로 읽히지 않으면 hook은 `Unknown`으로 처리한다(통과). CLI는 `state.json.broken-<시각>`으로 옮기고 빈 상태로 시작한 뒤 종료 코드 1로 그 사실을 알린다. `fullops doctor`가 같은 복구를 한다. 열린 결정과 보류는 `status.json`의 마지막 값에서 사람이 다시 넣는다.

### 6.6 유도값

**단계** `stage_of(key)`: 위에서부터 먼저 맞는 것.

1. `archive.jsonl`에 `landed_at`이 있는 줄이 있고 `state.json`에 그 키의 dispatch·completion이 없다 → `Done`
2. 그 키의 가장 늦은 Build·Fix dispatch가 가장 늦은 completion보다 뒤에 시작했다 → `Assigned`
3. 그 키의 비직접·성공 completion이 하나 이상이고 모두 `landed_at`이 있다 → `Merged`
4. 그 키의 `active` snapshot이 있다 → `Reviewing`
5. completion이 있다(성공·실패 무관) → `Reported`
6. Build·Fix dispatch가 있다 → `Assigned`
7. `routes/<KEY>.json`이 있다 → `Routed`
8. 그 밖 → `None`

**통합 위치** `landing(sha)`: `sha`가 통합 기준 ref의 조상이면 `Landed`. 아니고 원격이 있으며 `refs/heads/<base>`의 조상이면 `PushLeft`. 그 밖은 `MergeLeft`. 판정은 `git merge-base --is-ancestor`다. hook은 fetch하지 않는다.

**통합 대기**: `direct == false`, `outcome == succeeded`, `hold == null`, `landed_at == null`인 completion.

**준비된 행** `ready()`: 다음 다섯을 모두 채우는 `PLANS.md` 행을 표 순서대로.

1. 상태가 `Ready`다.
2. 원천 칸이 비어 있지 않다.
3. 담당 역할의 인박스 파일이 lead 체크아웃에 없다. 담당 칸에서 역할을 읽지 못한 행은 이 조건을 통과로 본다.
4. `after`의 모든 키가 `stage_of >= Reported`다.
5. 자기 키가 `stage_of < Assigned`다.

**세션이 만든 변경** `changed()`: X-01과 `done`이 같은 함수를 쓴다.

1. 시작점: 이 워크트리의 역할 인박스가 있으면 그 `base`, 없으면 세션의 `anchor`. dev 직접 작업은 항상 `anchor`.
2. 바닥: `merge-base(HEAD, 통합 기준 ref)`. 시작점이 바닥의 조상이면 시작점을 바닥으로 바꾼다. 통합 기준 ref가 없으면 그대로 둔다.
3. 시작점이 `HEAD`의 조상이 아니면(amend·rebase) 바닥을 쓴다.
4. 대상: `git diff --name-only -z <시작점> HEAD`와 `git status --porcelain -z --untracked-files=all`의 합집합 가운데 코드 파일(8.11절 `is_code`). dev 직접 작업은 확장자와 무관하게 lint 제외에 안 걸리는 모든 파일이다. 기록 경로는 항상 뺀다.
5. 기록 경로: `.fullops-squad/handovers/`, `.fullops-squad/docs/evaluations/`, `.fullops-squad/routes/`, `.fullops-squad/reviews/`, `.fullops-squad/PLANS.md`.

**lint 통과 인정** `pass_covers(head)`: `lint/pass.json`이 있고, `pass.head == head`이거나 `pass.head`가 `head`의 조상이며 그 사이 diff가 기록 경로뿐이고, `pass.base`가 요구 시작점과 같거나 그 조상이다.

### 6.7 지표

`turns.jsonl`, `state.json`, `archive.jsonl`에서 `fullops status`가 계산한다. 따로 저장하지 않는다.

- 사람 턴 수 = `kind == "human"`인 줄 수.
- 완료 과제 수 = 첫 사람 턴 시각 이후 `landed_at`이 있는 과제(키) 수. 직접 작업 포함.
- 과제당 사람 턴 = 사람 턴 수 ÷ 완료 과제 수(0이면 표시하지 않는다).
- 이어진 배정 수 = Build·Fix dispatch 가운데, 바로 앞의 사건(다른 dispatch 시작 또는 completion 수신)과 자기 시작 시각 사이에 사람 턴이 없는 것의 수. 첫 dispatch는 세지 않는다.
- 이어진 과제 수 = 그 dispatch들의 서로 다른 키 수.
- 재작업 = `rounds > 0`인 과제 수. 참고값이다.

지표는 어떤 규칙의 조건으로도 쓰지 않는다.

### 6.8 정리

- 세션 파일·추적·`seen`: `last_seen`이 14일 지난 것을 lead의 SessionStart가 지운다(50ms 예산).
- 추적 파일은 1MB를 넘으면 앞 절반을 버린다.
- `waiters/<run>.json`: Run이 닫히면 지운다.
- `state.json`의 닫힌 Run·정산된 dispatch·통합된 completion·닫힌 결정(30일 경과)·제거된 snapshot은 `fullops gc`와 `merge`가 `archive.jsonl`로 옮기거나 지운다. 결정은 `archive.jsonl`에 `{"decision":{…}}` 줄로 남긴다.
- `turns.jsonl`·`archive.jsonl`은 지우지 않는다.

## 7. hook

### 7.1 진입과 흐름

명령: `fullops hook <claude|codex> [--plugin-root <경로>] [--in-place]`. 호스트는 인자로 받는다. 이벤트 종류는 stdin의 `hook_event_name`으로 가른다. `main`은 `argv[1] == "hook"`을 인자 파서보다 먼저 본다.

```
1. 감시 스레드를 띄운다. 내부 기한을 넘으면 아직 출력이 없을 때 "{}"를 쓰고 exit(0) 한다.
2. stdin(최대 8MB)을 읽어 host::decode 한다. 실패하면 "{}".
3. cwd에서 위로 올라가며 ".git"(디렉터리 또는 "gitdir:" 파일)과 ".fullops-squad/fullops.toml"을 찾는다.
   git 프로세스는 띄우지 않는다. 표식이 없으면 "{}"를 내고 끝낸다(추적도 쓰지 않는다).
4. sessions/<sid>.seen/<이벤트>가 없으면 만든다(실행 근거).
5. 도구 입력을 Intent·편집 대상으로 분류한다(7.4절). PreToolUse에서 규칙 후보가 없으면 "{}"를 내고 끝낸다.
6. Facts를 지연 계산으로 만들고 core::decide(input, facts)를 부른다.
7. host::encode(verdict)를 stdout에 한 번에 쓴다.
8. 효과를 남은 시간 안에서 실행한다. 실패는 추적에만 남긴다.
9. 통과가 아니거나 오류가 있었으면 추적 한 줄을 쓴다. exit(0).
전체를 catch_unwind로 감싼다. 어느 단계든 오류·panic이면 "{}"를 내고 추적에 err를 남긴다.
```

효과 가운데 판정이 기대는 것(한 번 막기의 `blocks` 기록)은 7번 전에 쓴다. 나머지는 8번에서 쓴다.

| 이벤트 | 내부 기한 | 등록 타임아웃 | git 호출 상한 | Orca 호출 상한 |
|---|---|---|---|---|
| SessionStart | 4초 | 10초 | 3 | 0 |
| UserPromptSubmit | 1.5초 | 5초 | 0 | 0 |
| PreToolUse | 4초 | 8초 | 1 | 1 (T-11만, 2.5초) |
| Stop | 12초 | 20초 | 6 + 통합 대기 건수 × 2 | 1 (X-03만, 3초) |
| StopFailure | 8초 | 15초 | 0 | 1 (6초) |

상한을 넘는 사실 요청은 `Unknown`을 돌려준다.

### 7.2 호스트 어댑터

```rust
pub trait Host {
    fn decode(&self, raw: &[u8]) -> Result<HookInput, DecodeError>;
    fn encode(&self, event: EventKind, v: &Verdict) -> String;
    fn caps(&self) -> Caps;                       // { stop_failure, background_wake }
    fn tool_kind(&self, name: &str) -> ToolKind;  // Shell(Dialect) | Write | Edit | Patch | SubAgent | Other
}
pub struct HookInput {
    pub event: EventKind, pub sid: SessionId, pub cwd: PathBuf,
    pub source: Option<String>,          // SessionStart: startup|clear|resume|compact
    pub prompt: Option<String>,          // UserPromptSubmit
    pub tool: Option<ToolCall>,          // PreToolUse
    pub stop_reason: Option<String>,     // Stop: 필드 "reason"
    pub error: Option<String>,           // StopFailure: 필드 "error"의 앞 300자
}
```

읽는 입력 필드는 `hook_event_name`, `session_id`, `cwd`, `source`, `prompt`, `tool_name`, `tool_input`, `reason`, `error`다. snake_case만 읽는다. `stop_hook_active`와 `background_tasks`는 쓰지 않는다(7.6절, X-05). 환경 변수는 `ORCA_TERMINAL_HANDLE`과 `ORCA_CLI_COMMAND`를 읽는다.

| 항목 | `claude` | `codex` |
|---|---|---|
| 등록 이벤트 | SessionStart, UserPromptSubmit, PreToolUse, Stop, StopFailure | SessionStart, UserPromptSubmit, PreToolUse, Stop |
| PreToolUse 매처 | `Bash\|PowerShell\|Write\|Edit\|MultiEdit\|Agent\|Task` | `shell\|exec_command\|local_shell\|apply_patch\|spawn_agent` |
| 셸 도구 | `Bash`(Bash), `PowerShell`(PowerShell). 명령은 `tool_input.command` | `shell`·`exec_command`·`local_shell`. 방언은 OS로 정한다(Windows는 PowerShell) |
| 편집 도구 | `Write`(`file_path`, `content`), `Edit`(`file_path`, `old_string`, `new_string`, `replace_all`), `MultiEdit`(`file_path`, `edits[]`) | `apply_patch`(패치 본문이 `tool_input.command`에 온다) |
| 하위 에이전트 도구 | `Agent`, `Task`(글은 `prompt`·`description`) | `spawn_agent`(글은 `prompt`·`message`) |
| `caps` | `stop_failure = true`, `background_wake = true` | 둘 다 `false` |

판정 출력은 두 호스트에 같은 형태로 낸다. 1.24.0이 두 호스트에 같은 형태를 냈다는 것이 근거다. Codex가 각 필드를 실제로 처리하는지는 미정-M2다.

| Verdict | 출력 |
|---|---|
| `Pass` | `{}` |
| `Context(t)` (SessionStart·UserPromptSubmit) | `{"hookSpecificOutput":{"hookEventName":"<이벤트>","additionalContext":t}}` |
| `Warn(t)` | `{"systemMessage":"FullOps: " + t}` |
| `Deny` (PreToolUse) | `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"FullOps[<규칙>]: <사유> 다음: <탈출구>"}}` |
| `Block` (Stop) | `{"decision":"block","reason":"FullOps[<규칙>]: <사유> 다음: <탈출구> 함께 남은 것: <also>"}` |

사용자 문구는 한국어이고 `fullops-core::messages` 한 파일에 둔다. 모든 거부·차단 문구는 규칙 ID와 탈출구(다음에 칠 명령)를 담는다.

### 7.3 Actor 판정

`facts.actor()`가 다음 순서로 정한다. 첫 번째로 맞는 것을 쓴다.

1. 세션 파일의 `dispatch`가 있다 → `dispatch = Some(그 ID)`. 좌석은 4~6으로 정한다.
2. 환경 변수 `ORCA_TERMINAL_HANDLE`이 비어 있지 않고, `state.json`에 `terminal`이 같고 `settled == null`이며 `worktree`가 이 워크트리인 dispatch가 정확히 하나 있다 → 세션에 묶는다(`bound_by = "terminal"`). 좌석은 4~6으로 정한다.
3. 이 워크트리 루트가 `state.json`의 `active` snapshot 경로다 → 좌석 `Reviewer`.
4. `HEAD`가 detached다 → 좌석 `Other`.
5. 브랜치가 등록 역할 R의 브랜치다.
   - coor 모드: R의 kind가 `coordinator`면 `Lead`, 아니면 `Role { R, kind }`.
   - dev 모드: R이 `primary_role`이면 `Lead`, 아니면 `Role { R, kind }`.
6. 브랜치가 등록 역할의 브랜치가 아니다.
   - coor 모드: `Lead`.
   - dev 모드: 브랜치가 `primary_branch`면 `Lead`, 아니면 `Other`.

`is_lead()`는 좌석이 `Lead`이고 dispatch가 없을 때다. 브랜치는 `<git-dir>/HEAD` 파일을 읽어 얻는다. 좌석 `Other`와 `Reviewer`에는 lead 규칙과 역할 규칙을 적용하지 않는다.

2번에서 터미널 핸들이 같을 때만 묶는 이유: 사람이 역할 워크트리에서 직접 연 세션을 dispatch 받은 세션으로 오인하지 않기 위해서다.

### 7.4 도구 입력 분류

```rust
pub enum Intent {
    OrcaWorkerStart,
    OrcaTaskCreate { spec: Known<String> },
    OrcaInject,                              // "orchestration dispatch … --inject" 또는 terminal send 글에 "handovers/"
    OrcaTerminalSend { chars: usize },
    OrcaCheck { peek: bool, run: Option<String> },
    GitCommit { all: bool }, GitAdd,
    Fullops { sub: String },
    Other, Unparsed,
}
pub fn classify_shell(command: &str, dialect: Dialect, orca_names: &[String]) -> Vec<Intent>;
pub fn edit_targets(tool: &ToolCall, read: &mut dyn FnMut(&Path) -> Known<Option<String>>) -> Vec<EditTarget>;
pub struct EditTarget { pub path: PathBuf, pub before: Option<String>, pub after: Known<Option<String>> } // after None = 삭제
```

셸 분류 규칙(정규식을 쓰지 않는다):

1. here-doc 본문(`<<EOF`…`EOF`)과 PowerShell here-string(`@'`…`'@`, `@"`…`"@`)은 한 덩어리의 데이터로 둔다.
2. 따옴표(`'`, `"`)를 지키며 토큰으로 나눈다. Bash는 큰따옴표 안과 밖에서 `\`가 다음 글자를 이스케이프하고, PowerShell은 `` ` ``가 이스케이프한다.
3. `;`, `&&`, `||`, `|`, 줄바꿈에서 명령을 나눈다. `2>&1`·`>&2`·`&>` 같은 fd 복제의 `&`는 구분자가 아니다.
4. 명령 앞의 `NAME=value` 대입과 PowerShell의 호출 연산자 `&`를 뗀다.
5. 명령 이름은 첫 토큰에서 따옴표를 벗기고, `/`와 `\`를 모두 경로 구분자로 봐서 마지막 부분을 취하고, `.exe`·`.cmd`·`.bat`를 떼고, 소문자로 바꾼 것이다.
6. 첫 토큰이 `$이름`이면 같은 명령 문자열 안의 단일 리터럴 대입(`이름=값`, `$이름 = '값'`)으로 한 번 풀어 본다. 못 풀면 그 명령은 `Unparsed`다.
7. 명령 이름이 `bash`·`sh`·`zsh`·`pwsh`·`powershell`·`cmd`이고 `-c`·`-Command`·`/c` 뒤에 문자열이 오면 그 문자열을 한 단계만 다시 분류한다.
8. 명령 이름이 `orca`·`orca-dev`·`orca-ide` 또는 `ORCA_CLI_COMMAND`의 파일 이름이면 Orca 호출이다. `--help`·`-h`가 있으면 `Other`다. 둘째·셋째 토큰으로 가른다.
   - `orchestration worker-start` → `OrcaWorkerStart`
   - `orchestration task-create` → `OrcaTaskCreate`. `--spec` 값이 리터럴이면 `Known::Yes`, 변수면 6번처럼 한 번 풀고 못 풀면 `Unknown`
   - `orchestration dispatch`이고 `--inject`가 있다 → `OrcaInject`
   - `terminal send` → `--text` 값(또는 `--text=값`)의 유니코드 글자 수로 `OrcaTerminalSend`. 그 값에 `handovers/` 또는 `handovers\`가 있으면 `OrcaInject`도 함께 낸다. `--text`가 없으면 글자 수 0
   - `orchestration check` → `OrcaCheck { peek: "--peek" 유무, run: "--run" 값 }`
9. 명령 이름이 `git`이면 `-C <경로>`·`-c <설정>` 옵션을 건너뛴 뒤의 하위 명령으로 가른다. `commit` → `GitCommit { all: "-a"·"--all"·"-am" 유무 }`, `add` → `GitAdd`.
10. 명령 이름이 `fullops`면 `Fullops { sub: 둘째 토큰 }`.
11. 따옴표가 닫히지 않는 등 나누지 못하면 전체가 `Unparsed` 하나다. `Unparsed`와 `Other`에는 어떤 규칙도 걸리지 않는다.

편집 대상 계산:

- `Write`: `after = content`.
- `Edit`: 현재 파일 글에서 `old_string`의 첫 일치(또는 `replace_all`이면 전부)를 `new_string`으로 바꾼다. 일치가 없으면 `after = Unknown`.
- `MultiEdit`: `edits`를 차례로 같은 방법으로 적용한다.
- `apply_patch`: `*** Add File: <경로>`(뒤의 `+` 줄이 내용), `*** Delete File: <경로>`(`after = None`), `*** Update File: <경로>`(선택적 `*** Move to:`와 `@@` 묶음)를 읽는다. Update는 묶음의 `-`·공백 줄을 현재 글에서 찾아 바꾼다. 못 찾으면 `after = Unknown`. 패치 문법이 이와 다르면 대상 목록이 비고 통과다(미정-M2에서 실제 형식으로 확정).

PreToolUse의 규칙 후보는 다음 가운데 하나라도 있을 때 생긴다: `Other`·`Unparsed`가 아닌 Intent, 경로가 `.fullops-squad/handovers/`나 `.fullops-squad/docs/evaluations/qa-reports/` 아래인 편집, 좌석이 설계 역할일 때의 모든 편집, 하위 에이전트 도구, 정책 오류 상태. 후보가 없으면 설정 파일, `HEAD`, 세션 파일(정책 오류의 모드 변경 확인용)만 읽고 끝낸다.

### 7.5 규칙 표

```rust
pub struct Rule {
    pub id: &'static str, pub event: EventKind, pub applies: fn(&Actor) -> bool,
    pub strength: Strength,             // Context | Effect | Deny | BlockOnce | BlockAlways
    pub on_unknown: OnUnknown,          // Pass (v1의 모든 규칙)
    pub check: fn(&HookInput, &mut dyn Facts) -> Finding,   // None | Hit { fp, args } | Unknown
    pub message: MessageId, pub escape: &'static str, pub origin: &'static str,
}
pub static RULES: &[Rule];   // 표 순서가 평가 순서다
```

`fullops rules --markdown`이 이 표를 내고, 시험이 그 출력과 아래 표의 ID·이벤트·강도가 같은지 확인한다. `check`가 `Unknown`을 내면 그 규칙은 걸리지 않은 것으로 본다.

#### SessionStart (안내와 효과)

| ID | 대상 | 조건 | 결과 |
|---|---|---|---|
| S-00 | 전부 | 정책 오류(7.8절) | `Context`: 정책 오류 사유와 복구 방법. 아래 규칙은 건너뛴다 |
| S-01 | 전부, `source`가 `startup`·`clear` | 항상 | `Context`: 7.7절의 안내 |
| S-02 | 전부 | 세션 파일이 없다 | 효과: 세션 파일 생성(`anchor = HEAD`, `mode_at_start`, `terminal`) |
| S-03 | lead | 항상 | 효과: 경로와 Git 등록이 모두 사라진 `active`·`creating` snapshot을 `missing`으로 닫는다. 6.8절의 세션 정리 |
| S-04 | 전부 | `--in-place`가 아니고, 고정 위치 실행본이 없거나 `--plugin-root`의 `VERSION`이 자기 버전보다 높다 | 효과: 10.2절의 설치. 이번 호출의 판정에는 영향이 없다 |
| S-05 | 전부, `source`가 `compact` | 항상 | `Context`: 역할 한 줄과 현재 인박스 경로·과제 키 |

#### UserPromptSubmit

| ID | 대상 | 조건 | 결과 |
|---|---|---|---|
| P-00 | 전부 | 항상 | 효과: 세션의 `turn += 1`, `last_seen` 갱신 |
| P-01 | 전부 | 프롬프트에 `--dispatch-id` 뒤 공백 또는 `=`와 `[A-Za-z0-9_.:-]+`가 있고 그 값이 세션의 dispatch와 다르다 | 효과: 세션에 dispatch를 묶고(`bound_by = "preamble"`, `--task-id` 값이 있으면 함께) `blocks`를 비운다. `state.json`에 같은 ID의 dispatch가 있으면 `bound_session`을 쓴다. dev 모드면 `Context`로 역할 안내(S-01의 역할 부분)를 준다 |
| P-02 | lead | P-01의 패턴이 없다 | 효과: `turns.jsonl`에 한 줄. 프롬프트가 `You have`로 시작하고 `orchestration message`를 담으면 `machine:orca-message`, `<task-notification>`을 담으면 `machine:task-notification`, 그 밖은 `human`. 실패는 무시한다 |
| P-03 | 전부 | 정책 오류 | `Warn`(세션당 한 번) |

기계 알림 패턴 두 개는 1.24.0의 추정값이다. 실제 형식 확인은 미정-M5다.

#### PreToolUse (거부)

위에서 아래로 평가하고 먼저 걸린 거부 하나를 낸다. "모름" 열은 사실을 읽지 못했을 때의 판정이다.

| ID | 대상 | 조건 | 결과 | 모름 | 탈출구(문구에 넣는다) |
|---|---|---|---|---|---|
| T-00 | 전부 | 정책 오류다. 단 명령 이름이 `fullops`이고 하위 명령이 `doctor`·`mode`·`init`·`migrate`·`upgrade`·`status`·`why`인 단일 명령, 그리고 `.fullops-squad/fullops.toml` 편집은 통과 | Deny | - | 설정을 고치거나 새 세션을 연다. `fullops doctor` |
| T-01 | 전부 | Intent에 `OrcaWorkerStart`가 있다 | Deny | 통과 | `fullops dispatch <KEY>` |
| T-02 | 전부 | Intent에 `OrcaInject`가 있다 | Deny | 통과 | `fullops dispatch <KEY>` |
| T-03 | lead | `OrcaTerminalSend { chars > 20 }` | Deny | 통과 | 후속 지시는 `fullops dispatch <KEY> --terminal <핸들>` |
| T-04 | lead | `OrcaTaskCreate { spec }`이고, spec에 `Task key:` 뒤의 키 K가 없거나, `routes/K.json`이 없거나 `class`가 `unresolved`다. 단 spec에 `Purpose: review`가 있고 `routes/K.json`의 `tuples.reviewer`가 있으면 통과 | Deny | 통과 | `fullops route <KEY>` 뒤 spec에 `Task key: <KEY>`를 넣거나 `fullops dispatch` |
| T-05 | 전부 | 편집 대상이 `.fullops-squad/handovers/` 바로 아래 `.md`이고 `_TEMPLATE.md`가 아니며 아래 (a)~(d) 중 하나다 | Deny | 통과 | (a)(b) `fullops task new`, 다른 과제는 `fullops plan add` (c) `fullops done` (d) 같은 과제 수정은 본문만, 다른 과제는 `fullops plan add` |
| T-06 | lead, coor 모드 | 편집 대상이 있는 인박스이고 그 front matter의 `route`가 `design`·`product`이며 `from`이 `lead`가 아니다 | Deny | 통과 | 설계 역할에 넘긴다: `fullops dispatch <KEY> --role <설계 역할>` |
| T-07 | 좌석이 `Role { kind: Design }` | 편집 대상 경로가 `.fullops-squad/` 밖이고 확장자가 코드 확장자 목록에 있다 | Deny | 통과 | 지시서에 적어 구현 역할에 넘긴다 |
| T-08 | 전부 | 하위 에이전트 도구이고 아래 (e)~(g) 중 하나다 | Deny | 통과 | (e) 직접 처리한다. 레벨은 사람이 `fullops.toml`의 `subagent_level`로 바꾼다 (f) 지시에 `Purpose:`를 적는다 (g) 읽기 전용 조사·리뷰로 좁히거나 직접 처리한다 |
| T-09 | 전부 | `GitCommit`이 있고 링크 없는 QA 미디어가 커밋 대상에 있다(아래) | Deny | 통과 | 보고서 판정 항목에 링크하거나 `<과제 폴더>/raw/`로 옮긴다 |
| T-10 | 전부 | 편집이 `qa-reports/` 아래에 차수 폴더를 새로 만든다(아래) | Deny | 통과 | 기존 `qa-reports/<폴더>/report.md`에 절을 덧붙인다. 재작업은 `fullops task reopen` |
| T-11 | lead | `OrcaCheck { peek: false }` | 효과: Orca `orchestration check [--run R] --peek`(2.5초)를 불러 8.6절과 같은 방법으로 completion과 메시지를 기록한다. 판정은 통과 | 통과 | - |
| T-12 | 전부 | Intent에 `Fullops`가 있다 | 효과: `current.json` 쓰기. 판정은 통과 | 통과 | - |

T-05의 조건:

- (a) 파일 이름이 `to_<역할 문법>.md`가 아니고, 편집 결과가 비어 있지 않다(과제명 지시서 생성). 있는 과제명 파일을 지우거나 비우는 정리는 통과다.
- (b) 파일 이름이 `to_<역할>.md`이고 파일이 없다(직접 생성).
- (c) 파일이 있고 편집 결과가 삭제이거나 공백뿐이다.
- (d) 파일이 있고 편집 결과의 front matter가 없거나 `key`가 현재 값과 다르다.

T-08의 조건: 글은 도구 입력의 `prompt`·`message`·`description`을 줄바꿈으로 이은 것이다.

- (e) `subagent_level == off`.
- (f) 글에 `Purpose:` 뒤 `research`·`review`·`implementation`(대소문자 무시)이 없다.
- (g) `subagent_level == lite`이고, Purpose가 `implementation`이거나 글에 `read only`·`read-only`·`읽기 전용`(대소문자 무시)이 없다.

T-09의 계산: `git status --porcelain -z --untracked-files=all -- .fullops-squad/docs/evaluations/qa-reports`를 한 번 부른다. 커밋 대상은 인덱스 칸이 공백·`?`가 아닌 항목이고, 같은 명령에 `GitAdd`가 있거나 `GitCommit { all: true }`면 모든 항목이다. 그 가운데 확장자가 `.png`·`.jpg`·`.jpeg`·`.webp`·`.gif`·`.mp4`·`.webm`·`.mov`이고 경로에 `/raw/` 부분이 없는 파일이 미디어다. 미디어가 속한 과제 폴더(`qa-reports/` 바로 아래 폴더)의 `report.md`가 그 파일의 폴더 기준 상대 경로(슬래시 표기)를 글 안에 담고 있으면 링크된 것이다. 링크 안 된 미디어가 하나라도 있으면 걸린다. 문구에는 최대 5개를 적는다.

T-10의 계산: 편집 대상이 `.fullops-squad/docs/evaluations/qa-reports/<N>/…`이고 폴더 N이 없다. N 끝의 `-test`·`-review`를 뗀 것을 K라 한다. K에 차수 접미사 `-FIX`·`-RECHECK`·`-RETEST`·`-LOWS`·`-R<숫자>`(대소문자 무시, 뒤에 `-<숫자>`가 더 붙을 수 있고, K의 끝이거나 뒤에 `-`가 온다)가 있으면 그것을 뗀 것을 B라 한다. `qa-reports/` 아래에 `B`, `B-test`, `B-review` 가운데 하나가 있으면 걸린다.

#### Stop (차단)

공통 전제: 입력에 `reason`이 있고 `end_turn`이 아니면 전부 통과한다. X-00부터 X-06까지 모두 평가한다. 걸린 것 가운데 표 순서로 첫 `Block`의 사유를 내고 나머지 `Block`은 `also`에 ID와 한 줄 요약으로 붙인다. `Block`이 없고 `Warn`이 있으면 `Warn`들을 이어 낸다. X-07은 X-00~X-06에서 `Block`도 `Warn`도 없을 때만 평가한다.

| ID | 대상 | 조건 | 강도 | 지문 | 모름 | 탈출구 |
|---|---|---|---|---|---|---|
| X-00 | 전부 | 정책 오류 | BlockOnce | 오류 종류 | - | 설정을 고치거나 새 세션 |
| X-01 | 좌석이 `Lead`·`Role` | `changed()`가 비어 있지 않고, `pass_covers(HEAD)`가 거짓이거나 작업 트리에 대상 파일의 미커밋 변경이 있다 | dev 직접 작업은 BlockAlways, 그 밖은 BlockOnce | `HEAD` + 미커밋 대상 경로와 수정 시각 | 통과 | 커밋한 뒤 `fullops lint --from <시작점>`. 적용할 검사가 없으면 이유를 말하고 다시 끝낸다(BlockOnce일 때만) |
| X-02 | dev 직접 작업 | X-01에 안 걸렸고 `changed()`가 비어 있지 않으며 `review_need(시작점, HEAD).required`인데, `HEAD`(또는 `HEAD`와 기록 경로만 다른 조상)를 head로 하는 유효한 `accepted.json`이나 `waiver.json`이 없다 | BlockAlways | `HEAD` | 통과 | `fullops review snapshot <KEY>`로 독립 리뷰를 받거나, 사용자 결정 뒤 `fullops review waive` |
| X-03 | dispatch가 묶인 세션 | `state.json`에서 그 dispatch의 `settled`가 없고(또는 dispatch가 `state.json`에 없고), Orca 조회(`orchestration worker-show --dispatch <ID>`, 3초)로도 정산이 확인되지 않는다. 조회로 확인되면 효과로 `settled`를 쓴다 | BlockOnce | dispatch ID | 통과 | `fullops done`을 실행하고 출력된 본문으로 preamble의 `worker_done`을 보낸다. 판단이 필요하면 preamble의 `ask`, 막혔으면 `escalation` |
| X-04 | 세션의 `routed`가 있는 세션 | `routed`의 키 가운데, `state.json`에 dispatch가 없고, 이 체크아웃의 어떤 인박스의 `key`도 아니고, `PLANS.md` 과제 표의 키도 아니고, `handovers/logs/<KEY>.`로 시작하는 파일도 없는 것이 있다 | BlockOnce | 그 키 목록 | 통과 | `fullops task new`와 `fullops dispatch`로 배정하거나 `fullops plan add <KEY> --status "보류(<사유>)"` |
| X-05 | lead는 `owner == "lead"`인 열린 Run, dispatch가 묶인 세션은 `owner == "dispatch:<자기 ID>"`인 열린 Run | Run마다: (a) `acked_at`이 없는 메시지가 있다. 또는 (b) (a)가 아니고 `settled`가 없는 dispatch가 있는데 "`caps.background_wake`이고 `waiters/<run>.json`의 `heartbeat`가 90초 이내"가 아니다 | BlockOnce | Run ID + 미처리 메시지 ID + 미정산 dispatch ID + (a)/(b) | 통과 | (a) 메시지를 처리하고 `fullops wait --run <R> --ack <delivery>` (b) Claude Code는 `fullops wait --run <R>`을 백그라운드로 한 번 걸고 끝낸다. Codex는 포그라운드로 실행해 결과를 기다린다 |
| X-06 | lead | 통합 대기 completion이 있다. 평가 전에 `landed_at`이 없는 completion마다 `landing()`을 다시 계산해, `Landed`면 효과로 `landed_at`을 쓰고 hold를 `released_hold`로 옮긴다 | BlockAlways | 키와 SHA 목록 | 통과(그 건은 대기로 세지 않는다) | `fullops merge <KEY>`. 미룰 때는 `fullops hold <KEY> --reason … --owner … --resume …`. 문구에 건마다 `병합 남음`·`push 남음`·`SHA 없음`을 적는다 |
| X-07 | lead | `ready()`가 비어 있지 않다 | BlockOnce | 준비된 키 목록 | 통과 | 계속할지 묻지 말고 배정한다: `fullops task new` → `fullops dispatch`. 기다려야 하면 `fullops plan set <KEY> "보류(<사유>)"` 또는 `"대기(사람: …)"` |

X-05에서 `settled`가 없는 dispatch가 없고 미처리 메시지도 없는 Run은 효과로 닫는다(`closed_at`).

#### StopFailure (Claude Code만)

| ID | 대상 | 조건 | 결과 |
|---|---|---|---|
| F-01 | dispatch가 묶인 세션 | `state.json`에서 그 dispatch의 `settled`가 없고 `limit_escalated`가 거짓이다(dispatch가 `state.json`에 없으면 세션 파일의 `noticed`에 `f01:<ID>`가 없다) | 효과: Orca `orchestration send --type escalation --subject "[fullops:limit] worker 턴이 API 오류로 끝남" --body "<오류 앞 300자>. Task <task>, Dispatch <id>." --dispatch-id <id> [--task-id <task>]`(6초). 보낸 뒤 `limit_escalated = true`(또는 `noticed`에 기록). 정산으로 치지 않는다. 오류 종류는 가리지 않는다 |

### 7.6 한 번 막기의 정의

- `BlockOnce`: 세션 파일의 `blocks[규칙]`이 `{ fp: 이번 지문, turn: 현재 턴 }`과 같으면 `Warn`으로 통과시킨다. 아니면 `blocks[규칙]`에 그 값을 쓰고 `Block`을 낸다.
- 그래서 같은 턴 안에서 같은 상태로 다시 끝내면 통과하고, 상태(지문)가 바뀌었거나 다음 턴이 끝날 때는 다시 막는다. 1.24.0의 "두 번째 Stop은 통과, 남아 있으면 다음 턴 끝에 다시"와 같은 결과다.
- 호스트의 `stop_hook_active`에 기대지 않는다. 이유: Codex가 그 필드를 주는지 확인되지 않았고, 없으면 한 번 막기가 영구 차단이 된다. 턴 기준은 두 호스트에서 같게 동작한다. UserPromptSubmit이 오지 않는 호스트에서는 턴이 늘지 않아 첫 차단 뒤로 계속 통과한다(막는 쪽이 아니라 여는 쪽으로 틀린다).
- `blocks`를 쓰지 못하면(잠금 실패 등) 그 호출의 `BlockOnce`는 `Warn`으로 낸다.
- `BlockAlways`(X-01의 dev 직접 작업, X-02, X-06)는 지문과 무관하게 막는다. 탈출구는 표의 명령뿐이다.
- 안내와 경고의 세션당 한 번: `noticed`에 키가 있으면 내지 않는다.

### 7.7 SessionStart 안내 (S-01)

문구는 `messages`에 둔다. 담는 내용은 다음과 같다.

| 받는 쪽 | 내용 |
|---|---|
| 전부 | 역할 안내 5종 가운데 하나(dev 주 담당, coordinator, 설계 역할 또는 split의 제품 기획자, tester, worker). 좌석 `Other`·`Reviewer`는 한 줄(이 체크아웃에는 역할 규칙이 적용되지 않는다 / 읽기 전용 리뷰 사본이다) |
| 전부 | 테스트 레벨과 하위 위임 레벨의 요약. 인박스 규칙 한 문단(현재 과제는 인박스 하나, 다음 과제는 `PLANS.md` 대기, 끝나면 `fullops done`) |
| 전부 | lint 게이트: 코드를 바꾸면 커밋하고 `fullops lint --from <기준>`을 통과시킨다. 통과 기록이 없으면 종료가 한 번 막힌다(dev 직접 작업은 계속 막힌다) |
| 전부 | 현재 인박스가 있으면 그 경로와 과제 키 |
| lead | 완료 보고마다 `fullops merge`로 리뷰 판정·병합·push·동기화를 바로 처리한다 |
| lead | 항상 사람 승인인 네 가지(운영 배포, 운영 부하 테스트, backfill, 운영 데이터 정리). 되돌릴 수 있는 선택과 버그 수정 여부는 사용자에게 올리지 않는다 |
| lead | 준비된 행 최대 10개(키와 목표 앞 60자)와 "계속할지 묻지 않고 배정한다" |
| lead | `## 위임 범위`의 사용자 원문 |
| lead | 열린 결정 목록(ID, 질문, 막는 과제). 만료된 SHA 고정 결정은 다시 묻는다 |
| lead | 통합 대기·보류 건수, 예약된 동기화, 예산을 넘은 재작업(`rounds > budget`), 열린 Run ID(새 세션은 `fullops wait --run <R>`로 이어 기다린다) |
| lead | 설치 버전과 `applied_version`이 다르면 한 문장. 묻지 않는다 |
| 전부 | PATH에 `git`·`orca`가 없으면 한 문장. `state.json`을 못 읽었거나 `PLANS.md`에 문법 오류가 있으면 한 문장 |

효과로 하던 `.env*` 연결은 `dispatch`와 `task recover`가 한다(8.5절).

### 7.8 오류 처리

| 상황 | hook | CLI |
|---|---|---|
| 활성 표식 없음 | `{}` | "활성 레포가 아닙니다", 종료 코드 2 (`init`·`install`·`doctor`·`migrate` 제외) |
| stdin 해석 실패, 모르는 이벤트, 모르는 도구 | `{}`, 추적 | - |
| panic, 입출력 오류, 내부 기한 초과 | `{}`, 추적. 같은 오류 종류는 세션당 한 번 `Warn` | 종료 코드 1 |
| 잠금 실패 | 쓰기를 건너뛴다. 7.6절 | 10초 뒤 종료 코드 1 |
| git·Orca 호출 실패·시간 초과·상한 초과 | 그 사실은 `Unknown` → 규칙 통과 | 종료 코드 4. 상태를 쓰지 않거나 다시 실행해도 안전한 상태로 둔다 |
| `state.json` 파손, `v`가 더 큼 | `Unknown` → 통과. lead에게 세션당 한 번 알림 | 6.5절 |
| `PLANS.md`·인박스 문법 오류 | 그 행·파일을 판정에서 뺀다. 세션당 한 번 알림 | `plan check`가 보고 |
| **정책 오류**: `fullops.toml`이 TOML로 읽히지 않거나 6.1.1의 검증을 어긴다. 또는 세션의 `mode_at_start`가 현재 값과 다르다 | **막는다**: S-00 안내, P-03 경고, T-00 거부, X-00 한 번 차단 | 복구 명령만 받는다 |

막는 예외는 정책 오류 하나다. "hook이 돌았는가"는 `sessions/<sid>.seen/`의 표식으로 안다. `fullops why --session <sid>`가 이벤트별로 `실행됨`·`오류`·`기록 없음`을 낸다. 기록 없음은 정상이 아니라 확인 불가로 표시한다.

## 8. CLI

### 8.1 공통 규약

- 종료 코드: 0 성공, 1 내부 오류·사용법 오류, 2 규칙에 의한 거절(상태를 바꾸지 않는다), 3 사람 손이 필요함(결정을 열었거나 병합 충돌), 4 바깥 도구 실패(다시 실행해도 안전하다).
- 거절 출력: 사람용은 `FullOps[<규칙>]: <사유> 다음: <탈출구>`. `--json`이면 `{"ok":false,"refused":[{"rule","reason","escape"}]}`.
- 모든 명령은 `--repo <경로>`(기본은 cwd)와 `--json`을 받는다.
- 호출자 판정: 7.3절과 같다. 세션은 `current.json`(10분 이내)의 `sid`로 찾는다. 없으면 브랜치만으로 좌석을 정하고 dispatch는 7.3절 2번(터미널 핸들)으로만 묶는다.
- 문구는 한국어다.

### 8.2 명령 목록

| 명령 | 부르는 쪽 | 한 줄 | 1.24.0에서 대체하는 것 |
|---|---|---|---|
| `fullops hook <host>` | 호스트 | 7절 | `hook_runtime`, `flow_gate`, `done_gate`, `harness_gate` |
| `fullops route` | lead | 분류와 tuple 기록 | `jev_route.py` |
| `fullops task new\|reopen\|recover` | lead, 설계 역할 | 인박스 생성, 같은 키 재오픈, 끊긴 worker 복구 | `work.py new\|reopen\|recover` |
| `fullops plan list\|ready\|check\|add\|set` | lead | 대기 목록 읽기와 편집 | `board.py`의 준비 판정, 손 편집 |
| `fullops dispatch` | lead, 하위 Run을 가진 worker | 선행 조건 검사, 준비 커밋, 동기화, Orca 배정 | 날것 `worker-start`, hook의 배정 검사, `env_link.py` |
| `fullops wait` | lead | heartbeat 흡수, 완료 보고 기록, 처리할 메시지 반환 | `orca_wait.py` |
| `fullops done` | worker, 설계 역할, dev 주 담당 | 완료 기록 보존, 검사, push, 보낼 본문 출력 | `work.py finish` |
| `fullops merge` | lead | 리뷰 판정, 병합, lint, push, 기록, 동기화 | 스킬의 병합 절차, `review.py need`, `integration.py set-sha` 일부 |
| `fullops hold\|resume\|settle`, `fullops completion set-sha` | lead | 보류, 재개, 통합 일괄 확인, SHA 지정 | `integration.py` |
| `fullops sync` | lead | 예약된 워크트리 동기화 | 스킬의 동기화 절차 |
| `fullops review need\|snapshot\|check\|waive\|cleanup\|supersede\|prune` | lead, 리뷰어 | 8.9절 | `review.py` |
| `fullops decision add\|answer\|drop\|list` | lead, 봇 | 사용자 결정 기록 | `decision.py` |
| `fullops lint` | worker, lead | 8.11절 | `lint.py` |
| `fullops doc check\|stamp` | 산출물을 고치는 역할 | 8.12절 | `deliverables.py` |
| `fullops status`, `fullops why`, `fullops rules` | 사람, lead, 패널 | 요약, hook 추적, 규칙 표 | `flow_state.py`, `integration.py status`, `done_gate status`, `board.py` 지표 |
| `fullops init`, `fullops mode dev`, `fullops upgrade`, `fullops migrate` | 사람, setup 스킬 | 10·13절 | `setup.py`, `update.py` |
| `fullops install`, `fullops doctor`, `fullops env link`, `fullops gc` | 사람, setup 스킬 | 10절 | `deps.py`, `hook_runtime.py`, `env_link.py` |

### 8.3 `route`

```
fullops route <KEY> (--request-file <경로> | --request "<글>")
        [--class direct|design|product|unresolved --role <역할> --reason "<근거>"]
        [--agent <CLI> --model <모델> --effort <effort> --why "<이유>"]
        [--origin user|bot] [--deliverable <Dnn>]... [--force] [--distinct]
fullops route <KEY> --tuple-only --role <역할|reviewer> --agent … --model … --effort … [--why …]
```

1. 호출자는 lead다. 아니면 거절(R-01).
2. KEY가 문법에 안 맞으면 거절. KEY에 T-10의 차수 접미사가 있고 접미사를 뗀 키의 route가 있으면 `--distinct` 없이는 거절하고 `task reopen`을 안내한다(R-02).
3. 요청 글이 4,000자를 넘으면 거절하고 요약을 요구한다(R-03).
4. `routes/<KEY>.json`이 있고 `--force`가 없으면 기존 기록을 출력하고 종료 코드 0으로 끝낸다. 호출하지 않는다.
5. `--class`가 있으면 `decided_by = "manual"`. `unresolved`가 아니면 `--role`이 필수다. 기존 기록의 역할을 바꾸면 `--reason`이 필수다(R-04).
6. `--class`가 없으면 분류한다. `FULLOPS_OFFLINE=1`, `FULLOPS_JEV_ENABLED=0`, 키 없음, 호출 실패, 응답 형식 오류는 폴백이다.
   - 질문: 범위(Choice: legacy는 `simple`·`design`, split은 `implementation`·`product`), 담당 역할(Choice: kind가 `worker`·`tester`인 역할, 설명은 `about`), 산출물마다 예/아니오(인덱스의 제목과 요약).
   - legacy: `p(simple) ≥ 0.8`이고 `max p(역할) ≥ 0.6`이면 `direct` → 그 역할. 아니면 `design` → 설계 역할(없으면 `unresolved`).
   - split: `p(implementation) ≥ 0.8`이고 `max p(역할) ≥ 0.6`이면 `direct`. `p(product) ≥ 0.8`이면 `product` → 설계(제품 기획) 역할. 그 밖은 `unresolved`.
   - 폴백: legacy는 `design`, split은 `unresolved`. `decided_by = "fallback"`이고 stderr에 한 줄 경고를 낸다.
   - 산출물: 확률 0.5 이상을 높은 순으로 최대 3개.
   - 임계값은 1.24.0과 같게 둔다(결과 비교 때문).
7. tuple 플래그는 셋 다 주거나 다 안 준다(R-05). 주면 `tuples[<역할>]`에 기록한다.
8. `--tuple-only`는 호출 없이 `tuples`만 쓴다. 기록이 없으면 `class = "untracked"`로 만든다.
9. 파일을 쓰고(커밋하지 않는다) 세션 파일의 `routed`에 키를 더한다.

분류는 추천이다. 지시서를 쓰다가 파일 소유권·완료 기준·검증 명령 가운데 하나라도 못 정하면 lead가 `--force --class design`으로 바꾼다(스킬 문서).

### 8.4 `task`

```
fullops task new    --role <R> --key <KEY> --goal "<한 줄>" [--base <ref>]
fullops task reopen --role <R> --key <KEY> --goal "<수정 목표>" [--base <ref>]
fullops task recover --role <R> --key <KEY>
```

`task new`의 거절 조건:

| ID | 조건 |
|---|---|
| H-01 | `routes/<KEY>.json`이 없거나 `class`가 `unresolved`·`untracked`다. 단 dev 주 담당이 자기 역할(`primary_role`)의 인박스를 만드는 경우는 route가 없어도 된다 |
| H-02 | 작성 권한이 없다. 허용: (1) lead이고 `class == direct`이며 `role == R`. (2) 호출자의 좌석이 kind `design`인 역할이고 `class`가 `design`·`product`다(R은 어느 역할이든). (3) dev 주 담당의 자기 인박스. (4) 레포에 kind `design`인 역할이 없으면 lead는 어느 class든 쓸 수 있다 |
| H-03 | `handovers/to_<R>.md`가 이 체크아웃에 있다. 탈출구: 같은 과제면 본문을 고치고, 다른 과제면 `fullops plan add` |
| H-04 | `handovers/logs/<KEY>.<R>.`로 시작하는 기록이 있다. 탈출구: `fullops task reopen` |

통과하면 내장 템플릿으로 인박스를 만든다. `base`는 `--base`를 40자로 푼 값이고 기본은 `refs/heads/<base 브랜치>`의 tip이다. `from`은 lead면 `lead`, 아니면 호출자의 역할 ID다. `test_level`·`subagent_level`은 현재 설정 값이다. 커밋하지 않는다(`dispatch`의 준비 커밋에 들어간다).

`task reopen`: H-01·H-02를 같게 검사하고, 인박스가 없어야 하며, `handovers/logs/<KEY>.<R>.r<n>.md`가 있어야 한다. `round`는 가장 큰 n + 1이다. `state.json`의 `tasks[KEY].rounds`를 1 올린다. `rounds > budget`이면 "같은 과제의 수정·재검증이 예산(<budget>)을 넘었다. 사용자에게 올려라"를 출력한다(거절이 아니다). 반복 예산은 인박스 front matter의 `budget`(기본 2)이다.

`task recover`(역할 워크트리에서 lead가 실행): 미커밋 변경이 있으면 `wip(<R>): <KEY>` 커밋으로 보존한다. `refs/heads/<base>`를 merge한다. 충돌하면 `git merge --abort` 뒤 WIP 커밋을 남기고 종료 코드 3. `.env*`를 연결한다. WIP SHA를 출력한다.

### 8.5 `dispatch`

```
fullops dispatch <KEY> [--role <R>] [--purpose build|fix|review|research] [--run <ID>]
        [--terminal <핸들>] [--agent <CLI> --model <모델> --effort <effort>] [--why "<이유>"]
        [--sha <40자>] [--reason limit] [--task <Orca task id>] [--dry-run]
```

기본값: `--purpose build`. `--role`은 route의 `role`(class가 `design`·`product`이고 인박스가 없으면 설계 역할). tuple은 플래그 → `routes/<KEY>.json`의 `tuples[<역할 또는 reviewer>]` 순이다.

거절 조건(D-01~D-10은 바깥 호출 전에 모두 검사해 걸린 것을 한꺼번에 보고한다. D-11은 4단계 뒤에 검사한다):

| ID | 조건 | 예외 | 탈출구 |
|---|---|---|---|
| D-01 | route가 없거나 `class`가 `unresolved`다 | `--purpose review`는 `tuples.reviewer`가 있으면 통과 | `fullops route` |
| D-02 | Build·Fix인데 lead 체크아웃의 `handovers/to_<R>.md`가 없거나 `key`가 다르다 | 대상이 설계 역할이고 class가 `design`·`product`인 Build는 인박스가 없어도 된다(spec에 요청 파일 경로를 넣는다). Review·Research는 인박스가 필요 없다 | `fullops task new` |
| D-03 | 다른 키의 통합 대기 completion이 있다 | 같은 키의 Fix·Review | `fullops merge <그 키>` 또는 `fullops hold` |
| D-04 | 호출자가 lead가 아니고 dispatch가 묶인 세션도 아니다. 또는 dispatch가 묶인 세션이 자기 소유가 아닌 Run을 `--run`으로 줬다 | - | `--run`을 빼면 자기 Run을 만든다 |
| D-05 | R이 등록 역할이 아니거나, R의 브랜치를 체크아웃한 워크트리가 없거나, class가 `direct`인데 R이 route의 역할과 다르다 | Review의 대상은 그 리뷰의 `active` snapshot이다 | Orca로 역할 워크트리를 만든다(bootstrap) |
| D-06 | Review인데 `--sha`가 40자가 아니거나, snapshot의 `HEAD`가 `--sha`와 다르거나, snapshot이 detached가 아니거나 깨끗하지 않거나, `--terminal`이 구현 dispatch의 터미널과 같다 | - | `fullops review snapshot <KEY>` |
| D-07 | `--reason limit`인데 `tasks[KEY].limit_redispatch >= 2`이거나 tuple이 `last_tuple`과 다르다 | - | 횟수 초과면 결정을 자동으로 열고(`"<KEY> 한도 재배정 2회 초과"`, blocks `[KEY]`) 종료 코드 3. tuple이 다르면 종료 코드 2 |
| D-08 | 선택형 위임인데 레벨이 허용하지 않는다(아래) | lead의 필수 독립 리뷰(`--purpose review`와 `--sha`) | 레벨을 올리거나 직접 처리 |
| D-09 | tuple을 정할 수 없다 | - | `--agent --model --effort` |
| D-10 | R에 `settled`가 없는 다른 키의 dispatch가 있다 | 같은 키 | 그 과제가 끝난 뒤 배정하거나 `fullops plan add`로 대기 |
| D-11 | 아래 4단계의 동기화 뒤에도 대상 브랜치 tip이 `refs/heads/<base>`와 통합 기준 ref를 조상으로 가지지 않는다 | Review | `fullops sync --role <R>`. 원격이 앞서 있으면 `fullops settle`로 fetch한 뒤 다시 |

D-08: 선택형 위임은 (1) 호출자가 dispatch 받은 세션이거나 (2) 호출자가 dev 모드 lead이거나 (3) 목적이 Research이거나 필수 리뷰가 아닌 Review인 배정이다. `off`면 거절. `lite`면 Review·Research만 허용하고 spec에 `Read only: true`를 넣는다. `standard`·`full`은 통과다.

통과 뒤의 순서:

1. **Run 정하기.** `--run`이 있으면 그것. 없으면 호출자가 소유한 열린 Run이 하나면 그것, 없으면 Orca `run-create`로 만든다. dispatch 받은 세션은 항상 자기 소유(`owner = "dispatch:<ID>"`) Run을 쓴다. 상위 Run의 ID를 `--run`으로 주면 거절한다(D-04).
2. **준비 커밋.** lead 체크아웃에서 `.fullops-squad/routes/<KEY>.json`, `.fullops-squad/handovers/to_<R>.md`, `.fullops-squad/PLANS.md` 가운데 바뀐 것만 골라 `git commit --only`로 커밋 하나를 만든다(메시지 `fullops: 배정 준비 <KEY>`). tuple 기록은 이 커밋에 들어가도록 먼저 쓴다. 바뀐 것이 없으면 건너뛴다.
3. **기록 게시**(`publish_records`). lead의 브랜치가 기본 브랜치가 아니면 lead의 `HEAD`가 `refs/heads/<base>`를 조상으로 가져야 한다(아니면 종료 코드 2, 탈출구 `fullops sync --self`). 기본 브랜치가 다른 워크트리에 체크아웃돼 있고 깨끗하면 거기서 `git merge --ff-only <lead HEAD>`를 하고, 체크아웃돼 있지 않으면 `git update-ref refs/heads/<base> <lead HEAD> <이전 값>`을 한다. 그 워크트리가 더러우면 종료 코드 2. 원격이 있으면 `git push <remote> <base>`를 한다. 실패하면 종료 코드 4.
4. **대상 워크트리 동기화.** 대상 워크트리가 깨끗하면 `git merge --ff-only <base tip>`, ff가 아니면 `git merge --no-edit <base tip>`. 충돌하면 `git merge --abort` 뒤 `sync_pending[R]`에 기록하고 종료 코드 3. 대상이 더러우면 `sync_pending[R]`에 기록하고 종료 코드 2(탈출구: 그 worker의 완료 뒤 `fullops sync --role <R>`). 성공하면 `sync_pending[R]`을 지운다. Review는 이 단계를 건너뛴다.
5. **기준선 확인.** D-11을 검사한다. 걸리면 종료 코드 2.
6. **환경 파일 연결.** 원본 체크아웃(기본 브랜치 워크트리)의 미추적 `.fullops-squad/.env*` 가운데 대상에 없는 것을 심볼릭 링크로 만들고, 안 되면 복사한다. 추적 파일과 이미 있는 파일은 건드리지 않는다.
7. **지시서 알림.** 인박스 본문의 lint 명령 줄에 기준 `test_level`보다 높은 `--level`이 있으면 경고 한 줄을 낸다(1.24.0의 PostToolUse 알림을 여기로 옮겼다).
8. **Orca 배정.** `orchestration worker-start --run <R> --spec "<spec>" --worktree <경로> --agent <CLI> [--model <모델>] [--effort <effort>] [--terminal <핸들>] [--task <id>] --timeout-ms <ready_timeout_seconds × 1000> --json`. 실패하면 종료 코드 4이고 상태를 쓰지 않는다. 다른 CLI·호스트로 넘어가지 않는다. 인자 이름과 결과 필드는 미정-M3다.
9. **상태 쓰기.** Run(없으면 추가), dispatch, `tasks[KEY].last_tuple`, `--reason limit`이면 `limit_redispatch += 1`. Orca 결과에 dispatch ID가 없으면 `id`를 `"pending:<run>:<started_at>"`으로 쓰고 P-01·7.3절 2번이 묶일 때 실제 값으로 바꾼다.
10. run id, task id, dispatch id, worker 핸들과 "다음: `fullops wait --run <R>`"를 출력한다.

다시 실행해도 안전하다. 2~6은 이미 된 일을 건너뛴다.

spec 문자열(한 문단, 이 순서):

```
Task key: <KEY>; Purpose: <implementation|fix|review|research>; Role: <R>;
Handover: <대상 워크트리의 인박스 절대 경로>; Base: <40자>; Test level: <L>; Subagent level: <S>
[; Request: <요청 파일 경로>]                                   (설계 역할 배정)
[; Review SHA: <40자>; Implementer session: <ID>; Reviewer session: separate;
   Review folder: <lead 체크아웃의 reviews/<KEY>/<head 12자> 절대 경로>; Read only: true]   (리뷰)
[; Read only: true]                                              (조사)
. 끝나면 fullops done을 실행하고 출력된 본문으로 preamble의 worker_done을 한 번 보낸다.
```

사고 지시는 넣지 않는다.

### 8.6 `wait`

```
fullops wait [--run <ID>] [--ack <delivery id>]... [--max-minutes <N>]
```

1. `--run`이 없으면 호출자가 소유한 열린 Run이 하나일 때 그것을 쓴다. 여럿이면 거절하고 목록을 낸다.
2. `--ack` 값마다 `state.json`의 메시지에 `acked_at`을 쓰고 Orca에 ack한다.
3. `waiters/<run>.json`에 `{"pid","sid","heartbeat"}`를 쓰고 30초마다 `heartbeat`를 갱신한다. 끝날 때 지운다.
4. Orca `orchestration check --run <R> --json`을 기다림 방식으로 부른다(인자는 미정-M3). "no longer bound" 오류면 `orchestration run-use --id <R>` 뒤 한 번 다시 한다.
5. 받은 메시지를 종류로 가른다.
   - `heartbeat`·`status`: 세고 ack한다. 모델을 깨우지 않는다.
   - `worker_done`: 아래 completion 규칙으로 기록한 뒤 처리할 메시지로 모은다.
   - `question`: 처리할 메시지로 모은다.
   - `escalation`: dispatch의 `settled = { how: "escalated" }`를 쓰고, 제목이 `[fullops:limit]`으로 시작하면 `limit_escalated = true`를 쓴다. 처리할 메시지로 모은다.
6. 처리할 메시지가 하나라도 있으면 ack하지 않고 즉시 반환한다.
7. `--max-minutes`(기본 `wait.check_minutes`)가 지나면 Orca `worker-list --run <R>`로 생존을 확인한다. `settled`가 없는데 liveness가 `exited`인 dispatch는 `settled = { how: "lost" }`로 쓰고 `lost`에 담는다. `idle_timeout: true`로 반환한다.
8. Orca 호출이 실패하면 30초 뒤 다시 한다. 세 번 연속 실패하면 종료 코드 4.

**completion 기록 규칙**(T-11의 미리 보기도 같다). `state.json`에 쓴 뒤에만 ack할 수 있다.

1. 메시지의 `payload.dispatchId`(없으면 `payload.taskId`)로 `state.json`의 dispatch를 찾는다.
2. dispatch의 `settled = { how: "done", outcome, message }`를 쓴다. `outcome`은 Orca 메시지의 값이고 없으면 `succeeded`다.
3. dispatch의 목적이 Review·Research면 completion을 만들지 않는다.
4. 과제 키: dispatch의 `key`. dispatch를 못 찾았으면 본문의 `[완료]` 또는 `[설계]` 뒤 첫 토큰이 `TaskKey` 문법에 맞을 때 그것. 그래도 없으면 `key = null`로 기록하고 lead에게 `completion set-sha --key`를 안내한다.
5. SHA는 다음 순서로 정한다.
   - dispatch의 `reported.sha`(`fullops done`이 쓴 값) → `sha_source = "done"`.
   - 본문에서 `SHA`·`commit`·`커밋` 뒤(사이에 `:`·`=`·공백 허용)의 7~40자 hex를 모두 모아 `git rev-parse --verify <hex>^{commit}`으로 푼다. 하나면 그것. 여럿이면 나머지를 모두 조상으로 가지는 하나 → `sha_source = "body"`.
   - 역할 브랜치의 로컬 ref(`refs/heads/<역할 브랜치>`) tip → `sha_source = "branch_tip"`. lead에게 "보고에 SHA가 없어 브랜치 tip을 썼다"를 알린다. 로컬 ref를 쓰는 이유: 같은 Git 공용 디렉터리라 worker의 커밋이 push 없이도 보이고, 원격 tip보다 보고 시점에 가깝다.
   - 그래도 없으면 `sha = null`.
6. `outcome == failed`면 completion은 만들되 통합 대기로 세지 않는다.
7. 형식 알림(차단이 아니다): 본문에 브랜치가 없거나, `reported.lint_head`가 `sha`를 덮지 못하면 반환값의 `notes`에 적는다.

반환(JSON):

```json
{ "run": "run_ab12", "idle_timeout": false, "check_minutes": 60,
  "actionable": [ { "delivery": "dlv_8", "id": "msg_3", "type": "worker_done", "key": "PAY-12", "role": "dev",
                    "sha": "…", "sha_source": "done", "outcome": "succeeded", "body": "<본문>", "notes": [] } ],
  "absorbed": { "heartbeats": 11, "status": 2 },
  "lost": [ { "dispatch": "dsp_02", "key": "PAY-13", "role": "art" } ],
  "next": "worker_done은 fullops merge <KEY>, 처리 뒤 fullops wait --run run_ab12 --ack dlv_8" }
```

본문은 출력에만 있고 저장하지 않는다.

### 8.7 `done`

```
fullops done [--failed "<사유>"] [--followup "<역할>: <글>"]... [--no-checks "<사유>"]
```

worker·설계 역할·dev 주 담당이 자기 워크트리에서 실행한다. 보내지는 않는다. Orca에 보내는 것은 preamble의 `worker_done` 명령이다(이유: preamble이 `--from`·capability·task·dispatch 값을 가진 정본이고, 그 형식을 `fullops`가 흉내 내면 Orca가 바뀔 때 깨진다).

| 단계 | 내용 | 실패 |
|---|---|---|
| R-01 | 호출자의 좌석이 `Role` 또는 dev 직접 작업의 `Lead`다 | 거절 |
| R-02 | 작업 트리가 깨끗하다(미추적 포함, 무시 파일 제외) | 거절. 탈출구: 커밋 |
| R-03 | `changed()`가 비어 있지 않으면 `pass_covers(HEAD)`가 참이다 | 거절. 탈출구: `fullops lint --from <시작점>`. 적용할 검사가 없으면 `--no-checks "<사유>"`(사유는 완료 기록 커밋 본문에 남는다) |
| R-04 | 자기 인박스가 있으면 `## 완료 보고` 절에 템플릿 안내 문장이 아닌 줄이 하나 이상 있다 | 거절 |
| R-05 | 알림(거절이 아니다): `test_level`이 minimal·lite인데 그 과제의 `qa-reports/<폴더>/report.md`가 40줄을 넘는다(리뷰 보고서 제외). 보고서가 `raw/`를 링크한다 | - |
| R-06 | 자기 인박스가 있으면 `git mv handovers/to_<R>.md handovers/logs/<KEY>.<R>.r<round>.md` 뒤 커밋 하나(`fullops: 완료 기록 <KEY> (<R>)`) | 종료 코드 1 |
| R-07 | 원격이 있으면 `git push <remote> HEAD:refs/heads/<역할 브랜치>`. force는 쓰지 않는다 | 종료 코드 4. 다시 실행하면 R-06을 건너뛴다 |
| R-08 | `state.json`: dispatch가 묶여 있으면 `reported = { branch, sha: HEAD, lint_head, followups, at }`. dev 직접 작업이면 `completions`에 `direct: true`인 기록을 더한다 | 종료 코드 1 |
| R-09 | dispatch가 묶여 있으면 보낼 본문을 출력한다 | - |

출력 본문: `[완료] <KEY> | 브랜치 <브랜치> | SHA <HEAD 40자> | 검증: lint <head 7자> ERROR 0 WARNING <n> | 후속: <followup 요약 또는 없음>`. 설계 역할은 머리가 `[설계]`이고 `| 지시서: <이번 과제에서 만든 인박스 경로들>`이 붙는다. `--failed`면 `[실패] <KEY> | 사유: …`이고 preamble 명령에 `--outcome failed`를 쓰라고 안내한다. 이미 R-06~R-08을 마친 상태에서 다시 실행하면 본문만 다시 낸다.

후속 요청은 `--followup` 플래그로만 받는다(완료 보고 산문에서 뽑지 않는다). worker 브랜치의 `PLANS.md`는 고치지 않는다. `merge`가 병합 기록 커밋에서 `## 완료 보고 후속 요청`에 옮긴다.

### 8.8 `merge`와 통합 명령

```
fullops merge <KEY> [--completion <메시지 ID>] [--accept-missing-lint "<사유>"] [--no-push] [--continue | --abort]
fullops hold <KEY> --reason "<사유>" --owner "<담당>" --resume "<재개 조건>" [--completion <ID>]
fullops resume <KEY> [--completion <ID>]
fullops settle
fullops completion set-sha <메시지 ID> <SHA> [--key <KEY>]
fullops sync [--role <R>] [--self]
```

`merge`의 순서:

| 단계 | 내용 | 실패 |
|---|---|---|
| M-01 | 호출자는 lead다 | 거절 |
| M-02 | KEY의 통합 대기 completion이 하나이고 `sha`가 있다. 여럿이면 `--completion` | 거절. SHA가 없으면 `completion set-sha` |
| M-03 | 기본 브랜치를 체크아웃한 워크트리(기본 워크트리)가 있고 깨끗하다. 원격이 있으면 `git fetch <remote> <base>` 뒤 `git merge --ff-only <통합 기준 ref>`. `FULLOPS_OFFLINE=1`이면 fetch를 건너뛰고 그 사실을 출력한다 | 워크트리가 없거나 더러우면 종료 코드 2. fetch·ff 실패는 종료 코드 4 |
| M-04 | 리뷰 판정: `need = review_need(merge-base(base, sha), sha)`. 불필요면 통과. 필요면 아래 표 | 아래 표 |
| M-05 | lint 증거: `reported.lint_head`가 `sha`를 덮는다(같거나, 조상이고 차이가 기록 경로뿐). 없으면 worker 워크트리의 `lint/pass.json`으로 같은 확인을 한다. 그 과제의 `changed`가 비어 있으면(문서만) 통과 | 거절. 탈출구: worker 워크트리에서 `fullops lint`, 또는 `--accept-missing-lint "<사유>"` |
| M-06 | 기본 워크트리에서 `git merge --no-edit <sha>`(fast-forward 허용, squash·cherry-pick 금지) | 충돌이면 병합을 진행 중 상태로 두고 종료 코드 3. 해결 뒤 `--continue`, 포기는 `--abort`(`git merge --abort`) |
| M-07 | 병합 결과의 tree가 `sha`의 tree와 다르면 기본 워크트리에서 `fullops lint --from <병합 전 base tip>`을 한 번 돌린다. 같으면 worker lint를 재사용한다. LLM 리뷰는 더하지 않는다 | 실패하면 `git reset --keep <병합 전 tip>` 뒤 종료 코드 2. 탈출구: 수정 배정(`fullops task reopen`) 또는 `fullops hold` |
| M-08 | 병합 기록 커밋 하나(`fullops: 병합 기록 <KEY>`): `PLANS.md`에서 그 키의 행 상태를 `완료(<sha 7자>)`로 바꾸고(행이 있을 때), `## 배정 기록`에 그 과제의 dispatch 줄을 덧붙이고, `reported.followups`를 `## 완료 보고 후속 요청`에 덧붙이고, `reviews/<KEY>/…`의 미커밋 파일을 넣는다. 커밋 본문에 리뷰 판정 사유(`need.reason`)와 lint 재사용 여부를 적는다 | 종료 코드 1 |
| M-09 | 원격이 있고 `--no-push`가 아니면 `git push <remote> <base>`. force는 쓰지 않는다 | 종료 코드 4. completion은 통합 대기로 남는다. 다시 실행하면 M-06~M-08을 건너뛰고 push만 한다 |
| M-10 | `landing(sha) == Landed`를 확인하고 `landed_at`을 쓴다. hold가 있었으면 `released_hold`로 옮긴다. 과제에 열린 dispatch가 없으면 `archive.jsonl`로 옮긴다 | - |
| M-11 | 동기화: 등록 역할의 워크트리마다(lead의 coordinator 브랜치 포함) `settled` 없는 dispatch가 없고 깨끗하면 `git merge --ff-only <base>`, ff가 아니면 `git merge --no-edit <base>`(충돌이면 abort). 그 밖은 `sync_pending[R] = <base tip>`. 강제 reset과 삭제는 쓰지 않는다 | 예약만 남기고 계속 |
| M-12 | 그 과제의 `active` snapshot이 있으면 8.9절의 `cleanup --finished`를 한다 | 정리하지 못하면 snapshot에 hold 사유를 적고 계속 |
| M-13 | 결과 요약과 "다음: `fullops wait --run <R> --ack <delivery>`", 준비된 행 목록을 출력한다 | - |

M-04에서 `need.required`일 때:

| 상태 | 결과 |
|---|---|
| `reviews/<KEY>/<sha 12자>/accepted.json`이 유효하다(8.9절) | 통과 |
| 그 base·head의 `waiver.json`이 있다 | 통과 |
| `result.json`이 있고 결론이 `reject`이거나 미해결 critical·high가 있다 | 거절. 탈출구: 수정 배정 뒤 새 SHA로 다시 |
| 리뷰 기록이 없고 `review.auto`가 참이다 | 거절. 탈출구: `fullops review snapshot <KEY>` |
| 리뷰 기록이 없고 `review.auto`가 거짓이며 그 SHA에 묶인 결정이 없다 | 결정을 연다(질문: 판정 사유·예상 비용·선택지 `진행`/`생략`, `sha` 고정, blocks `[KEY]`). 종료 코드 3 |
| 그 SHA의 결정이 열려 있다 | 종료 코드 3. 결정 ID를 다시 출력한다 |
| 그 SHA의 결정 답이 `생략`이다 | 거절. 탈출구: `fullops review waive <KEY> --decision <ID> --reason …` |
| 그 SHA의 결정 답이 `진행`이다 | 거절. 탈출구: `fullops review snapshot <KEY>` |

`hold`: 통합 대기 completion에 세 값을 모두 받아 쓴다. 보류는 병합 완료가 아니다. `resume`: hold를 `released_hold`로 옮긴다(사유는 지우지 않는다). `settle`: 원격이 있으면 fetch한 뒤 모든 미통합 completion의 `landing()`을 다시 계산해 통합된 것을 닫는다. 건수 상한이 없다. `completion set-sha`: SHA를 40자로 풀어 쓰고 `sha_source = "manual"`. `sync`: `sync_pending`의 역할마다 M-11과 같은 일을 한다. `--self`는 lead의 브랜치에 기본 브랜치를 반영한다.

날것 `git merge`·`git push`로 통합해도 X-06과 `settle`이 조상 관계로 인식한다. 그때 기본 브랜치가 보고 SHA 뒤의 역할 브랜치 커밋(기록 경로만 바꾼 커밋 제외)을 함께 담았으면 `drift`에 수를 쓰고 lead의 다음 Stop·`status`에서 한 번 알린다(1.24.0의 병합 SHA 어긋남 알림).

### 8.9 `review`

```
fullops review need <KEY> | --from <ref> --to <ref>
fullops review snapshot <KEY> [--sha <SHA>] [--implementer-session <ID>]
fullops review check <KEY> [--sha <SHA>]
fullops review waive <KEY> --decision <ID> --reason "<사용자 지시 요지>"
fullops review cleanup <KEY> [--sha <SHA>] (--finished | --abandon --reason "<사유>")
fullops review supersede <KEY> --old <head 12자> --new <head 12자>
fullops review prune
```

**필요 판정** `review_need(base, head)`: `mb = merge-base(base, head)`. 정책은 `mb` 시점의 `[review]`와 `[code]`, lint 제외는 `mb` 시점의 `lint.json`이다. `git diff --numstat mb head`에서 코드 파일(8.11절)의 추가+삭제 줄 합이 `lines`, 파일 수가 `files`다. `always` glob에 맞는 바뀐 경로가 `forced`다. `required = forced가 있음 || lines >= min_code_lines || files >= min_code_files`. `action`은 필요 없으면 `skip`, 필요하고 `auto`면 `review`, 아니면 `ask`다.

**snapshot**: SHA 기본값은 그 과제의 통합 대기 completion의 `sha`(dev 직접 작업은 `HEAD`). `state.json`에 `creating`으로 쓴 뒤 `git worktree add --detach <레포 부모>/.fullops-review-<무작위 8자> <sha>`를 하고 `active`로 바꾼다. 같은 리뷰가 `creating`으로 남았고 경로와 Git 등록이 모두 없으면 다시 만든다. `reviews/<KEY>/<head 12자>/`에 다음을 쓴다(이미 있으면 덮어쓰지 않고 거절).

- `targets.json`: `{"v":1,"key","base","merge_base","head","policy_sha256","files":[{"path","status":"A|M|D|R","rule":"<문장 또는 null>","preset":"skipped|null","reason"}]}`. 대상은 `git diff --name-status mb head`의 모든 파일이다. `review/rule.json`의 `exclude`에 맞거나 `qa-reports/`의 미디어·로그, `handovers/logs/**`, `routes/**`, `*.bak`이면 `preset = "skipped"`와 고정 사유를 넣는다. `rule`은 `rule.json`의 `rules`에서 `path` glob이 처음 맞는 문장이고 없으면 `null`이다(그때는 스킬 문서의 세 관점 정책만 적용한다). OCR CLI를 쓰지 않는다.
- `result.json` 뼈대: `{"v":1,"key","base","head","reviewer_session":null,"implementer_session","snapshot_path","snapshot_head","read_only":true,"conclusion":null,"files":[{"path","status","review_status":null,"reason":""}],"findings":[]}`. `findings[]`의 필드는 `path`, `content`, `severity`(`critical`·`high`·`medium`·`low`), `resolved`, `start_line`, `end_line`이다.
- `report.md` 틀과, 찾을 수 있으면 `lint.json`(같은 base·head의 `lint/result.json` 사본. 등록된 모든 워크트리의 Git 디렉터리에서 찾는다).

리뷰어는 snapshot만 읽고 결과는 lead 체크아웃의 리뷰 폴더(spec의 `Review folder`)에 쓴다. lint·테스트를 다시 돌리지 않는다. 끝나면 preamble의 `worker_done`을 보낸다(본문 `[리뷰] <KEY> | SHA <head> | 결론 accept|reject | high <n>`).

**check**가 실패하는 경우(하나라도):

1. `targets.json`의 `base`·`head`가 현재 completion의 것과 다르다.
2. `result.json`의 `files`가 `targets.json`의 `(path, status)` 집합과 다르다(누락·추가).
3. `review_status`가 `reviewed`·`skipped`가 아니거나 `skipped`인데 `reason`이 비었다.
4. `reviewer_session`이 비었거나 `implementer_session`과 같다. `read_only`가 참이 아니다.
5. snapshot이 `active`인데 `HEAD`가 `head`와 다르거나 detached가 아니거나 깨끗하지 않다. snapshot이 이미 `removed`면 이 항목은 건너뛴다.
6. `conclusion`이 `accept`·`reject`가 아니다.
7. `resolved`가 거짓인 `critical`·`high` finding이 있다.
8. `review/rule.json`의 sha256이 `targets.json`의 `policy_sha256`과 다르다.
9. `lint.json`이 없거나, 그 `head`가 리뷰 `head`를 덮지 못하거나, `config_sha256`이 기준 시점 설정과 다르거나, ERROR가 있거나, `unrunnable` 명령의 `reason`이 비었거나, 기준 시점 설정에 등록된 명령의 결과가 없거나 중복이다.

통과하고 결론이 `accept`면 `accepted.json`을 쓴다: `{"v":1,"key","base","head","result_sha256","lint_sha256","reviewer_session","implementer_session","at"}`. `accepted.json`이 유효하다는 것은 두 해시가 현재 파일과 같다는 뜻이다. check 통과는 기록 검사이고 수락 판단은 검토자와 병합 책임자의 몫이다.

**waive**: 결정이 `answered`이고 `sha`가 리뷰할 head와 같아야 한다. 아니면 거절한다. `waiver.json`(`{"v":1,"key","base","head","reason","decision","decided_by":"user","need":{…},"at"}`)을 쓴다. 이미 있으면 덮어쓰지 않는다. 진행 중이던 snapshot이 있으면 `--abandon`으로 닫는다. 면제는 수락이 아니다.

**cleanup**: Orca `terminal list --worktree path:<snapshot>`(10초)으로 살아 있는 터미널이 없는지 확인한다(없어질 때까지 최대 8초 다시 확인). 확인하지 못했거나 snapshot에 미커밋·미추적·무시 파일이 있으면 지우지 않고 snapshot의 `hold`에 사유를 쓴다. 통과하면 `git worktree remove <경로>`(force 없음)를 하고 `removed`(또는 `--abandon`이면 `abandoned`와 `abandoned.json`)로 바꾼다.

**supersede**: 새 head가 옛 head의 후손이고 새 리뷰의 `accepted.json`이 유효할 때 옛 폴더에 `superseded.json`(`{"by","old_result_sha256","new_result_sha256","at"}`)을 쓴다. 옛 `result.json`은 고치지 않는다. 실패한 리뷰를 후속 수락에 연결하는 회복 경로다.

**prune**: S-03과 같은 일을 하고 남은 `active` snapshot 목록을 낸다. 있는 워크트리는 지우지 않는다.

리뷰 정책(세 관점, 심각도 처분, nit 5건)은 스킬 문서의 규약이고 `check`가 판정하지 않는다.

### 8.10 `decision`

```
fullops decision add --question "<질문>" [--key <KEY>] [--option <선택지>]... [--blocks <과제 키 또는 메시지 ID>]...
        [--sha <SHA>] [--risk "<사유>"]... [--origin user|bot] [--allow <사람>]... [--repository <owner/repo>]
fullops decision answer <ID> --answer "<답>" [--by <사람>] [--via user|bot] [--sha <SHA>]
fullops decision drop <ID> --reason "<사유>"
fullops decision list [--open]
```

- 묻기 전에 기록한다. ID는 `D` + 3자리 이상 숫자다.
- `sha`가 있으면 `expires_at = asked_at + 24시간`이다.
- `answer`는 다음을 모두 지켜야 받는다. 어기면 기록을 바꾸지 않고 종료 코드 2다. (1) `sha`가 있으면 `--sha`가 같다. (2) `expires_at`이 지나지 않았다. (3) `allow`가 있으면 `--by`가 그 안에 있다.
- 이미 닫힌 결정에 온 답은 덮어쓰지 않는다. `{"already":true,"answer":…}`를 내고 종료 코드 0이다.
- 열린 결정은 `blocks`에 적힌 과제만 멈춘다. 이것을 강제하는 hook 규칙은 없다. lead의 S-01 안내와 `status`에 나오고, `merge`의 M-04가 리뷰 결정을 본다.
- 봇(orca-helm) 경로: 진입점이 봇인 과제(`routes`의 `origin = "bot"`)의 결정은 `--origin bot`으로 열고, 봇은 `fullops decision list --open --json`과 `status.json`으로 읽고 `decision answer --via bot`으로 닫는다. 어느 쪽에서 먼저 답해도 같은 기록을 닫는다.

### 8.11 `lint`

```
fullops lint [--from <ref>] [--level <레벨>] [--out <경로>]
```

1. 작업 트리가 깨끗해야 한다(아니면 종료 코드 2). `--from` 기본값은 `changed()`의 시작점이다.
2. `lint/lock`을 잡는다. 이전 `result.json`·`pass.json`을 지운다.
3. `mb = merge-base(from, HEAD)`. 설정은 `mb` 시점의 `lint/lint.json`과 `fullops.toml`이다. 한 번의 `git cat-file --batch`로 읽는다. `HEAD`의 `lint.json`이 `mb`와 다르면 LINT-001 경고를 낸다.
4. 바뀐 파일: `git diff --name-status -z mb HEAD`에서 `exclude` glob에 안 맞는 것. 추가된 줄은 `git diff -U0 mb HEAD`에서 얻는다.
5. 코드 파일 `is_code(path)`: 확장자가 `[code].extensions`에 있고 `exclude`에 안 맞는다. 내장 확장자 목록은 `.py .sh .bash .rb .gd .pl .r .js .jsx .ts .tsx .mjs .cjs .svelte .vue .go .rs .java .kt .kts .scala .c .h .cc .cpp .hpp .cs .swift .php .dart .css .scss .less .html .gdshader .shader`다.
6. 명령 선택(`commands[]`의 각 항목 `{name, run[], cwd, kind, level, required, paths, timeout_seconds}`):
   - `paths`가 있고 바뀐 파일이 하나도 안 맞으면 skipped(사유 `바뀐 파일이 명령의 paths와 맞지 않음`). `required`보다 우선한다.
   - `kind == "test"`이고 `level`이 있으며 적용 레벨(`--level`과 기준 `test_level` 가운데 높은 쪽)보다 높고 `required`가 아니면 skipped.
   - `kind == "test"`이고 `required`가 아니며 코드 파일 변경이 없으면 skipped(사유 `코드 파일 변경 없음`).
   - 그 밖은 실행한다. `level`이 없는 명령은 항상 실행한다.
7. 실행 순서: `kind`가 `test`가 아닌 명령을 등록 순서대로 차례로 모두 실행한다. 하나라도 실패하면 test 명령은 skipped(사유 `앞 단계 명령이 실패해 실행하지 않음`). 통과하면 test 명령을 동시에 실행한다. `parallel_tests: false`면 차례로 실행하고 하나가 실패하면 나머지를 건너뛴다.
8. 실행기: 셸을 거치지 않는 인자 배열, `cwd`는 레포 안이어야 한다. 타임아웃은 명령의 `timeout_seconds`, 없으면 설정의 `timeout_seconds`(기본 600). 넘으면 프로세스 트리를 끝낸다(Windows는 Job Object, POSIX는 프로세스 그룹). 실행 파일을 못 찾으면 `unrunnable`이다. 출력 전체는 `lint/logs/<name>.log`에, 끝 40줄은 결과의 `output_tail`에 둔다.
9. 내장 검사(코드·심각도는 템플릿 `lint/README.md`의 검사 표와 같다):

   | 코드 | 심각도 | 대상과 조건 |
   |---|---|---|
   | SIZE-001 | `size.severity` | 바뀐 코드 파일의 주석·빈 줄 제외 줄 수가 `max_code_lines`를 넘고 `mb`보다 늘었다. `.md`는 전체 줄 수를 `max_doc_lines`와 비교 |
   | SIZE-002 | WARNING | 추가 줄 합이 `max_diff_added`를 넘는다(0이면 끈다) |
   | DEP-001 | WARNING | `package.json`, `pyproject.toml`, `requirements*.txt`, `go.mod`, `build.gradle*`의 변경·삭제·이름 변경 |
   | ANTI-002 | ERROR | 추가된 줄의 `eval(`·`exec(`(Python·JS 계열, 앞 글자가 단어·`.`·`$`가 아닐 때) |
   | ANTI-003 | ERROR | 추가된 줄의 범위 없는 억제: `# type: ignore`(뒤에 `[` 없음), `# pyright: ignore`(뒤에 `[` 없음), `# noqa`(뒤에 `:` 없음), `@ts-ignore`, `@ts-nocheck`, 규칙 이름 없는 `eslint-disable` |
   | ANTI-004 | ERROR | 테스트 파일의 추가된 줄에 skip·only 표식(`.skip(`, `.only(`, `.todo(`, `xit(`·`fit(` 계열, `@pytest.mark.skip`·`xfail`, `pytest.skip(`, `@unittest.skip`, `t.Skip(`, `#[ignore]`, `@Ignore`, `@Disabled`, `XCTSkip`, `pending(`) |
   | ANTI-005 | WARNING | 테스트 케이스 수 감소 또는 테스트 파일 삭제 |
   | SEC-001 | ERROR(테스트 파일·`.md`는 WARNING) | 추가된 줄에 `password`·`api_key`·`secret`·`token` 계열 이름에 따옴표 리터럴 대입. 값이 `test`·`example`·`dummy`·`placeholder`·`changeme`·`xxx`·`your_`·`<`로 시작하거나 환경 변수 참조면 예외 |
   | DOC-001 | WARNING | 새 코드 파일 맨 위에 헤더 설명(주석·docstring 1~3줄)이 없다 |
   | DOC-002 | ERROR | 바뀐 산출물 원천 문서의 front matter가 8.12절 규칙에 어긋난다 |
   | DOC-003 | ERROR | 바뀐 `.fullops-squad/**/*.md`(인박스·로그·템플릿·`rules/`·`FULLOPS.md`·`PLANS.md` 제외)의 공통 front matter가 없거나 어긋난다. 코드 lint의 `exclude`와 별도로 적용한다 |
   | LINT-000 | WARNING | `commands`가 비었다 |
   | LINT-001 | WARNING | 브랜치가 `lint.json`을 바꿨다 |
   | LINT-002 | WARNING | 범위 없는 명령이 120초를 넘었다 |
   | `rules[].code` | 규칙별 | 레포 정규식 규칙. `file_extensions`, `exclude_paths`(glob), `exclude_patterns`, `multiline`, `enabled`를 지킨다. 추가된 줄(여러 줄 규칙은 일치 구간에 추가 줄이 있을 때)에만 적용한다 |

   정규식 엔진은 lookaround와 인라인 플래그를 받는 `fancy-regex`다. 템플릿의 DESIGN 규칙이 lookbehind를 쓰기 때문이다. 컴파일에 실패한 규칙은 `unrunnable` 한 건으로 세고 사유에 규칙 코드를 적는다.

10. 결과: `lint/result.json`

    ```json
    { "v": 1, "base": "…", "merge_base": "…", "head": "…", "config_sha256": "…", "test_level": "lite",
      "started_at": 1794500000, "seconds": 41.2,
      "commands": [ { "name": "ruff", "kind": "lint", "run": ["ruff","check","."], "cwd": "backend",
                      "status": "passed", "exit_code": 0, "seconds": 3.1, "reason": "", "output_tail": "…", "log": "…/ruff.log" } ],
      "findings": [ { "code": "SEC-001", "severity": "ERROR", "path": "a.py", "line": 12, "message": "…", "suggestion": "…" } ],
      "errors": 0, "warnings": 2, "unrunnable": 0, "passed": true }
    ```

    `commands[].status`: `passed`·`failed`·`timeout`·`unrunnable`·`skipped`. skipped는 통과가 아니고 사유와 함께 남는다. `failed`·`timeout`은 ERROR로 센다.
11. 통과(`errors == 0 && unrunnable == 0`)일 때만 `lint/pass.json`을 쓴다: `{"v":1,"base":<mb>,"head","tree","config_sha256","test_level","at"}`. WARNING은 통과를 막지 않는다.
12. 종료 코드: 통과 0, 미통과 2, 실행 실패 1.

### 8.12 `doc`

```
fullops doc check [--id <Dnn>] [--strict]
fullops doc stamp --path <경로> [--owner <담당>] [--summary "<요약>"] [--task <KEY>] [--status draft|review|approved]
```

- 공통 front matter 필수 필드: `id`, `title`, `status`(`draft`·`review`·`approved`), `updated`(`YYYY-MM-DD`), `owner`, `summary`. 목록 필드: `tasks`, `upstream`, `downstream`. 쓰는 순서는 `id, title, status, statuses, updated, owner, tasks, upstream, downstream, summary`다.
- `stamp`는 그 순서로 front matter를 다시 쓰고 `updated`를 오늘로, `--task`를 `tasks`에 더한다. 본문은 건드리지 않는다.
- `check`: 인덱스(`docs/deliverables/README.md`)의 D01–D13 행마다 원천 문서의 존재, 로컬 링크의 존재, front matter 필수 필드, 인덱스 상태와 문서 상태의 일치를 본다. 인덱스 표의 열 문법은 미정-M7이다.
- 의미 추적성(요구사항 → 화면 → 모듈 → QA)은 자동으로 검사하지 않는다. 스킬 문서의 수동 점검이다.

### 8.13 `plan`, `status`, `why`, `rules`

- `plan list`: 과제 표의 행을 낸다. `plan ready`: `ready()`. `plan check`: 문법 오류의 줄 번호.
- `plan add <KEY> --role <R> --goal "…" --source "…" [--after K,…] [--status "…"]`: 과제 표 끝에 행을 더한다. 상태 기본값은 `대기`다. `plan set <KEY> "<상태 칸 글>"`: 상태 칸을 바꾼다. 둘 다 표의 다른 부분을 건드리지 않는다. 커밋하지 않는다.
- `status`: `status.json`과 같은 내용을 사람용으로 낸다. `--json`이면 그대로 낸다. 통합 대기 건마다 `landing`을 다시 계산한다.
- `why [--session <sid>] [--last <N>] [--rule <ID>]`: 추적 줄과 이벤트별 실행 표식을 낸다.
- `rules [--markdown|--json]`: 규칙 표.

## 9. 바깥 도구 연동

### 9.1 git

- 프로세스 없이 읽는 것: 레포 루트, Git 디렉터리, 공용 디렉터리(`.git` 파일의 `gitdir:`와 그 안의 `commondir`), 현재 브랜치와 detached 여부(`HEAD` 파일).
- 프로세스로 읽는 것: `merge-base`, `merge-base --is-ancestor`, `diff`, `status`, `cat-file --batch`, `rev-parse --verify`, `worktree list --porcelain`.
- 쓰기는 CLI만 한다: `commit --only`, `mv`, `merge`(`--ff-only` 또는 `--no-edit`), `merge --abort`, `reset --keep`(M-07만), `push`(force 없음), `fetch`, `update-ref`(이전 값 확인 포함), `worktree add --detach`, `worktree remove`(force 없음).
- 모든 호출의 환경: `GIT_TERMINAL_PROMPT=0`, `GIT_NO_LAZY_FETCH=1`, `LC_ALL=C`. 인자에 `-c core.quotepath=false`.
- 제한 시간: hook에서는 호출당 2초. CLI에서는 30초, `fetch`·`push`·`merge`는 120초.
- 최소 버전 2.41. `doctor`가 확인한다.
- hook은 fetch하지 않는다. fetch하는 명령은 `merge`, `settle`, `status --fetch`뿐이다.

### 9.2 Orca

실행 파일은 `ORCA_CLI_COMMAND` → `orca-dev`(`ORCA_DEV_REPO_ROOT`가 있을 때) → `orca-ide`(Linux) → `orca` 순으로 찾는다. 실패해도 다른 실행 파일로 바꾸지 않는다. 모든 호출에 `--json`을 붙이고 `{"ok":true,"result":…}` 형태를 읽는다. 최소 버전 1.4.246(`doctor`가 확인한다).

| 부르는 곳 | 명령 | 제한 시간 | 실패하면 |
|---|---|---|---|
| X-03 | `orchestration worker-show --dispatch <ID>` | 3초 | `Unknown` → 통과 |
| T-11 | `orchestration check [--run R] --peek` | 2.5초 | 효과를 건너뛴다 |
| F-01 | `orchestration send --type escalation …` | 6초 | 추적에 남긴다 |
| `dispatch` | `orchestration run-create`, `orchestration worker-start …` | `ready_timeout_seconds` + 30초 | 종료 코드 4 |
| `wait` | `orchestration check --run R`, ack, `orchestration worker-list --run R`, `orchestration run-use --id R` | 호출당 15초(기다림 호출은 `max-minutes`) | 30초 뒤 다시, 세 번이면 종료 코드 4 |
| `review cleanup` | `terminal list --worktree path:<경로>` | 10초 | 정리하지 않는다 |

응답 해석은 `fullops-ext::orca` 한 곳에서 한다. 모르는 필드는 무시하고 필수 필드가 없으면 `Unknown`이다. worker 응답에서 읽는 값은 `dispatchId`, `taskId`, `runId`, `outcome`(또는 `state`가 `succeeded`·`failed`), `stage == "settled"`, `liveness`, `terminal`이다(1.4.246의 평면 형식만 읽는다). 메시지에서 읽는 값은 `id`, `type`, `body`, `payload.taskId`, `payload.dispatchId`, `run_id`, 전달 ID다. 실제 응답과 인자 이름은 미정-M3이고 묶음 0이 fixture로 고정한다.

### 9.3 분류 호출 (Jev)

- `route`만 부른다. hook은 부르지 않는다.
- 엔드포인트: `TYPESAFE_API_KEY`가 있으면 `https://api.typesafe.ai/v1/systemone`(model `jev-latest`), 없고 `OPENROUTER_API_KEY`가 있으면 `https://openrouter.ai/api/v1/systemone`(model `~typesafe/jev-latest`). 키를 찾는 순서는 `--env-file` → 환경 변수 → `.fullops-squad/.env`. env 파일은 키 줄만 읽는다.
- 전송은 `curl` 프로세스다. 키와 본문은 `curl --config -`의 표준입력으로 넘기고 명령행 인자에 두지 않는다. 연결 5초, 전체 20초. HTTP 라이브러리를 바이너리에 링크하지 않는 이유: hook 실행 파일을 작게 유지하고 TLS 빌드 의존을 없앤다. curl은 Windows 10 이상·macOS·Linux에 기본으로 있다.
- 캐시: 요청 본문의 sha256으로 `~/.fullops/cache/jev/`에 둔다. `FULLOPS_JEV_CACHE_BYPASS=1`이면 읽지 않는다.
- `FULLOPS_OFFLINE=1` 또는 `FULLOPS_JEV_ENABLED=0`이면 호출과 캐시 읽기를 모두 건너뛰고 폴백한다.
- 요청·응답 스키마는 미정-M6이다.

### 9.4 gh, GitHub

v1은 쓰지 않는다. 현황판의 이슈 목록이 없어졌고 이슈 자동 모드는 나중이다.

## 10. 크레이트, 모듈, 설치, 배포

### 10.1 워크스페이스

```
Cargo.toml                     # workspace
crates/
  fullops-core/                # 순수 로직. 파일·프로세스·시계에 직접 닿지 않는다
    model.rs  config.rs  plan.rs  frontmatter.rs  shell.rs  patch.rs
    facts.rs  rules/{mod,session,prompt,tool,stop,failure}.rs  derive.rs
    review.rs  lint_rules.rs  messages.rs
  fullops-store/               # 파일 계층
    repo.rs  state.rs  session.rs  turns.rs  trace.rs  files.rs  lock.rs
  fullops-ext/                 # 바깥 프로세스
    proc.rs  git.rs  orca.rs  jev.rs
  fullops/                     # 바이너리
    main.rs  hook/{mod,host_claude,host_codex,facts_real,effects}.rs
    cmd/{route,task,plan,dispatch,wait,done,merge,hold,review,decision,lint,doc,status,why,rules,
         init,mode,upgrade,migrate,install,doctor,env,gc}.rs
    assets/                    # include_str!: 레포 템플릿, 릴리스 노트
  fullops-testkit/             # dev: 임시 레포 빌더, fake-orca, 시나리오 실행기
xtask/                         # dist, bench, compare
plugin/                        # 매니페스트, hook 등록 파일, 스킬, bin 런처
scenarios/                     # 12절
panel/                         # Orca 패널(TypeScript)
```

의존 방향: `fullops-core` ← `fullops-store`, `fullops-core` ← `fullops-ext`, 둘 ← `fullops`. `core`는 `serde`·`fancy-regex`·`globset`만 쓴다. `store`와 `ext`는 서로 모른다. 외부 크레이트: `serde`, `serde_json`, `toml`, `fancy-regex`, `globset`, `sha2`, `lexopt`(인자 해석). 비동기 런타임, HTTP·TLS, SQLite, git 라이브러리, 날짜 라이브러리는 쓰지 않는다. 시각은 에폭 초다(날짜 문자열은 자체 변환).

### 10.2 모듈 사이 인터페이스

```rust
// fullops-core::facts — hook 규칙과 CLI 검사가 바깥을 보는 유일한 통로
pub trait Facts {
    fn now(&self) -> i64;
    fn caps(&self) -> Caps;
    fn config(&mut self) -> Result<&Config, PolicyError>;
    fn policy_error(&mut self) -> Option<PolicyError>;         // 설정 오류 또는 세션 중 모드 변경
    fn actor(&mut self) -> Known<Actor>;
    fn session(&mut self) -> Known<&Session>;
    fn state(&mut self) -> Known<&State>;
    fn archive_keys(&mut self) -> Known<&BTreeSet<TaskKey>>;
    fn plan(&mut self) -> Known<&Plan>;
    fn inbox(&mut self, role: &RoleId) -> Known<Option<Inbox>>;     // 이 체크아웃의 인박스
    fn route(&mut self, key: &TaskKey) -> Known<Option<Route>>;
    fn file_text(&mut self, path: &Path) -> Known<Option<String>>;
    fn path_exists(&mut self, path: &Path) -> Known<bool>;
    fn changed(&mut self) -> Known<ChangeSet>;                      // 6.6절. { start: Sha, code: Vec<String>, dirty: Vec<(String, i64)> }
    fn pass_covers(&mut self, head: &Sha, start: &Sha) -> Known<bool>;
    fn head(&mut self) -> Known<Sha>;
    fn review_need(&mut self, base: &Sha, head: &Sha) -> Known<Need>;
    fn review_evidence(&mut self, head: &Sha) -> Known<ReviewEvidence>;  // Accepted | Waived | None
    fn landing(&mut self, sha: &Sha) -> Known<Landing>;
    fn qa_commit_candidates(&mut self, include_unstaged: bool) -> Known<Vec<String>>;
    fn orca_settled(&mut self, id: &DispatchId) -> Known<bool>;
    fn waiter_alive(&mut self, run: &RunId) -> Known<bool>;
}

// fullops-core
pub fn decide(input: &HookInput, facts: &mut dyn Facts) -> Outcome;
pub struct Outcome { pub verdict: Verdict, pub effects: Vec<Effect>, pub trace: Vec<RuleTrace> }
pub enum Effect { CreateSession, TouchSession, BumpTurn, BindDispatch { id, task, by }, RecordBlock { rule, fp },
                  RecordNotice(String), AppendRouted(TaskKey), RecordTurn(TurnKind), StampCurrent,
                  MarkLanded(Vec<MessageId>), MarkSettled(DispatchId), CloseRuns(Vec<RunId>),
                  CloseLostSnapshots, GcSessions, SelfInstall, PeekCompletions { run: Option<RunId> },
                  SendLimitEscalation { dispatch: DispatchId, task: Option<String>, error: String } }
pub fn parse_plan(text: &str, template: &str) -> Plan;
pub fn ready(plan: &Plan, state: &State, archive: &BTreeSet<TaskKey>, inbox_exists: &dyn Fn(&RoleId) -> bool) -> Vec<TaskKey>;
pub fn stage_of(key: &TaskKey, state: &State, archive: &BTreeSet<TaskKey>, routed: bool) -> Stage;
pub fn parse_front_matter(text: &str) -> Result<(FrontMatter, usize /* 본문 시작 바이트 */), FrontMatterError>;
pub fn is_code(path: &str, code: &CodeDef, exclude: &globset::GlobSet) -> bool;
pub fn review_need(policy: &ReviewPolicy, numstat: &[FileStat], code: &CodeDef, exclude: &globset::GlobSet,
                   base: Sha, head: Sha, merge_base: Sha) -> Need;
pub fn check_dispatch(req: &DispatchRequest, facts: &mut dyn Facts) -> Vec<Refusal>;      // D-01~D-10
pub fn check_task_new(req: &TaskNewRequest, facts: &mut dyn Facts) -> Vec<Refusal>;       // H-01~H-04
pub fn record_completion(state: &mut State, msg: &OrcaMessage, resolve: &mut dyn FnMut(&str) -> Option<Sha>,
                         branch_tip: &mut dyn FnMut(&RoleId) -> Option<Sha>) -> CompletionNote;   // 8.6절
pub fn metrics(turns: &[Turn], state: &State, archive: &[ArchiveLine]) -> Metrics;
pub struct Refusal { pub rule: &'static str, pub reason: String, pub escape: String, pub exit: u8 }

// fullops-store
pub struct Repo { pub root: PathBuf, pub git_dir: PathBuf, pub common_dir: PathBuf }
pub fn discover(cwd: &Path) -> io::Result<Option<Repo>>;          // None: 활성 표식 없음. git 프로세스를 띄우지 않는다
pub fn head(repo: &Repo) -> io::Result<HeadRef>;                   // Branch(String) | Detached(Sha)
pub fn load_config(repo: &Repo) -> Result<Config, PolicyError>;
pub fn read_state(repo: &Repo) -> Known<State>;
pub fn update_state<T>(repo: &Repo, wait: Duration, lead_plan: Option<&Plan>,
                       f: impl FnOnce(&mut State) -> T) -> Result<T, StoreError>;   // 잠금, 원자 교체, status.json 재생성
pub fn read_session(repo: &Repo, sid: &SessionId) -> Known<Option<Session>>;
pub fn update_session<T>(repo: &Repo, sid: &SessionId, wait: Duration, f: impl FnOnce(&mut Session) -> T) -> Result<T, StoreError>;
pub fn append_turn(repo: &Repo, turn: &Turn) -> io::Result<()>;
pub fn append_trace(repo: &Repo, sid: &SessionId, line: &TraceLine);
pub fn read_inbox(repo: &Repo, role: &RoleId) -> Known<Option<Inbox>>;
pub fn read_plan(repo: &Repo) -> Known<Plan>;
pub fn read_route(repo: &Repo, key: &TaskKey) -> Known<Option<Route>>;
pub fn write_route(repo: &Repo, route: &Route) -> io::Result<()>;
pub fn read_lint_pass(repo: &Repo) -> Known<Option<LintPass>>;

// fullops-ext
pub fn run(argv: &[String], cwd: &Path, timeout: Duration, env: &[(&str, &str)], stdin: Option<&[u8]>) -> ProcResult;
        // ProcResult { status: Exited(i32) | TimedOut | NotFound, stdout, stderr }. 타임아웃이면 프로세스 트리를 끝낸다
pub trait Git {   // 구현: GitCli { repo_root, timeout }
    fn is_ancestor(&self, older: &str, newer: &str) -> Known<bool>;
    fn merge_base(&self, a: &str, b: &str) -> Known<Sha>;
    fn rev_parse(&self, rev: &str) -> Known<Sha>;
    fn diff_names(&self, from: &str, to: &str) -> Known<Vec<String>>;
    fn numstat(&self, from: &str, to: &str) -> Known<Vec<FileStat>>;
    fn status(&self, pathspec: Option<&str>) -> Known<Vec<StatusEntry>>;
    fn show_blobs(&self, items: &[(String, String)]) -> Known<Vec<Option<Vec<u8>>>>;   // (rev, path) 묶음 조회
    fn worktrees(&self) -> Known<Vec<Worktree>>;                                        // { path, branch: Option<String>, head }
    fn commit_only(&self, paths: &[String], message: &str) -> Result<Option<Sha>, ExtError>;  // 바뀐 것이 없으면 None
    fn merge(&self, cwd: &Path, rev: &str, mode: MergeMode) -> Result<MergeOutcome, ExtError>; // FfOnly | NoEdit → Done | Conflict
    fn push(&self, remote: &str, refspec: &str) -> Result<(), ExtError>;
    fn fetch(&self, remote: &str, branch: &str) -> Result<(), ExtError>;
    // 그 밖: mv, merge_abort, reset_keep, update_ref, worktree_add_detached, worktree_remove
}
pub trait Orca {  // 구현: OrcaCli. 시험: fake-orca 실행 파일
    fn run_create(&self) -> Result<RunId, ExtError>;
    fn worker_start(&self, req: &StartRequest) -> Result<Started, ExtError>;   // Started { dispatch, task, terminal }
    fn check(&self, run: &RunId, peek: bool, wait: Option<Duration>) -> Result<Vec<OrcaMessage>, ExtError>;
    fn ack(&self, run: &RunId, deliveries: &[String]) -> Result<(), ExtError>;
    fn worker_show(&self, dispatch: &DispatchId) -> Result<WorkerView, ExtError>;
    fn worker_list(&self, run: &RunId) -> Result<Vec<WorkerView>, ExtError>;
    fn send_escalation(&self, dispatch: &DispatchId, task: Option<&str>, subject: &str, body: &str) -> Result<(), ExtError>;
    fn terminals_in(&self, worktree: &Path) -> Result<Vec<String>, ExtError>;
    fn run_use(&self, run: &RunId) -> Result<(), ExtError>;
}
pub trait Judge { fn ask(&self, state: &str, questions: &[Question]) -> Result<Vec<Answer>, JudgeError>; }
```

묶음 사이에 공유하는 파일 형태는 넷이다: `state.json`(C·E가 쓰고 B가 읽는다), 세션 파일(B가 쓰고 C의 `route`가 `routed`를 더한다), `lint/pass.json`·`result.json`(D가 쓰고 B·C·E가 읽는다), 리뷰 폴더의 `accepted.json`·`waiver.json`(E가 쓰고 B가 읽는다).

### 10.3 실행본 위치와 설치

- 자체 마켓플레이스판(Claude Code)과 Codex 패키지의 hook은 고정 위치 `~/.fullops/bin/fullops[.exe]`를 실행한다. 호스트가 버전별 캐시를 지워도 떠 있는 세션의 hook이 깨지지 않는다.
- `fullops install [--prune] [--rollback]`: 자기 자신을 `~/.fullops/bin/fullops.new-<pid>`로 복사 → 있던 파일을 `fullops.old-<시각>`으로 이름 변경 → 새 파일을 제자리로 이름 변경 → `install.json` 쓰기. 중간에 실패하면 이름을 되돌린다. 실행 중인 hook은 이름이 바뀐 옛 파일로 끝까지 돈다(Windows도 실행 중인 exe의 이름은 바꿀 수 있다). `--prune`은 옛 파일을 지우고 잠긴 것은 건너뛴다. `--rollback`은 가장 최근 옛 파일로 되돌린다.
- 자동 설치(S-04): hook이 `--plugin-root`를 받았고 `--in-place`가 아닐 때, (1) 자기 경로가 고정 위치가 아니고 고정 위치에 실행본이 없거나 (2) `<plugin-root>/VERSION`이 자기 버전보다 높으면(semver 비교) 플러그인 폴더의 맞는 바이너리로 위 절차를 한다. 낮은 버전으로는 바꾸지 않는다. 판정을 낸 뒤 효과로 실행하고 다음 호출부터 적용된다. 이것으로 첫 설치 직후와 플러그인 갱신 직후에 사람이 `install`을 따로 실행하지 않아도 된다.
- 디렉터리판은 자기 폴더 밖에 쓰지 않는다. `--in-place`로 플러그인 폴더의 바이너리를 직접 실행하고 자동 설치를 하지 않는다.
- `state.json`의 `v`가 실행본이 아는 값보다 크면 hook은 통과하고 lead에게 세션당 한 번 "설치본이 낡았다"를 알린다.

### 10.4 패키지 배치와 hook 등록

```
<패키지>/
  .claude-plugin/plugin.json  또는  .codex-plugin/plugin.json
  hooks/hooks.json            # Claude 판(마켓플레이스판과 디렉터리판은 명령 문자열이 다르다)
  hooks/codex.json            # Codex 패키지에서만
  skills/<이름>/SKILL.md
  bin/fullops                 # POSIX sh 런처: uname으로 아래 가운데 하나를 exec 한다
  bin/fullops.exe             # windows-x64 (arm64는 에뮬레이션으로 실행)
  bin/fullops-darwin-arm64  bin/fullops-darwin-x64  bin/fullops-linux-x64
  VERSION  releases/*.md  mcp.json(Context7 원격 HTTP)
```

`bin/fullops`를 확장자 없이 두는 이유: cmd·PowerShell은 `fullops`를 `fullops.exe`로 풀고, POSIX 셸은 런처 스크립트를 실행한다. 명령 문자열 하나가 세 OS에서 돈다.

hook 명령 문자열(호스트가 POSIX 셸로 실행한다는 가정. 1.24.0 디렉터리판 README의 "Windows에서 Claude Code는 hook을 Git Bash로 실행한다"가 근거이고 확인은 미정-M1):

| 판 | 명령(모든 이벤트 공통) |
|---|---|
| Claude 자체 마켓플레이스판 | `B="$HOME/.fullops/bin/fullops"; [ -x "$B" ] \|\| B="${CLAUDE_PLUGIN_ROOT}/bin/fullops"; exec "$B" hook claude --plugin-root "${CLAUDE_PLUGIN_ROOT}"` |
| Claude 디렉터리판 | `exec "${CLAUDE_PLUGIN_ROOT}/bin/fullops" hook claude --in-place` |
| Codex 패키지 | `exec "$HOME/.fullops/bin/fullops" hook codex`. 플러그인 루트 변수가 확인되면 마켓플레이스판과 같은 폴백을 넣는다(미정-M1). 그 전까지 Codex는 setup 스킬이 `<패키지>/bin/fullops install`을 한 번 실행한다 |

미정-M1의 결과가 가정과 달라도 바이너리 쪽 인터페이스(`fullops hook <host> [--plugin-root] [--in-place]`)는 바뀌지 않는다. 바뀌는 것은 등록 파일의 문자열뿐이고 묶음 F가 고친다.

판별 제약: 디렉터리판은 자기 폴더만 쓰고 다른 플러그인·전역 명령에 기대지 않는다. Codex 등록 파일에는 StopFailure가 없다. 배포본은 `cargo xtask dist`의 생성물이고 원본(`plugin/`)을 고쳐 다시 만든다.

### 10.5 빌드

- `cargo xtask dist`: 네 타깃(`x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`)의 릴리스 바이너리, `dist/claude-marketplace/fullops`, `dist/claude-directory/fullops`, `dist/codex/fullops`, 파일별 `SHA256SUMS`, `VERSION`을 만든다. CI(GitHub Actions의 OS별 러너)가 타깃별로 빌드해 모은다.
- 릴리스 프로필: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `panic = "unwind"`(hook의 `catch_unwind` 때문), `strip = true`.
- 폐쇄망 빌드: `cargo vendor`한 의존을 저장소 밖 보관소에 두고 `--offline`으로 빌드하는 절차를 `docs/build.md`에 적는다(묶음 F).
- 바이너리 유통과 서명은 사람 결정이다(17절 H3).

### 10.6 `init`, `mode`, `upgrade`, `doctor`

- `fullops init --roles a,b [--kind <역할>=<kind>]... [--mode coor|dev --primary-role R] [--remote origin --base main | --local-only] [--test-level L] [--subagent-level S] [--dry-run]`: 없는 파일만 만든다(템플릿은 바이너리에 내장). `fullops.toml`을 쓴다. `AGENTS.md`·`CLAUDE.md`에 표식으로 감싼 관리 블록을 넣는다. 원격 역할 브랜치 `fullops/<역할>`이 없으면 만든다(있는 브랜치는 옮기지 않는다). 심볼릭 링크 대상은 거부한다. 커밋하지 않는다. 다시 실행하면 이어서 한다. 새 레포의 기본 레벨은 `lite`/`lite`다.
- `fullops mode dev --primary-role R`: `fullops.toml`의 `mode`·`primary_role`·`primary_branch`를 원자 교체로 바꾼다. 떠 있는 세션은 정책 오류가 되어 새 세션이 필요하다. dev → coor는 지원하지 않는다(1.24.0과 같다).
- `fullops upgrade [--verify]`: `applied_version` 이후의 내장 릴리스 노트를 내고, 코드에 든 필수 적용 검사 목록(`RequiredApply { since, id, check, hint }`)을 돌린다. 전부 통과하면 `applied_version`을 올린다. 낮추지 않는다.
- `fullops doctor [--json]`: 네트워크 없이 본다. 실행본 버전·해시와 `install.json`, git·orca 버전, `fullops.toml` 검증, `state.json` 읽기, 최근 24시간 추적의 오류·시간 초과 수, 추적이 없는 최근 세션 수, Git 공용 디렉터리가 네트워크·동기화 폴더에 있는지.

## 11. 기능 대응표

### 11.1 hook 동작

| 1.24.0 기능 | 새 설계 | 처리 |
|---|---|---|
| 미설정 레포 빈 판정, fail-open, 정책 오류 차단 | 7.1절 3번, 7.8절 | v1 |
| SessionStart 역할 안내, 레벨, lead의 준비된 행·위임 범위·열린 결정·버전 차이 | S-01 | v1 |
| SessionStart 세션 기준점(reflog 길이) | S-02의 `anchor`(HEAD SHA)와 6.6절 `changed()` | v1, 방법 변경 |
| SessionStart 사라진 snapshot 닫기 | S-03 | v1 |
| SessionStart `.env*` 연결 | `dispatch` 6단계, `task recover`, `env link` | v1, 위치 이동 |
| SessionStart 필수 CLI 누락 알림 | S-01(git·orca) | v1 |
| SessionStart caveman 스킬 주입 | - | 버림. 하네스 기능이 아니다. 그 스킬의 플러그인이 맡는다 |
| SessionStart python3·orca 전체 경로 안내 | - | 버림. 스킬이 `fullops`만 부른다 |
| SessionStart 이슈 모드 세션·터미널 결합 기록 | 세션 파일의 `terminal` | v1(이슈 모드용 기록은 나중) |
| UserPromptSubmit dispatch 인식, dev 역할 안내 | P-01 | v1 |
| UserPromptSubmit 사람 턴 기록 | P-02 | v1 |
| PreToolUse 터미널 주입, 긴 `terminal send` | T-02, T-03 | v1 |
| PreToolUse `--run` 없는 `worker-start` | T-01(날것 `worker-start` 전부) | v1, DF-1 |
| PreToolUse 배정 선행 조건(route, 인박스, 미통합, 기준선, 리뷰 snapshot) | `dispatch`의 D-01~D-10, 날것 `task-create`는 T-04 | v1, 위치 이동 |
| PreToolUse 인박스 규칙 | T-05, `task new`의 H-03·H-04 | v1 |
| PreToolUse 지시서 작성 권한 | `task new`의 H-02, T-06 | v1 |
| PreToolUse 설계 역할 코드 수정 | T-07 | v1 |
| PreToolUse 하위 위임 레벨 | T-08(호스트 내장), D-08(Orca 배정) | v1, DF-6 |
| PreToolUse 링크 없는 캡처 커밋, 차수 폴더 | T-09, T-10 | v1 |
| PreToolUse 이슈 모드 경계·dispatch intent | - | 나중(이슈 모드와 함께) |
| PreToolUse lead의 `check` 미리 보기 기록 | T-11 | v1 |
| PostToolUse 전달 영수증 정산 | `wait`의 정산 기록과 X-03의 Orca 조회 | v1, 방법 변경. PostToolUse hook을 없앤다 |
| PostToolUseFailure | - | 버림. 위와 같다 |
| PostToolUse 지시서의 `--level` 알림 | `dispatch` 7단계 | v1, 위치 이동(DF-2) |
| PostToolUse 보고서 40줄·`raw/` 링크 알림 | `done`의 R-05 | v1, 위치 이동(DF-2) |
| PostToolUse 폭·테마 캡처 요구 탐지 | - | 버림. 산문 휴리스틱이고 값어치가 측정되지 않았다 |
| PostToolUse 완료 보고 형식·lint 뒤 변경 알림 | `wait`의 `notes`, `done`의 R-03, `merge`의 M-05 | v1, 위치 이동 |
| PostToolUse 병합 SHA 어긋남 알림 | `completions[].drift`(8.8절 끝) | v1, 위치 이동 |
| PostToolUse 반복 예산 알림 | `task reopen` 출력, S-01 | v1, 위치 이동 |
| PostToolUse 흐름 노드 기록 | 6.6절 `stage_of`(유도) | v1, 방법 변경 |
| PostToolUse 긴 출력 줄이기 | - | 나중. Claude Code Bash 전용이고 절감량이 측정되지 않았다 |
| PostToolUse 이슈 모드 lease 연장 | - | 나중 |
| Stop lint 통과 게이트, dev 직접 작업의 리뷰 요구 | X-01, X-02 | v1 |
| Stop 미정산 dispatch | X-03 | v1 |
| Stop 분류 후 미배정 | X-04 | v1 |
| Stop Run 대기(백그라운드 대기 통과) | X-05(대기 프로세스의 heartbeat로 판정) | v1, 방법 변경 |
| Stop 미통합 완료 보고 | X-06 | v1 |
| Stop PLANS만 바꾸고 현황판 단계 미갱신 | - | 버림. 현황판 단계 파일이 없다(DF-3) |
| Stop 준비된 대기 행 | X-07 | v1 |
| Stop 현황판 데이터 재생성 | `status.json`(상태가 바뀔 때) | 대체 |
| StopFailure escalation | F-01 | v1 |
| SessionEnd 이슈 모드 끄기 | - | 나중 |
| hook 실행 기록 조회 | `sessions/<sid>.seen/`, `trace/`, `fullops why` | v1 |
| 고정 hook 실행 경로와 실행본 보존 | 10.3절 | v1, 방법 변경 |

### 11.2 스크립트

| 1.24.0 | 새 설계 | 처리 |
|---|---|---|
| `flow_gate`, `done_gate`, `harness_gate`, `policy`, `storage` | `fullops hook`, `fullops-core::rules`, `fullops-store` | v1 |
| `hook_runtime` | `fullops install`, S-04 | 대체 |
| `output_trim` | - | 나중 |
| `caveman_start` | - | 버림 |
| `jev_route`(분류, tuple 기록, `--model-only`, `--origin`, 재분류) | `fullops route` | v1 |
| `jev_route --bind-inbox`, `--strict` | - | 버림. 인박스 front matter가 `route`를 가진다 |
| `work new\|reopen\|recover` | `fullops task …` | v1(recover의 패킷 재생성 제외) |
| `work finish` | `fullops done` | v1. 후속 요청은 `--followup`으로 받는다(DF-7) |
| `orca_wait` | `fullops wait` | v1 |
| `orca_wait --settings` | `fullops.toml`의 `[wait]` | 대체 |
| `session_cache` | - | 버림. 호스트 내부 기록 형식에 기댄다. 기본은 새 세션이고 이어 하기는 `dispatch --terminal`로 lead가 정한다 |
| `env_link` | `dispatch` 6단계, `fullops env link` | v1 |
| `grok_trust` | - | 버림(호스트 범위 밖) |
| `decision` | `fullops decision` | v1 |
| `flow_state` | `stage_of`, `fullops status` | 대체 |
| `integration status\|hold\|resume\|set-sha\|settle` | `fullops status`, `hold`, `resume`, `completion set-sha`, `settle` | v1. 호출당 5건 상한은 없다 |
| `review need\|waive\|snapshot\|check\|cleanup\|supersede\|prune` | `fullops review …` | v1 |
| `review prepare`(OCR CLI) | `review snapshot`이 대상과 규칙을 자체 계산 | 대체. npm 의존을 없앤다 |
| `review check --historical` | `accepted.json`의 해시 확인 | 대체 |
| `autonomy judge\|compare`(섀도 판정) | - | 나중. 기록만 하고 병합을 바꾸지 않으며 결과를 검토한 기록이 없다. `merge`의 차단 조건(lint ERROR, 통과 기록 없음, 미해결 critical·high)은 M-04·M-05에 있다 |
| `lint` | `fullops lint` | v1 |
| `lint_tune` | - | 나중(제안 도구) |
| `jev_find`, `jev_context`, `jev_packet`, `search_index`, packet-outcomes 검사 | - | 나중. 선택 기능이고 효과가 측정되지 않았다 |
| `jev_observe`, find의 score·rerank·hierarchical | - | 버림(실험, 게이트와 무관) |
| `board`의 준비된 행·위임 범위 | `fullops-core::plan` | v1 |
| `board`의 지표 | 6.7절, `fullops status` | v1 |
| `board`의 `board-data.js`·gh 이슈 목록, `board_documents`, `board/index.html` | Orca 패널(`status.json`) | 버림(#18) |
| `deliverables` | `fullops doc` | v1 |
| `issue_mode`, `issue_github` | - | 나중(18.3절) |
| `jev_test_web`, `web_*`, `jev_test_unity`, `unity_bridge`, `test_record`, `render.mjs`, `frontend.py` | - | 버림. Node·브라우저·Unity가 필수다. 플러그인에 싣지 않는다 |
| `setup` | `fullops init`, `fullops mode` | v1. 저널·롤백은 없다(없는 파일만 더하므로 다시 실행하면 이어진다) |
| `update` | `fullops upgrade` | v1 |
| `deps` | `fullops doctor`, `fullops install` | 대체. npm 도구·외부 스킬 설치는 하지 않는다 |
| `build.py`, `install.py`, `bin/fullops-squad.cjs` | `cargo xtask dist`, `fullops install` | 대체 |
| `mods/fullops-meter` | Orca 패널 | 버림(#18) |
| `mcp.json`(Context7) | 원격 HTTP 등록만 | v1 |

### 11.3 스킬과 자산

| 1.24.0 | 새 설계 | 처리 |
|---|---|---|
| 스킬 `setup-fullops`, `update-fullops`, `fullops-orca`, `fullops-work`, `fullops-review`, `fullops-deliverables` | 같은 이름. 명령 표기를 `fullops …`로 다시 쓰고 절차를 감싼 명령에 맞춘다 | v1 |
| 스킬 `fullops-test`, `fullops-frontend` | 문서만 남긴다(싼 수단부터, 증거 규칙, 디자인 정본 연결). 도구 설치·실행 안내는 뺀다 | v1(문서) |
| 스킬 `fullops-issues` | - | 나중 |
| 스킬 `fullops-motion` | - | 버림 |
| 자산 `FULLOPS.md`, `project.md`, `orca-agents.md`, `PLANS.md`, `handovers/_TEMPLATE.md`, `rules/`, `docs/` 뼈대, `review/rule.json`, `lint/lint.json`, `lint/README.md`, `.env.example`, `.gitignore` | 바이너리에 내장하고 `init`이 없는 것만 더한다. 문구의 스크립트 표기는 `fullops` 명령으로 바꾼다 | v1 |
| `orca-agents.md`의 라우팅 기준 표식 줄(설계·coordinator·tester·제품 기획·기술 계획 역할) | `fullops.toml`의 `kind`·`routing`·`tech_plan_role`·`about`. 문서의 줄은 사람용 설명으로 남는다 | 대체 |
| `orca-agents.md`의 `## 모델 후보` | 그대로. 기계는 읽지 않는다. lead가 읽고 고른 tuple을 `dispatch`가 기록한다 | v1 |
| `fullops.json` | `fullops.toml` | 대체 |
| `board/board.json`, `board/index.html` | - | 버림 |
| `frontend.json`, 외부 스킬 설정 문서 3종, Unity 패키지 | - | 버림 |
| 엔트리 문서 블록(AGENTS·CLAUDE·GEMINI.md) | AGENTS.md와 CLAUDE.md | v1(GEMINI.md 제외) |
| 역할 컨텍스트 `contexts/<역할>.md`의 150줄·10KB 한도 | 스킬 문서의 규약으로 유지. 기계 검사는 없다 | v1(문서) |

### 11.4 폐기·변경된 컨셉을 되살리지 않았다는 확인

| 폐기·변경된 컨셉(근거 릴리스) | 이 설계의 상태 |
|---|---|
| Jev의 모델·effort 선택, 리뷰어 모델 자동 선택(1.10.0) | `route`는 역할·산출물만 분류한다. tuple은 lead가 넘긴 값을 기록할 뿐이고 다음 과제로 넘기지 않는다 |
| 모든 변경의 독립 리뷰(1.5.0) | `review need`가 필요로 판정할 때만 M-04가 리뷰를 요구한다 |
| 패킷 필수, 전수 outcomes, route의 패킷 요구(1.8.0, 1.24.0) | v1에 패킷이 없고 `requires_packet` 필드를 읽지 않는다 |
| 터미널 주입 배정, `--run` 없는 `worker-start`(0.6.0) | T-01·T-02. `dispatch`는 항상 Run을 가진다 |
| 과제명 파일 병렬 배정(0.9.12) | T-05(a), 역할당 인박스 하나, D-10 |
| 완료마다 QA 전문 재독·lint 재실행, heartbeat로 모델 깨우기(0.4.0, 0.9.4) | `wait`가 흡수한다. M-07은 트리가 달라졌을 때만 lint를 돌린다 |
| 레벨과 무관한 다폭·테마 캡처, 차수 보고서 폴더, 링크 없는 증거 커밋(1.8.3) | T-09, T-10. 캡처를 요구하는 규칙이 없다 |
| 한도 종료 때 다른 모델·호스트로 전환(1.24.0) | D-07. `dispatch`에 대체 경로가 없다 |
| 되돌릴 수 있는 선택의 사용자 결정, 병합 뒤 계속 여부 질문, 결정 질문 개수 요구(1.24.0) | X-07이 준비된 행을 배정하게 한다. 결정을 요구하는 규칙이 없다 |
| 로컬 기본 브랜치 기준 통합 판정(1.24.0) | `landing()`은 통합 기준 ref(원격)를 본다 |
| lease가 무효이거나 보류 중인 coor의 조회·기록·`disable` 차단(1.11.0) | v1에 이슈 경계 규칙이 없다. 넣을 때의 조건은 18.3절 |
| 의존성 버전 고정, npm 게시, codebase-memory-mcp, 루트 `.env` 키 탐색 | npm 의존이 없다. 키는 `.fullops-squad/.env`와 환경 변수에서만 찾는다 |
| 1.23.0이 지운 입력(`--token`, `--handover`, `--host both`, `delay_minutes` 등) | 해당 기능·옵션이 없다 |
| fullops-meter mod 출력(#18) | 없다 |
| hook 명령이 버전별 캐시 경로를 직접 가리킴(1.4.4) | 고정 위치 실행본(10.3절). 디렉터리판만 판의 제약으로 플러그인 폴더를 쓴다 |
| PreToolUse 시점의 완료 확정(1.0.0) | 정산은 메시지 수신과 Orca 조회로만 확정한다 |
| 기각된 안: 실패 명령만 재실행, 레포 밖 쓰기 감지 hook, 대기 시간 뒤 추천안 자동 확정, 지표 임계값 게이트 등 | 어느 것도 없다. 지표는 규칙 조건에 쓰지 않는다 |
| `auto`·`delay` 자동 병합(미개방) | 섀도 판정 자체를 v1에서 뺐다 |
| 호환 잔재: `review_schema_version` 없는 기록 통과, Orca 응답 이중 해석, 누락 설정 기본값, grok camelCase | 읽지 않는다. `fullops.toml`은 필수 필드를 명시한다 |

## 12. 합격 기준과 비교 방법

### 12.1 방법

- 시나리오는 `scenarios/<묶음>/<이름>.toml` 파일 하나다. 상황과 단계를 의도 수준의 동사로 적는다. 상태 파일을 직접 주입하지 않는다(두 판의 형식이 다르다).
- 구동기는 둘이다. `rs` 구동기는 빌드한 `fullops`를 hook(stdin JSON)과 CLI로 실제 프로세스로 돌린다. `py` 구동기는 py-baseline 사본(v1.24.0)의 hook 명령과 스크립트를 돌린다. 둘 다 임시 git 레포(원격 bare 레포, 역할 워크트리 포함)를 실제로 만들고 같은 `fake-orca` 실행 파일을 `ORCA_CLI_COMMAND`로 쓴다. `fake-orca`는 각본 파일의 응답을 내고 받은 호출을 `calls.jsonl`에 남긴다.
- `py` 구동기는 Python 스크립트(`scenarios/drivers/py_driver.py`)다. 개발 시험 도구이고 배포본에 들어가지 않는다.
- 비교 항목: 단계별 판정 종류(통과·안내·경고·거부·차단), 거부·차단 뒤 탈출구를 실행하면 다음 단계가 통과하는가, 시나리오가 끝났을 때 사람 입력 없이 어디까지 갔는가(병합된 과제 수, lead가 받은 차단 수), 단계별 hook 시간. 문구와 파일 바이트는 비교하지 않는다. 예외로 거부·차단 문구에 규칙 ID와 탈출구 명령 이름이 있는지는 `rs`에서만 단언한다.
- 동사 대응: 예를 들어 `lead_dispatches`는 `py`에서 "`orca orchestration worker-start --run … --spec …`을 실은 PreToolUse 이벤트"이고 `rs`에서 "`fullops dispatch`". 1.24.0에서 hook 거부이고 새 판에서 CLI 거절인 경우는 "그 배정이 일어나지 않았다"는 같은 결과로 본다.
- `cargo xtask compare`가 표(시나리오, 단계, py 판정, rs 판정, py 흐름, rs 흐름, 결과, py 시간, rs 시간)를 `scenarios/report.md`로 낸다.

```toml
name = "미통합 결과를 두고 다른 과제를 배정한다"
origin = "flow-graph E4"
[setup]
mode = "coor"; roles = { dev = "worker", architecture = "design" }; remote = true
[[step]]
act = "worker_reports"; role = "dev"; key = "A-1"
[[step]]
act = "lead_dispatches"; key = "B-2"; role = "dev"
expect = { verdict = "deny", rule = "D-03", flow = "stopped" }
[[step]]
act = "lead_merges"; key = "A-1"
[[step]]
act = "lead_dispatches"; key = "B-2"; role = "dev"
expect = { verdict = "pass", flow = "continues" }
```

### 12.2 시나리오 목록

"1.24.0" 열이 `-`인 것은 `py` 구동기로 재현할 수 없거나 기준선이 없는 것이다.

| ID | 상황과 단계 | 1.24.0 | 새 판 | 흐름 기대 |
|---|---|---|---|---|
| A1 | 표식 없는 레포에서 다섯 이벤트 | 통과 | 통과 | 영향 없음 |
| A2 | 깨진 stdin, 모르는 도구 이름 | 통과 | 통과 | 영향 없음 |
| A3 | 깨진 설정 파일에서 Edit, 이어서 복구 명령 | 거부, 통과 | T-00 거부, 통과 | 복구 뒤 이어짐 |
| A4 | 세션 도중 모드 변경 뒤 도구 호출과 Stop | 거부, 차단 1회 | T-00, X-00 | 새 세션에서 이어짐 |
| B1 | coor: 기본 브랜치 세션 시작 | lead 안내 | lead 안내(S-01) | - |
| B2 | coor: 역할 브랜치 세션 시작 / coordinator 역할 브랜치 | worker 안내 / lead 안내 | 같음 | - |
| B3 | dev: 주 담당 브랜치 / 다른 등록 역할 | dev 주 담당 안내 / worker 안내 | 같음 | - |
| B4 | detached HEAD 세션, 등록된 리뷰 snapshot 세션 | coordinator 안내(오판) | `Other`·`Reviewer` 한 줄(DF-4) | 리뷰어가 lead 규칙에 안 걸림 |
| B5 | preamble을 받은 기본 브랜치 세션의 Stop | 미정산 차단 | X-03 | lead 규칙 미적용 |
| B6 | compact 뒤 SessionStart | 안내 없음 | 현재 인박스 한 줄(S-05) | 진행 복구 |
| C1 | 날것 `worker-start`(`--run` 없음) | 거부 | T-01 | `fullops dispatch`로 배정 |
| C2 | 날것 `worker-start --run`(조건 충족) | 통과 | T-01 거부(DF-1) | 감싼 명령으로 같은 배정 |
| C3 | PowerShell `& "C:\…\orca.exe" orchestration worker-start …` | 통과(인식 못 함) | T-01(DF-5) | - |
| C4 | `terminal send --text`에 지시서 경로 / lead의 21자 글 / 20자 글 | 거부 / 거부 / 통과 | T-02 / T-03 / 통과 | - |
| C5 | lead의 날것 `task-create`: 분류된 키 있음 / 없음 | 통과 / 거부 | 통과 / T-04 | - |
| D1 | route 없는 키 배정 / unresolved 키 배정 | 거부 | D-01 | 분류 뒤 이어짐 |
| D2 | 인박스의 키가 다른 배정 | 거부 | D-02 | - |
| D3 | 다른 키의 미통합 결과가 있는 배정 / 같은 키의 수정 배정 | 거부 / 통과 | D-03 / 통과 | 병합 뒤 이어짐 |
| D4 | 기본 브랜치를 안 받은 역할 워크트리에 배정 | 거부 | `dispatch`가 동기화한 뒤 통과(더 나음). 워크트리가 더러우면 종료 코드 2 | 사람 손 없이 이어짐 |
| D5 | 리뷰 배정: SHA 40자·별도 세션·깨끗한 snapshot / 구현자와 같은 터미널 | 통과 / 거부 | 통과 / D-06 | - |
| D6 | 한도 재배정 1·2회 / 3회 / 다른 tuple | 통과(지침만) | 통과 / D-07 종료 코드 3 / D-07 종료 코드 2(DF-8) | 3회째에 사람에게 |
| D7 | `subagent_level` off·lite·standard에서 호스트 내장 하위 에이전트 | 거부 / 조건부 / 통과 | T-08 | - |
| D8 | dispatch 받은 세션의 하위 배정: 자기 Run / 상위 Run | 통과 / (보고가 샘) | 통과 / D-04 거절(더 나음) | - |
| E1 | 인박스 직접 생성(Write) | 제목과 route가 맞으면 통과 | T-05(b) 거부(DF-9) | `task new`로 같은 결과 |
| E2 | 과제명 지시서 생성 / 인박스 비우기 / 다른 키로 덮어쓰기 | 거부 | T-05(a)·(c)·(d) | - |
| E3 | 같은 경우를 Codex `apply_patch`로 | 거부 | T-05 | - |
| E4 | lead가 design 과제의 타 역할 지시서 작성 | 거부 | H-02 거절, T-06 | 설계 역할 배정 |
| E5 | 설계 역할의 코드 파일 Edit / `.fullops-squad/` 아래 Edit | 거부 / 통과 | T-07 / 통과 | - |
| F1 | 보고서가 링크 안 한 PNG를 커밋 / 링크한 PNG / `raw/` | 거부 / 통과 / 통과 | T-09 / 통과 / 통과 | - |
| F2 | `qa-reports/A-1-FIX-test/` 생성(기존 `A-1-test` 있음) | 거부 | T-10 | 기존 보고서에 덧붙임 |
| G1 | worker: 코드 변경 뒤 lint 없이 Stop, 같은 턴에 다시 Stop | 차단, 경고 통과 | X-01, 경고 통과 | - |
| G2 | worker: lint 통과 뒤 기록 전용 커밋, Stop | 통과 | 통과 | - |
| G3 | worker: 작업 브랜치가 `lint.json`의 exclude를 넓힘 | 기준 시점 설정 적용 | 같음 | 스스로 완화 불가 |
| G4 | dev 직접 작업: lint 없이 Stop을 두 번 | 차단, 차단 | X-01, X-01 | - |
| G5 | dev 직접 작업: 400줄 변경, lint 통과, 리뷰 없음 / 면제 기록 / 수락 리뷰 | 차단 / 통과 / 통과 | X-02 / 통과 / 통과 | - |
| H1 | dispatch 받은 세션이 보고 없이 Stop, 다시 Stop | 차단, 경고 통과 | X-03, 경고 통과 | - |
| H2 | 날것 `worker_done` 전송 뒤 Stop(fake-orca가 정산을 보고) | 통과 | 통과 | - |
| H3 | Orca가 응답하지 않을 때 dispatch 받은 세션의 Stop | 통과 | 통과 | 갇히지 않음 |
| H4 | `fullops done`: lint 없음 / 완료 보고 빈칸 / 정상 | - | R-03·R-04 거절 / 본문 출력 | - |
| I1 | lead: 분류만 하고 Stop / `PLANS.md`에 보류로 적고 Stop | 차단 / 통과 | X-04 / 통과 | - |
| I2 | lead: worker 실행 중, 대기 없음 (Claude) / 백그라운드 대기 있음 | 차단 / 통과 | X-05(b) / 통과 | 대기 중에도 사용자 입력을 받음 |
| I3 | lead: 같은 상황 (Codex) | 차단(포그라운드 안내) | X-05(b) | - |
| I4 | lead: 미처리 `worker_done`이 있는데 Stop | 차단 | X-05(a) | - |
| I5 | 새 lead 세션(이전 세션이 연 Run이 열려 있음)의 Stop | 통과(세션이 Run을 모름) | X-05(더 나음) | 재시작 뒤에도 대기 의무 유지 |
| J1 | 미통합: 어디에도 없음 / 로컬 기본에만 있음 / 원격에 있음 | 차단 / 차단 / 통과 | X-06 / X-06 / 통과 | - |
| J2 | 로컬 기본 브랜치가 뒤처졌고 원격에는 들어가 있음 | 통과 | 통과 | 거짓 차단 없음 |
| J3 | 미통합 상태에서 Stop을 세 번 | 세 번 차단 | 세 번 차단 | - |
| J4 | hold 뒤 Stop / hold된 SHA가 원격에 들어간 뒤 `status` | 통과 / 자동 해제 | 같음(상한 없음) | - |
| J5 | 완료 보고 본문에 SHA 없음 | 기록의 SHA가 비고 `set-sha` 필요 | `done` 기록 또는 브랜치 tip으로 채움(더 나음) | 사람 손 없이 병합까지 |
| K1 | 준비된 행이 있는 lead의 Stop, 같은 턴에 다시, 다음 턴 끝 | 차단, 경고 통과, 차단 | X-07 같음 | 묻지 않고 배정 |
| K2 | 준비 조건 다섯을 하나씩 어긴 행 | 통과 | 통과 | - |
| K3 | `대기(사람: 배포)` 행 뒤에 준비된 행 | 뒤 행으로 차단 | 같음 | 사람 대기가 뒤를 안 막음 |
| K4 | 과제 3개(선행 1개, 사람 대기 1개)를 가짜 Orca로 끝까지 | - | - | 사람 입력 없이 병합된 과제 수가 1.24.0 이상, lead가 받은 차단 수가 1.24.0 이하 |
| L1 | `review need`: 399줄 / 400줄 / 15파일 / always 경로 / 문서만 | skip / ask / ask / ask / skip | 같음 | - |
| L2 | 대규모 변경 `merge`: 결정 없음 / 답 생략 뒤 waive / 답 진행 뒤 수락 리뷰 | (지침) | 종료 코드 3 / 통과 / 통과 | 사람 결정 한 번 |
| L3 | 구현자와 같은 세션의 리뷰 `check`, 미해결 high | 실패 | 실패 | - |
| L4 | 수정 커밋 뒤 이전 SHA의 리뷰로 `merge` | (알림) | M-04 거절 | 새 SHA 리뷰 |
| L5 | `merge`: fast-forward / 병합 커밋(트리 다름, lint 통과) / lint 실패 / 충돌 / push 실패 | (지침) | lint 재사용 / lint 1회 / 되돌림 / 종료 코드 3 / 종료 코드 4 | - |
| M1 | StopFailure 두 번(같은 dispatch) | escalation 1회 | F-01 1회 | - |
| M2 | lead 세션의 사람 입력 / Orca 알림 / dispatch 프롬프트 | 기록 / 미기록 / 미기록 | `human` / `machine:*` / 미기록 | 본문 미저장 |
| N1 | hook 중 강제 종료, 잠금 경합(병렬 PreToolUse 8개), `state.json` 파손 | - | 통과, 상태 온전 | - |
| N2 | `install` 중 hook 실행, 플러그인 갱신 뒤 SessionStart | - | 옛 실행본으로 완료, 다음 호출부터 새 실행본 | - |
| N3 | Python 레포에서 `migrate` → 판정 확인 → `--rollback` → `py` 구동기로 판정 확인 | - | 왕복 성공 | - |

M1·M2와 Codex의 Stop 동작은 1.24.0도 실제 세션에서 확인하지 못했다. 묶음 0이 실제 호스트에서 뜬 입력 JSON을 `scenarios/fixtures/host/`에 두고 그것을 정답으로 삼는다.

### 12.3 의도적 차이

| ID | 차이 | 이유 |
|---|---|---|
| DF-1 | 날것 `worker-start`를 조건과 무관하게 거부한다 | 선행 조건 검사와 상태 기록을 한 명령에 모은다. 결과(검사를 거친 배정)는 같다 |
| DF-2 | 지시서 `--level`, 보고서 분량, `raw/` 링크 알림이 편집 직후가 아니라 `dispatch`·`done` 시점에 나온다 | PostToolUse hook을 없애 도구 호출당 프로세스를 줄인다. 알림은 알림으로 남는다 |
| DF-3 | 현황판 단계 미갱신 확인이 없다 | 현황판을 없앴다(#18) |
| DF-4 | detached 세션은 lead가 아니고, 등록된 리뷰 snapshot은 `Reviewer`다 | 1.24.0의 오판(추정)을 고친다 |
| DF-5 | PowerShell `&` 호출과 역슬래시 경로의 Orca 명령을 인식한다 | 1.24.0의 알려진 한계. Windows 우선 |
| DF-6 | lite 위임의 수정 동사 휴리스틱이 없고 `Purpose`와 읽기 전용 선언만 본다. 폭·테마 캡처 요구 탐지가 없다 | 산문 휴리스틱의 오탐. 값어치가 측정되지 않았다 |
| DF-7 | 후속 요청을 완료 보고 산문에서 뽑지 않고 `done --followup`으로 받으며, `PLANS.md` 등재는 `merge`가 한다 | 산문 해석 제거, worker 브랜치의 `PLANS.md` 충돌 제거 |
| DF-8 | `done`이 lint 통과 없이 거절하고(`--no-checks`로 진행), `dispatch --reason limit`이 3회째와 다른 tuple을 거절한다 | 1.24.0은 알림·지침이다. 같은 게이트를 명령 안으로 당겼다. 사람 결정(H5) |
| DF-9 | 인박스 생성은 `task new`만 한다. Write로 만든 인박스는 거부한다 | front matter를 명령만 쓰게 해 과제 키 판독을 확정한다 |
| DF-10 | Stop에 여러 사유가 걸리면 하나의 차단에 모아 낸다 | Stop 프로세스가 하나다. 에이전트가 한 턴에 다 처리한다 |
| DF-11 | 재시도 판정이 `stop_hook_active`가 아니라 턴 기준이다 | 호스트 차이를 없앤다(7.6절). 결과는 같다 |
| DF-12 | caveman 주입, python3·orca 경로 안내, 긴 출력 줄이기가 없다 | 범위 |
| DF-13 | 새 lead 세션도 열린 Run의 대기 의무를 진다(I5). 기준선이 낡은 워크트리는 `dispatch`가 동기화한다(D4). SHA 없는 보고를 스스로 채운다(J5) | 흐름이 더 이어진다 |

### 12.4 합격 기준

- **같다**: 12.2의 모든 단계에서 판정 종류와 흐름(이어짐·멈춤)이 1.24.0과 같거나, 다르면 12.3에 ID와 이유가 있다. 목록에 없는 차이는 회귀다. 합격은 회귀 0건이다.
- **더 낫다**: 다음 가운데 하나를 수치로 보인다. (1) K4에서 사람 입력 없이 병합된 과제 수가 더 많거나 같고 lead가 받은 차단 수가 더 적다. (2) D4·I5·J5처럼 1.24.0에서 사람 손이 필요하던 단계가 없어졌다. (3) 2.2절의 성능 목표를 채운다.
- **성능 측정** `cargo xtask bench`: 이벤트별 대표 입력 12개(비활성 레포, 무관한 Bash, 무관한 Edit, 인박스 Edit, 날것 `worker-start`, `git commit`, worker Stop, lead Stop 4종(깨끗함·통합 대기 10건·준비된 행·열린 Run), SessionStart 2종)를 같은 fixture 레포에서 각 300회 실행하고 처음 20회를 버린 뒤 p50·p95·최대를 낸다. 시간은 프로세스 생성부터 종료까지의 벽시계다. PATH 앞에 둔 git·orca 대리 실행 파일이 호출 수를 센다. 같은 PC에서 py-baseline의 hook 명령을 같은 입력으로 돌려 나란히 적는다. 결과는 PC 사양과 함께 `docs/bench.md`에 둔다. CI는 Windows 러너에서 2.2절 목표의 두 배를 넘으면 실패한다(러너 편차 때문).
- **실환경 점검**(자동화하지 않는다. 전환 전 사람이 한 번): Windows의 Claude Code와 Codex 각각에서 설치 → `init` → 배정 → 완료 → 병합 → 다음 배정이 사람 입력 없이 이어지는지, 한도 오류 때 F-01이 가는지, Orca가 세션을 깨우는 입력이 기계로 분류되는지 본다. `fullops why` 출력으로 남긴다.

### 12.5 시험의 종류

- 통합 시나리오가 주력이다. 규칙 표의 모든 ID에 "걸림"과 "안 걸림" 시나리오가 하나 이상 있고, `fullops rules --json`으로 빠진 ID를 검사한다.
- 단위 시험은 넷만 둔다: 셸 분류(Bash·PowerShell 사례 표), `PLANS.md` 문법, front matter와 패치 적용, `review_need`.
- fullops-spec 레포의 사례 형식(stdout 필드 비교)은 쓰지 않는다. 두 판의 형식이 달라 같은 stdout을 낼 수 없다. 비교 자산은 이 레포의 `scenarios/`에 둔다.

## 13. 이행

### 13.1 공존

- 새 플러그인 이름은 `fullops`, Python판은 `fullops-squad`다. 한 PC에 둘 다 설치돼 있어도 된다.
- 각자 자기 표식만 본다(`fullops.toml` / `fullops.json`). 표식이 없는 쪽의 hook은 빈 판정을 낸다. 그래서 전환 단위는 레포다.

### 13.2 `fullops migrate --from-python [--dry-run]`

전제(하나라도 아니면 거절하고 남은 것을 낸다): 모든 역할 인박스가 비어 있다. Python판 통합 대기가 없다(hold는 허용). 이슈 자동 모드가 꺼져 있고 점유·답변 대기 잡이 없다. 작업 트리가 깨끗하다. `plugin_version`이 `1.24.0`이다.

| 순서 | 하는 일 | 되돌리기 |
|---|---|---|
| 1 | `<git-common-dir>/fullops/py-backup/`에 Python판 상태 폴더(`fullops-flow`, `fullops-integration`, `fullops-decisions`, `fullops-autonomy`, `fullops-issues`)를 복사한다. 원본은 건드리지 않는다 | 복사본 삭제 |
| 2 | `fullops.json`과 `orca-agents.md`의 라우팅 기준 표식 줄에서 `fullops.toml`을 만든다. `설계 역할` → `kind = "design"`, `coordinator 역할` → `"coordinator"`, `tester 역할` → `"tester"`, `제품 기획 역할`·`기술 계획 역할`이 둘 다 있으면 `routing = "split"`과 `tech_plan_role`. 역할 설명 줄은 `about`. `review_policy`는 `[review]`. 누락 값은 1.24.0의 실제 동작(`coor`·`standard`·`off`)으로 채운다. 읽지 못한 줄은 보고서에 낸다 | 파일 삭제 |
| 3 | `lint/lint.json`의 정규식 규칙을 하나씩 컴파일해 본다. 실패한 규칙을 보고서에 낸다(파일은 고치지 않는다) | - |
| 4 | 열린 결정(`fullops-decisions/D*.json`의 `status: open`)과 hold가 있는 통합 기록을 `state.json`으로 가져온다. 살아 있는 `PLANS.md` 행의 `선행:` 키 가운데 흐름 노드가 `report` 이상인 것을 `archive.jsonl`에 넣는다. 그 밖(닫힌 결정, 흐름 노드, 사람 턴, route 기록, 섀도 판정, 이슈 상태)은 가져오지 않는다 | `state.json`·`archive.jsonl` 삭제 |
| 5 | `PLANS.md` 문법 검사 결과를 보고서에 낸다(고치지 않는다) | - |
| 6 | `AGENTS.md`·`CLAUDE.md`의 관리 블록을 새 것으로 바꾸고, `.fullops-squad/.gitignore`에 필요한 줄을 더하고, 없는 템플릿 파일을 더한다 | 블록·줄 복원 |
| 7 | `git mv .fullops-squad/fullops.json .fullops-squad/fullops.json.py-1.24.0` | `git mv`로 복원 |

1~6은 Python판 동작에 영향이 없다. 7이 전환 순간이다. 커밋은 하지 않는다. 사람이 변경과 보고서를 보고 커밋한다. 그 커밋이 기본 브랜치에 들어가고 역할 워크트리가 받으면 그 워크트리도 넘어온다.

그대로 쓰는 것: `PLANS.md`, `handovers/logs/`, 규칙·산출물 문서, `lint/lint.json`, `review/rule.json`. 옮기지 않아서 생기는 결과: 지표는 전환 시점부터 다시 센다. 옛 route의 `requires_packet`은 사라진다. 옛 리뷰 폴더(`docs/evaluations/qa-reports/*-review/`)는 읽기 전용 이력으로 남는다.

### 13.3 `fullops migrate --rollback`

전제: 인박스가 비어 있고 통합 대기가 없다. 7 → 6 순으로 되돌린다. 전환 뒤 생긴 열린 결정과 hold를 화면에 낸다(사람이 Python판의 `decision.py add`·`integration.py hold`로 옮긴다). `fullops.toml`과 `state.json`은 남겨 둔다(Python판은 읽지 않는다). 커밋은 사람이 한다. 인박스의 front matter 인박스 전용 필드는 Python판이 무시한다.

### 13.4 jevpet 전환 순서

1. `cargo xtask compare`가 회귀 0건이고 실환경 점검을 마쳤다.
2. jevpet 사본에서 `migrate --dry-run`을 돌려 보고서(읽지 못한 표식 줄, 컴파일 실패 규칙, `PLANS.md` 문법 오류)를 정리한다.
3. 사람이 전환을 승인한다.
4. 과제 경계에서 jevpet의 인박스를 비우고 Python판 `integration.py settle`을 돌린다.
5. 새 플러그인을 설치하고 `fullops doctor`를 본다.
6. `fullops migrate --from-python` → 커밋 → push → 역할 워크트리 동기화 → 새 세션.
7. 과제 하나를 끝까지 돌리고 `fullops why`로 판정을 확인한다.
8. 문제가 있으면 `migrate --rollback`과 커밋. Python 플러그인은 지우지 않았으므로 바로 돈다.

Python 플러그인 제거는 2주 문제없이 쓴 뒤 따로 승인받는다.

## 14. 실행 중 Python·Node 의존이 남는 곳

| 항목 | 종류 | 왜 남는가 | 언제 없애는가 |
|---|---|---|---|
| hook과 CLI(`fullops` 바이너리) | 없음 | - | - |
| `scenarios/drivers/py_driver.py`와 py-baseline | Python (개발 시험) | 기준선 1.24.0을 돌려 비교해야 한다. 배포본에 없다 | 전환 뒤 Python 플러그인을 제거할 때 함께 지운다 |
| Orca 패널 `panel/` | TypeScript (Orca 플러그인 런타임) | Orca 패널의 형식이다(#18). hook·CLI와 별개로 배포한다 | 없애지 않는다. "완전 Rust"의 명시적 예외다 |
| `bin/fullops` 런처 | POSIX sh | 한 명령 문자열로 OS·CPU에 맞는 바이너리를 고른다. 디렉터리판과 첫 설치 전에만 거친다 | 호스트가 OS별 hook 명령을 지원하면 없앤다 |
| 분류 호출 | `curl` 프로세스 | 9.3절 | 이슈 자동 모드를 넣을 때 내장 HTTP와 함께 다시 정한다 |
| 프로젝트가 `lint.json`에 등록한 명령(npm, ruff 등) | 프로젝트 소관 | 프로젝트의 도구다 | - |
| Context7 MCP | 원격 HTTP | 로컬 Node 프로세스가 없다 | - |
| 웹·Unity 조작 테스트, 모션 렌더, 프런트 도구 설치 | Node·브라우저·Unity | 플러그인에 싣지 않는다. 쓰는 레포는 그 도구를 직접 부른다 | 범위 밖 |

## 15. 비평에 대한 판정

| 비평의 항목 | 판정 |
|---|---|
| 완료 보고 정본 규칙이 없다(C1) | 동의. 8.6절에 정했다. 다만 SHA 폴백은 원격 tip이 아니라 역할 브랜치의 로컬 ref를 쓴다. 같은 Git 공용 디렉터리라 push 없이 보이고 보고 시점에 더 가깝다. 그 앞에 `fullops done`이 남긴 기록을 1순위로 둔다 |
| 준비 커밋·push·워크트리 반영 순서가 없다(C2) | 동의. `dispatch` 2~5단계. 같은 키의 후속 배정은 대상에 열린 dispatch가 있어도 허용한다 |
| Codex 재시도 표시 폴백(C3) | 더 나아간다. 재시도 표시가 없는 호스트만 지문 비교를 쓰는 대신 두 호스트 모두 턴 기준으로 판정한다(7.6절). 호스트별 분기가 없어진다 |
| hook 부트스트랩(C4) | 동의. S-04 자동 설치와 플러그인 폴더 폴백(10.3·10.4절). 명령 문자열은 미정-M1 |
| dispatch 2차 결합의 오판(A) | 동의. 터미널 핸들이 같을 때만 묶는다 |
| 분류 후 미배정의 범위는 세션이어야 한다(A) | 동의. X-04는 세션의 `routed`만 본다 |
| 디렉터리판 유지(B) | 동의. `--in-place` |
| 분리 라우팅 유지(B) | 동의. `routing = "split"`과 `RouteClass::Product`·`Unresolved` |
| `review supersede`는 살아 있는 회복 경로다(B) | 동의. 8.9절 |
| 후보 밖 tuple 거절은 잘못이다(B) | 동의. tuple은 기록만 하고 후보와 대조하지 않는다 |
| `--no-ff`와 fast-forward 모순(C) | 동의. `git merge --no-edit`(ff 허용)이고 lint 재사용 기준은 tree 동일성이다(M-06·M-07) |
| Run 대기 의무는 세션이 아니라 레포 상태여야 한다(C) | 동의. `runs[].owner` |
| 기준 시점은 merge-base다(C) | 동의 |
| SQLite는 무겁다(C) | 동의. JSON 파일 하나 |
| `task-create` 전면 차단은 과하다(C) | 동의. T-04는 분류된 키만 요구한다 |
| 크레이트 5개 이하 | 동의. 4개와 dev 크레이트 |
| PostToolUse 알림 유지(C의 표) | 동의하지 않는다. 알림의 전달 시점을 `dispatch`·`done`·`wait`·`status`로 옮기고 PostToolUse hook을 없앤다. 도구 호출마다 프로세스가 하나 더 뜨는 비용이 편집 직후 알림의 값어치보다 크다 |
| `done`은 본문을 채워 주는 편의 기능이다 | 동의하고 한 걸음 더 간다. `done`은 보내지 않고 준비와 기록만 한다. 전송은 preamble 명령이다 |
| 사람이 정해야 하는 것: 대기 목록 정본, 분리 라우팅 | 문서 안에서 정했다. `PLANS.md` 유지는 되돌릴 수 있는 쪽이고(나중에 구조화 파일로 옮길 수 있다), 분리 라우팅은 둘 다 지원해 결정이 필요 없다 |

## 16. 미정 목록

| ID | 정하지 못한 것 | 지금의 가정 | 누가 언제 |
|---|---|---|---|
| M1 | Windows의 Claude Code와 Codex가 hook 명령을 어떤 셸로 실행하는가. `$HOME`·`${CLAUDE_PLUGIN_ROOT}`가 풀리는가. Codex에 플러그인 루트 변수가 있는가 | POSIX 셸(10.4절) | 묶음 0. 뼈대 동결 전. 결과로 `plugin/hooks/*.json`의 문자열만 바뀐다 |
| M2 | Codex의 hook 계약: 이벤트 이름, 입력 필드, 거부·종료 차단·문맥 주입·경고의 출력 형식, `apply_patch` 본문 형식, UserPromptSubmit이 오는가 | Claude Code와 같은 형태 | 묶음 0. 결과로 `host_codex.rs`와 `patch.rs`만 바뀐다 |
| M3 | Orca CLI: `worker-start`·`run-create`·`check`·ack·`worker-show`·`worker-list`·`send`·`terminal list`의 인자 이름과 JSON 응답 필드, preamble의 실제 글자, worker 셸에 `ORCA_TERMINAL_HANDLE`이 있는가 | 9.2절과 8.5절의 형태. preamble에는 `--dispatch-id <값>`이 있다(1.24.0이 실사용 중인 패턴) | 묶음 0이 `orca … --help`와 실제 응답을 fixture로 고정. 결과로 `fullops-ext::orca`만 바뀐다 |
| M4 | Orca 패널 플러그인이 Git 공용 디렉터리의 파일을 읽을 수 있는가, 프로세스를 띄울 수 있는가 | `status.json` 파일을 읽는다. 안 되면 `fullops status --json`을 부른다 | 묶음 0. 묶음 H 시작 전 |
| M5 | Orca와 Claude Code가 세션을 깨울 때 넣는 입력의 실제 형식 | P-02의 두 패턴 | 묶음 0이 실제 세션에서 채집. 결과로 `messages`가 아니라 `rules/prompt.rs`의 패턴 상수만 바뀐다 |
| M6 | Jev 요청·응답 스키마와 응답 검증 규칙 | `Judge` trait 뒤에 숨긴다 | 묶음 C가 TypeSafe 문서로 확정 |
| M7 | 산출물 인덱스(`docs/deliverables/README.md`) 표의 열 문법 | 템플릿의 표를 그대로 읽는다 | 묶음 D가 템플릿을 읽고 `docs/formats.md`에 적는다 |
| M8 | Anthropic 플러그인 디렉터리가 네이티브 바이너리가 든 플러그인을 받는가 | 받는다 | 레포 주인. 디렉터리판 제출 전 |
| M9 | 성능 목표의 확정 수치 | 2.2절 | 묶음 G의 첫 측정 뒤 레포 주인과 |

미정 항목이 가정과 다르게 확정돼도 5~8절의 타입·상태 형식·규칙 표·CLI 표면은 바뀌지 않도록 경계를 잡았다. 바뀌는 곳은 표의 "결과로" 뒤에 적은 파일이다.

## 17. 사람이 정해야 하는 것

되돌리기 어려운 것만 적는다. 각 항목의 기본안으로 구현을 시작하고, 뼈대 동결(18.1절) 전에 답이 다르면 해당 절을 고친다.

| ID | 결정 | 기본안 | 되돌리기 어려운 이유 |
|---|---|---|---|
| H1 | 이슈 #18의 "패널이 읽는 상태 파일 형식은 Python판과 호환" 조건 | 지운다. 패널과 봇은 `status.json`과 `fullops decision list --json`을 읽는다. 민감 정보 제외 원칙은 유지한다 | orca-helm이나 봇이 `fullops-decisions/D*.json`을 직접 읽고 있으면 전환과 같은 시점에 그쪽도 바꿔야 한다. 그 사용 여부를 확인해 달라 |
| H2 | 이름과 버전 | 플러그인·CLI 이름 `fullops`, 첫 버전 `2.0.0`, 레포 폴더 `.fullops-squad/` 유지, 홈 폴더 `~/.fullops/` | 마켓플레이스에 올린 이름은 바꾸기 어렵다. 공존 이행이 이름이 다르다는 데 기댄다 |
| H3 | 바이너리 유통과 서명 | 타깃별 바이너리를 배포 전용 브랜치(또는 전용 저장소)의 패키지 폴더에 커밋한다. 폐쇄망에서 내려받기 없이 설치된다. 서명은 하지 않고 시작한다 | 릴리스마다 저장소가 커지고 지울 수 없다. Windows SmartScreen과 macOS 공증이 첫 실행을 막을 수 있다(추측). 인증서 비용이 든다 |
| H4 | 이슈 자동 모드를 전환 뒤로 미룸 | 미룬다. jevpet이 지금 이 모드를 쓰고 있으면 전환 전에 만들어야 하고 일정이 크게 늘어난다. 사용 여부를 확인해 달라 | 미루면 전환 시점에 대기열·질문 outbox를 비워야 한다. 비우지 않으면 질문이 중복 게시될 수 있다 |
| H5 | 감싼 명령의 새 거절(DF-1, DF-8, DF-9) | 받아들인다 | "새 차단을 넣지 않는다"는 원칙에서 한 걸음 나간다. 스킬의 절차 문구가 감싼 명령 기준으로 다시 쓰이고, Orca CLI 인자가 바뀌면 `fullops`를 다시 배포해야 한다 |
| H6 | 버리는 기능 | grok·agy 지원, 웹·Unity 조작 테스트와 모션 렌더, OCR CLI, HTML 현황판과 단계 정의, caveman 주입, `session_cache` | 전환하는 순간 그 레포에서 사라진다. 그 호스트로 역할을 돌리는 레포는 전환할 수 없다 |
| H7 | 나중으로 미루는 기능 | 탐색(find·context·패킷·색인), 섀도 판정, 긴 출력 줄이기, `lint_tune` | 전환 뒤 다시 만들 때까지 없다. 옛 route의 `requires_packet`은 사라진다 |
| H8 | 전환 자체와 지표의 단절 | 13.4절의 순서. 사람 턴 기록은 옮기지 않는다 | 전환 전후의 지표를 직접 비교하려면 Python판 백업을 따로 집계해야 한다 |
| H9 | 이슈와 fullops-spec 정리 | #1·#2는 이 문서로 대체해 닫는다. #3~#13은 18.2절의 묶음으로 다시 묶는다. #9의 caveman은 닫는다. #17의 전환 조건은 "12.4절의 합격"으로 바꾼다. fullops-spec은 README에 이 레포를 가리키는 한 줄만 남기고 보관한다 | 이슈를 닫고 고치는 일은 사람이 한다 |

## 18. 구현 계획

### 18.1 먼저 한 사람이 까는 뼈대 (묶음 A)

다른 묶음은 A가 동결한 것만 믿고 시작한다. A가 끝났다는 확인: 아래가 통합 브랜치(`tourbut/dev`)에 있고 `cargo test`가 통과하며, `docs/formats.md`가 6절의 형식을 필드 단위로 옮겨 적었고, 빈 규칙 표로 `fullops hook claude`가 모든 입력에 `{}`를 낸다.

| 산출 | 내용 |
|---|---|
| 워크스페이스 | `Cargo.toml`, 네 크레이트와 `fullops-testkit`·`xtask`의 빈 골격, CI(세 OS 빌드와 시험) |
| 공용 타입 | `fullops-core::model` 전부(5절), `Known`, `Verdict`, `Refusal`, `Effect`, `Intent`, `HookInput` |
| 인터페이스 | `Facts`, `Git`, `Orca`, `Judge`, `Host` trait의 시그니처(10.2절). 구현은 비워 둔다 |
| 저장 | `fullops-store`의 `discover`, `load_config`, `read_state`·`update_state`(잠금·원자 교체·`status.json`), 세션 읽기·쓰기, `State`·`Session` serde 형식 |
| 파서 | `frontmatter`, `plan`(6.1.2절), `config` 검증 |
| 규칙 표 골격 | `RULES`에 모든 ID·이벤트·강도·탈출구를 넣고 `check`는 `Finding::None`. `fullops rules` |
| 문구 골격 | `messages.rs`에 규칙 ID별 구역 주석 |
| 시험 도구 | `fullops-testkit`: 임시 레포·원격·워크트리 빌더, `fake-orca`(각본 파일 규약), 시나리오 파일 형식과 `rs` 구동기의 동사 목록 |
| CLI 규약 | 종료 코드, `--json`, 거절 출력, 호출자 판정 함수 |

묶음 0(계약 확인)은 A와 동시에 한다. M1~M5를 실제 호스트와 Orca에서 확인해 `docs/contracts/`와 `scenarios/fixtures/`에 둔다. A의 동결 전에 M1~M3의 결과를 반영한다.

### 18.2 동시에 진행하는 묶음

| 묶음 | 범위 | 맡는 파일 | 기대는 인터페이스 | 끝났다는 확인 |
|---|---|---|---|---|
| 0 계약 확인 | 미정-M1~M5 채집 | `docs/contracts/*.md`, `scenarios/fixtures/host/`, `scenarios/fixtures/orca/` | 없음 | 호스트별 hook 입력 JSON 표본, Orca 응답 표본, preamble 표본이 fixture로 있고 16절의 가정과 다른 점이 문서에 있다 |
| A 뼈대 | 18.1절 | 18.1절 | 0의 M1~M3 | 18.1절 |
| B hook | 호스트 어댑터, 셸·패치 분류, 규칙 S·P·T·X·F의 `check`, `decide`, 효과 실행, 감시 스레드, `facts_real`, `why`, 추적 | `fullops-core/src/{shell,patch,derive}.rs`, `rules/*`, `fullops/src/hook/*`, `cmd/why.rs`, `fullops-store/src/{trace,turns}.rs` | A의 타입·`Facts`·저장. D의 `pass.json` 형식, E의 `accepted.json` 형식(A의 `docs/formats.md`에 있다) | 12.2의 A·B·C·E·F·G·H1~H3·I·J1~J4·K1~K3·M·N1 시나리오가 `rs`에서 통과. 규칙 ID마다 걸림·안 걸림 시나리오가 있다 |
| C 흐름 명령 | `route`(분류 포함), `task`, `plan`, `dispatch`, `wait`, `done`, `sync`, `env link`, `status`, Orca 구현, `curl` 호출 | `fullops/src/cmd/{route,task,plan,dispatch,wait,done,status,env}.rs`, `fullops-ext/src/{orca,jev}.rs`, `fullops-core`의 `check_dispatch`·`check_task_new`·`record_completion`·`metrics` | A. `Git` trait(E가 구현하기 전에는 testkit의 실제 git을 직접 부르는 임시 구현) | D1~D8, H4, J5, K4의 배정·대기 단계, DF-7 시나리오 통과. `dispatch`·`done`을 중간에 죽이고 다시 실행하는 시나리오 통과 |
| D lint·문서 | 명령 실행기, 내장 규칙, 정규식 규칙, `pass.json`·`result.json`, `doc check\|stamp` | `fullops/src/cmd/{lint,doc}.rs`, `fullops-core/src/lint_rules.rs`, `fullops-ext/src/proc.rs` | A | 내장 규칙 코드마다 검출·비검출 사례, 명령 선택 규칙(6번)의 시나리오, 타임아웃 때 프로세스 트리 종료(Windows·POSIX), G3 통과 |
| E 리뷰·통합 | `review` 전부, `merge`, `hold`·`resume`·`settle`·`completion set-sha`, `decision`, git 구현, `gc` | `fullops/src/cmd/{review,merge,hold,decision,gc}.rs`, `fullops-core/src/review.rs`, `fullops-ext/src/git.rs` | A. D의 `result.json` 형식 | L1~L5, J4, 결정 검증(SHA 불일치·만료·허용 밖·늦은 답) 시나리오 통과. `merge`를 M-06·M-09에서 죽이고 다시 실행하는 시나리오 통과 |
| F 설치·이행·배포 | `install`, `doctor`, `init`, `mode`, `upgrade`, `migrate`, `xtask dist`, 호스트 패키지와 hook 등록 파일, 런처, 스킬 8개 다시 쓰기, 내장 자산, `docs/build.md` | `fullops/src/cmd/{install,doctor,init,mode,upgrade,migrate}.rs`, `fullops/assets/`, `plugin/`, `xtask/src/dist.rs` | A. 스킬 문구는 C·D·E의 명령 표면이 굳은 뒤 | N2·N3 통과. 세 판 패키지가 만들어지고 `SHA256SUMS`가 맞다. 스킬 문서에 `python3`·스크립트 경로가 없다 |
| G 비교·성능 | `py` 구동기, 시나리오 작성(12.2 전부), `xtask compare`, `xtask bench`, `docs/bench.md` | `scenarios/`, `xtask/src/{compare,bench}.rs` | A의 testkit. 시나리오는 B~E와 함께 늘린다 | 12.4절: 회귀 0건, 성능 표 |
| H 패널 | `status.json`을 읽는 Orca 패널 | `panel/` | A의 `status.json` 형식, 0의 M4 | `orca plugin validate`·`check` 통과, 패널 화면 캡처. 전환을 막지 않는다 |

### 18.3 순서와 나중 묶음

1. 0과 A를 동시에 시작한다. A는 0의 M1~M3을 반영해 타입·형식·trait을 동결한다.
2. 동결 뒤 B, C, D, E, F(스킬 제외), G를 서로 다른 워크트리에서 동시에 진행한다. 묶음이 `fullops-core::model`·`facts`·`docs/formats.md`를 바꿔야 하면 A 담당에게 요청한다. `messages.rs`는 묶음마다 자기 규칙 ID 구역만 고친다.
3. C·D·E의 명령 표면이 굳으면 F가 스킬을 다시 쓰고, H를 시작한다.
4. G가 전체 비교와 성능 측정을 돌린다. 회귀를 각 묶음이 고친다.
5. 실환경 점검(12.4절) → 사람 승인 → 13.4절의 전환.

전환 뒤의 묶음과 넣을 때 지킬 조건:

- **이슈 자동 모드**: 자기 상태 파일을 Git 공용 디렉터리의 별도 폴더에 둔다. hook은 그 폴더가 없으면 관련 코드를 타지 않는다. 모드가 꺼져 있으면 보류 잡이 무관한 worker에 Orca 조회를 붙이지 않는다. 이슈 경계 규칙만 "모르면 거부"이고 7.8절 표에 두 번째 예외로 적는다. lease가 유효하지 않으면 조율자를 제한하지 않는다.
- **탐색(find·context·패킷·색인)**: 패킷 파일이 있는 과제만 `dispatch`·`done`·`review check`가 검사한다. route가 패킷을 요구하지 않는다. 결과는 추천이고 게이트가 아니다.
- **섀도 판정**: `review need` 결과에 등급 필드를 더하는 것으로 넣는다. 병합 흐름을 바꾸지 않는다.
- **긴 출력 줄이기**: PostToolUse hook을 다시 등록해야 하므로 절감량을 먼저 잰다.
