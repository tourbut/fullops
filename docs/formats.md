# fullops 파일 형식

이 문서는 [design.md](design.md) 6절의 형식과 8절·10절의 공용 규약을 필드 단위로 옮겨 적은 것이다. 구현 묶음은 설계 문서 전체를 다시 읽지 않고 이 문서로 형식을 본다. 설계 문서가 정본이고, 둘이 어긋나면 이 문서가 틀린 것이다.

- Rust 타입은 따로 적지 않으면 `fullops_core::model`에 있다. 이 문서의 표는 그 타입의 serde 형식과 같다.
- 시각은 모두 에폭 초(`i64`)다. 날짜 문자열은 사람이 읽는 두 곳에만 쓴다: 문서 front matter의 `updated`(`YYYY-MM-DD`)와 `PLANS.md` 배정 기록의 `YYYY-MM-DD HH:MM`. 둘 다 지역 시간대다(git이 아는 오프셋. 읽지 못하면 UTC).
- "쓰는 주체" 열이 비어 있으면 그 객체를 쓰는 주체와 같다.
- 형식을 바꿔야 하면 `fullops-core::model`과 이 문서를 함께 고친다. 묶음 A 담당에게 요청한다(설계 18.3절).
- 설계 6.1.1·6.1.4·6.1.5·6.2.1·6.2.3·6.3·10.7절의 예시 JSON·TOML이 타입으로 읽히고 다시 쓰면 같은 값이 되는지는 `crates/fullops-store/tests/store.rs`가 설계 문서에서 예시를 꺼내 확인한다.

## 1. 식별자와 공통 타입

| 타입 | 문법 | 비고 |
|---|---|---|
| `TaskKey` | `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$` | 과제 키 |
| `RoleId` | `^[a-z][a-z0-9_-]{0,31}$` | 역할 ID. `reviewer`도 이 문법에 맞는다 |
| `Sha` | 40자 소문자 hex | `Sha::short(7)`은 표시용, `Sha::short(12)`는 리뷰 폴더 이름 |
| `SessionId` | `[A-Za-z0-9_-]{1,100}` | 호스트 값은 `SessionId::from_host`가 밖의 글자를 `_`로 바꾸고 100자로 자른다. 빈 값은 해석 실패다 |
| `RunId`, `DispatchId`, `MessageId` | 비어 있지 않고 200바이트 이하이며 공백·제어 문자가 없다 | Orca가 준 값이고 형태에 기대지 않는다. `DispatchId`는 예약 기록에서 `pending:<request>`다 |
| `DecisionId` | `D` + 3자리 이상 숫자 | `DecisionId::from_number(next_decision)` |
| `HostId` | `claude` \| `codex` \| `grok` \| `agy` | hook 어댑터가 있는 호스트. `fullops hook`의 첫 인자다 |
| `Parallel` | `off` \| `read` \| `on` \| `ultra` | 병렬 방침(설계 8.15절) |
| `ModelTuple` | `{ "agent": 글, "model": 글, "effort": 글 }` | 세 값이 모두 있어야 한다. `agent`는 Orca의 에이전트 ID다(agy는 `antigravity`) |
| 경로 글 | `/` 구분, Windows의 `\\?\` 접두어와 끝의 `/` 없음 | `path_text()`로 만들고 `same_path()`로 비교한다(Windows는 대소문자 무시. 심볼릭 링크는 풀지 않는다) |
| 모르는 필드 | `Extra = BTreeMap<String, serde_json::Value>` | `#[serde(flatten)]`으로 받아 다시 쓴다 |
| 지문 | `sha256_hex(bytes)`의 소문자 hex 64자 | `brief`와 `prompt_fp`는 앞 16자다 |

파일에서 읽은 식별자가 문법에 안 맞으면 그 파일 전체를 읽지 못한 것으로 본다(hook은 `Unknown`).

능력 표 `caps_of(host) -> Caps`(설계 7.2절. 규칙은 호스트 이름이 아니라 이 값을 본다)와 `host_of_agent(orca_agent) -> Option<HostId>`:

| `Caps` | `claude` | `codex` | `grok` | `agy` |
|---|---|---|---|---|
| `stop_failure` | 참 | 거짓 | 참 | 참 |
| `background_wake` | 참 | 거짓 | 거짓 | 거짓 |
| `prompt_event` | 참 | 참 | 참 | 거짓 |
| `context_at_start` | 참 | 참 | 거짓 | 참 |
| `context_at_prompt` | 참 | 참 | 거짓 | 거짓 |
| `user_warn` | 참 | 참 | 거짓 | 거짓 |
| `subagent_tool` | `Agent` | `spawn_agent` | `spawn_subagent` | `invoke_subagent` |
| `workflow` | 참 | 거짓 | 거짓 | 거짓 |
| `orca_agent` | `claude` | `codex` | `grok` | `antigravity` |

## 2. 파일의 자리

경로는 `fullops_store::Repo`의 메서드로 얻는다. 문자열로 직접 잇지 않는다. 커밋되는 파일의 레포 상대 경로(슬래시 표기)는 `fullops_core::layout`이 만들고(`config_toml()`, `plans()`, `lint_json()`, `review_json()`, `inbox(role)`, `task_dir(key)`, `route(key)`, `request(key)`, `handover_log(key, role, round)`, `review_dir(key, head12)`, `qa_dir(key)`, `qa_report(key)`, `deliverables_index()`, `guide(name)`, `template(name)`, `archive_dir()`, 흐름 기록 판정 `is_record(path)`), `Repo`의 메서드는 `root.join(layout::…)`만 한다. 2.0.x까지의 자리 `.fullops-squad/`에서 옮기는 대응은 `layout::moved`·`layout::rewrite`다(design.md 13.6절).

| 자리 | 경로 | `Repo` 메서드 |
|---|---|---|
| 커밋되는 파일(design.md 6.1절) | `<작업 트리>/.fullops/` | `dir()` |
| | `fullops.toml`(`migrate --rollback` 뒤에는 `fullops.toml.rolled-back`으로 이름이 바뀌어 남는다) | `config_path()` |
| | `PLANS.md` | `plan_path()` |
| | `inbox/to_<역할>.md` | `inbox_path(role)` |
| | `tasks/<KEY>/<역할>.r<차수>.md` | `handover_log_path(key, role, round)` |
| | `tasks/<KEY>/route.json` | `route_path(key)` |
| | `tasks/<KEY>/reviews/<head 12자>/` | `review_dir(key, head)` |
| Git 공용 디렉터리(모든 워크트리 공유) | `<git-common-dir>/fullops/` | `shared()` |
| | `state.json`, `state.json.lock`, `state.json.tmp.<pid>`, `state.json.broken-<시각>` | `state_path()` |
| | `status.json` | `status_path()` |
| | `archive.jsonl` | `archive_path()` |
| | `turns.jsonl` | `turns_path()` |
| | `waiters/<run>.json` | `waiter_path(run)` |
| | `py-backup/`(Python판 상태 폴더의 사본. 전환할 때마다 새로 뜬다. 되돌리기 기록 `migrate.json`: `{"v":2,"at","changes":[{"path","before","after_sha"}]}`. `before`는 전환 전 글 전문이고 없던 파일은 null, `after_sha`는 전환이 쓴 글의 sha256이다. v 1의 `blocks`·`gitignore_added`·`removed_inboxes`·`delegation_replaced`는 쓰지 않는다. 설계 13.2·13.3절) | (묶음 F의 `migrate`) |
| | `requests/<KEY>.md`(비밀값 무늬가 있는 `route --request` 글. 커밋되지 않는다) | `route` |
| | `explore/<KEY>-find.json`, `<KEY>-documents-find.json`, `<KEY>-context.json`, `explore/history/`(탐색 결과와 덮어쓰기 전 사본. 커밋되지 않는다. 2.1.0에서 `docs/evaluations/jev/`에서 옮겼다) | `explore_dir()` |
| 작업 트리별 Git 디렉터리 | `<git-dir>/fullops/` | `local()` |
| | `sessions/<sid>.json`, `sessions/<sid>.json.lock` | `session_path(sid)` |
| | `sessions/<sid>.seen/<이벤트>` | `seen_path(sid, event)` |
| | `current.json` | `current_path()` |
| | `trace/<sid>.jsonl` | `trace_path(sid)` |
| | `lint/result.json`, `lint/pass.json`, `lint/logs/<명령>.log`, `lint/lock` | `lint_dir()` |
| | `search/code.json`, `search/documents.json`, `search/lock` | `search_dir()` |
| 사용자 홈 | `~/.fullops/` (Windows는 `USERPROFILE`, 그 밖은 `HOME`) | `fullops_store::home()`, `fullops_home()` |
| | `bin/fullops`(Windows는 `bin/fullops.exe`). 고정 위치 실행본(설계 10.3절) | `fullops_store::fixed_bin()` |

`waiters/<run>.json`의 파일 이름은 Run ID에서 `[A-Za-z0-9_.-]` 밖 글자를 `_`로 바꾼 것이다.

## 3. 커밋되는 파일

### 3.1 `fullops.toml`

활성 표식이자 `Config`다. 쓰는 주체는 `init`·`mode`·`upgrade`·`migrate`와 사람이다. 해석은 `fullops_store::parse_config`(글)와 `load_config`(작업 트리의 파일)가 한다. 파일에 적힌 그대로의 형태는 `fullops_core::config::RawConfig`다.

| 키 | 타입 | 필수 | 뜻 |
|---|---|---|---|
| `schema` | 정수 | 필수 | `2`여야 한다 |
| `applied_version` | 글 | 필수 | 이 레포에 적용한 fullops 버전. `upgrade`가 올린다 |
| `mode` | `"coor"` \| `"dev"` | 필수 | |
| `primary_role` | `RoleId` | dev 모드에서 필수 | 등록 역할이어야 한다 |
| `primary_branch` | 글 | dev 모드에서 필수 | |
| `routing` | `"legacy"` \| `"split"` | 필수 | |
| `tech_plan_role` | `RoleId` | split에서 필수 | 등록 역할이고 design 역할과 달라야 한다 |
| `test_level` | `"minimal"` \| `"lite"` \| `"standard"` \| `"full"` \| `"exhaustive"` | 필수 | 뒤로 갈수록 높다. 새 레포의 기본값은 `lite` |
| `parallel` | `"off"` \| `"read"` \| `"on"` \| `"ultra"` | 필수 | 병렬 방침. 새 레포의 기본값은 `on`. 1.24.0의 `subagent_level`은 받지 않는다(`migrate`가 옮긴다) |
| `[git] base` | 글 | 필수 | 기본 브랜치 |
| `[git] remote` | 글 | 선택 | 없으면 로컬 전용이다. 원격 이름의 꼴만 받는다(영문·숫자로 시작, 영문·숫자·`._-/`, `..`·`//`·끝 `/` 금지. URL·경로·빈 문자열은 오류). 쓸 때는 이 레포에 등록된 원격이어야 한다(설계 9.1절) |
| `[roles.<RoleId>] branch` | 글 | 필수 | 역할마다 고유하다 |
| `[roles.<RoleId>] kind` | `"worker"` \| `"design"` \| `"tester"` \| `"coordinator"` | 필수 | `design`·`coordinator`·`tester`는 각각 최대 1개 |
| `[roles.<RoleId>] about` | 글 | 선택 | 분류 질문에 쓰는 설명 |
| `[review] min_code_lines` | 정수 | 선택, 기본 400 | |
| `[review] min_code_files` | 정수 | 선택, 기본 15 | |
| `[review] always` | 글 목록(glob) | 선택, 기본 `[]` | |
| `[review] auto` | 불 | 선택, 기본 `false` | |
| `[code] extensions` | 글 목록 | 선택, 기본은 내장 목록(`fullops_core::config::CODE_EXTENSIONS`) | 점을 포함한 확장자 |
| `[wait] check_minutes` | 정수 | 선택, 기본 60 | |
| `[wait] ready_timeout_seconds` | 정수 | 선택, 기본 90 | |
| `[wait] log_limit` | 정수 | 선택, 기본 30 | |

모르는 키는 무시한다. `[roles]`가 없으면 역할 0개다.

정책 오류(`PolicyError`)의 종류는 셋이다.

| `PolicyKind` | `as_str()` | 언제 |
|---|---|---|
| `Unreadable` | `config_unreadable` | 파일을 읽지 못했다. TOML이 아니다. 필수 키가 없다. enum 값이 모르는 값이다. 역할 ID가 `RoleId` 문법에 맞지 않는다 |
| `Invalid` | `config_invalid` | `schema != 2`, 역할 브랜치 중복, kind 중복, dev 모드의 `primary_*` 누락이나 미등록 역할, split의 `tech_plan_role` 누락·미등록·design 역할과 같음, `remote`가 원격 이름의 꼴이 아님, `[git].base`·`primary_branch`·역할 `branch`가 `git check-ref-format --branch`의 꼴이 아니거나 `-`로 시작(검증 뒤 수정 R2-1) |
| `ModeChanged` | `mode_changed` | 세션 파일의 `mode_at_start`가 현재 `[mode, primary_role, primary_branch]`와 다르다(`config::mode_changed`) |

신원에 쓰는 값은 작업 트리의 파일에서, 검사 기준에 쓰는 값(`[review]`, `[code]`, lint 명령 선택의 `test_level`)은 merge-base 커밋의 blob에서 읽는다. 그 선택은 `baseline`이 한다.

| 함수 | 자리 | 결과 |
|---|---|---|
| `baseline(toml_text: Option<&str>) -> Baseline` | `fullops_store` | `Baseline { review, code, test_level, fell_back }`. 글이 없으면 내장 기본값(`ReviewPolicy::default()`, `CodeDef::default()`, `test_level = lite`)과 `fell_back = false`. 다른 묶음은 이 함수를 부른다. 읽히지 않거나 검증을 어기면 내장 기본값과 `fell_back = true`(`lint`·`review need`·`merge`가 "기준 시점 설정을 읽지 못해 기본값을 썼다" 한 줄을 낸다. hook은 알리지 않는다) |
| `config::baseline(parsed: Option<Result<Config, PolicyError>>) -> Baseline` | `fullops_core` | 위의 선택 규칙 자체다. core는 `toml` 크레이트를 쓰지 않으므로 글을 해석하는 쪽은 store에 있다 |

### 3.2 `PLANS.md`

쓰는 주체는 lead, `plan add|set`, `merge`다. 해석은 `fullops_core::parse_plan(text, template)`이 하고 결과는 `Plan { rows, delegation, parse_errors }`다. `fullops_store::read_plan(repo, template)`이 이 체크아웃의 파일을 읽는다. `template`은 내장 템플릿 `assets::PLANS_MD`다.

| 대상 | 규칙 |
|---|---|
| 과제 표 | 헤더 줄 다음 줄이 구분선(`\|---\|…`)이고, 헤더에 `과제` 또는 `키`가 든 열과 `상태`가 든 열이 모두 있는 표. 여럿이면 모두 읽고 파일 순서를 유지한다 |
| 코드 펜스 | ```로 시작하는 줄 사이의 줄은 읽지 않는다(줄 번호는 그대로다) |
| 열 찾기 | 키: `과제`·`키`. 상태: `상태`. 목표: `목표`·`결과`·`다음`·`내용`. 담당: `담당`·`책임`. 원천: `지시서`·`원천`. 한 헤더 칸이 여러 조건에 맞으면 키, 상태, 목표, 담당, 원천 순으로 처음 맞는 쪽의 열이 된다 |
| 칸 정규화 | `[글](주소)` → `글`, `*`와 `` ` `` 삭제, 앞뒤 공백 제거. `\|`는 구분자가 아니라 칸 안의 `\|`다 |
| 키 칸 | 빈 행은 버린다. `TaskKey` 문법에 안 맞으면 `parse_errors`에 줄 번호(1부터)를 넣고 버린다 |
| 상태 칸 | `보류(`로 시작 → `Held(괄호 안)`. `대기` 뒤 첫 비공백 글자가 `(`·`（` → `WaitHuman(괄호 안)`. 그 밖의 `대기…` → `Ready`. 나머지 → `Other(원문)`. 괄호 안은 짝이 맞는 닫는 괄호 전까지다(겹친 괄호를 센다). 닫는 괄호가 없으면 칸 끝까지다 |
| 선행 | 상태 칸의 `선행:` 뒤를 쉼표와 공백으로 나눠, 토큰 끝의 `.`·`)`·`）`를 떼고 `TaskKey` 문법에 맞는 것만 `after`에 넣는다 |
| 담당 | 담당 칸 맨 앞의 `[A-Za-z0-9_-]+`를 소문자로 바꾼 것. `RoleId` 문법에 안 맞으면 `role = None`이다(문법 오류로 세지 않는다) |
| `## 위임 범위` | 제목 다음부터 다음 `# `·`## ` 제목 전까지의 비어 있지 않은 줄 가운데 `template`의 줄(앞뒤 공백 제거 뒤 같음)이 아닌 것. 최대 10줄 |
| `## 완료 보고 후속 요청` | `- [ ] <역할>: <글> (<KEY>)` 줄. `merge`가 덧붙인다. `Plan`에는 담지 않는다 |
| `## 배정 기록` | `- <YYYY-MM-DD HH:MM> <KEY> → <역할> \| <agent>/<model>/<effort> \| <fresh\|retain> \| run <id> \| 재배정 <n> \| <이유>` 줄. `merge`가 덧붙인다. `Plan`에는 담지 않는다 |

`PlanRow`의 필드는 `key`, `goal`, `role`, `status`, `after`, `source`, `line`(1부터 세는 줄 번호)이다.

### 3.3 인박스 `inbox/to_<역할>.md`와 문서 front matter

front matter는 파일 맨 앞의 `---` 줄과 다음 `---` 줄 사이다. 읽는 문법과 쓰는 규칙은 1.24.0의 `deliverables.py`와 같다. 해석은 `fullops_core::parse_front_matter(text) -> Result<(FrontMatter, 본문 시작 바이트), FrontMatterError>`가 하고, 인박스 전용 필드는 `frontmatter::inbox_of`가 `Inbox`로 바꾼다. 산출물 문서도 같은 파서로 읽는다. 타입은 `fullops_core::frontmatter`에 있다: `FmValue { Text(String), List(Vec<String>), Map(BTreeMap<String, String>) }`, `FrontMatter { fields: Vec<(String, FmValue)> }`(파일에 적힌 순서).

| 읽기 | 규칙 |
|---|---|
| 머리 | 파일 머리의 BOM은 뗀다. 첫 줄이 `---`가 아니거나 닫는 `---`가 없으면 front matter가 없는 문서다(`FrontMatterError::Missing`). 줄 끝의 CR은 무시한다 |
| 줄 | `이름: 값`. 이름은 줄 맨 앞의 `[A-Za-z_][A-Za-z0-9_-]*`다. 이 꼴이 아닌 줄(빈 줄, 들여쓴 줄 포함)은 건너뛴다. 같은 이름이 두 번 나오면 뒤의 것이 이기고 자리는 처음 그대로다 |
| 글 값 | 값이 `"`·`[`·`{`로 시작하지 않으면 공백 뒤의 `#`부터 줄 끝까지를 주석으로 버리고, 앞뒤 공백과 양끝의 `'`·`"`를 뗀 글이다(`Text`) |
| `"…"` | 값이 `"`로 시작하고 `"`로 끝나면 JSON 문자열로 푼다(`Text`). 풀지 못하면 front matter 전체가 문법 오류다(`FrontMatterError::Syntax(줄 번호)`) |
| `[…]` | 값이 `[`로 시작하고 `]`로 끝나면 목록이다(`List`). JSON 문자열 배열로 풀리면 그것이고, 아니면 대괄호 안을 쉼표로 나눠 앞뒤 공백과 양끝 따옴표를 뗀 것이다(빈 원소는 버린다. `[57]`은 글 `57` 하나다) |
| `{…}` | 값이 `{`로 시작하면 JSON 객체(문자열 → 문자열)로 푼다(`Map`). 풀지 못하면 문법 오류다. 쓰는 곳은 산출물 문서의 `statuses` 하나다 |
| 그 밖 | `"`나 `[`로 시작하지만 위 꼴이 아니면 주석을 떼지 않고 양끝 따옴표만 뗀 글이다(1.24.0과 같다) |

| 쓰기(`FrontMatter::render`) | 규칙 |
|---|---|
| 글 값 | 다음 가운데 하나면 JSON 문자열로 감싸고 아니면 그대로 쓴다: 비었다, `:`·`#`·`[`·`]`·`{`·`}`·`,`·`"`·`'`·줄바꿈이 들었다, 첫 글자가 `-`·`?`·`&`·`*`·`!`·`\|`·`>`·`@`·`%`·백틱 가운데 하나다, 앞뒤에 공백이 있다, `true`·`false`·`yes`·`no`·`null`·`~`다(대소문자 무시) |
| 목록 | 원소 가운데 감싸야 하는 것이 있으면 JSON 배열(`["a, b", "c"]`. 원소 사이는 `, `), 아니면 `[a, b]` |
| 객체 | 키를 정렬한 JSON 한 줄(`{"D06": "review", "D07": "draft"}`. 구분은 `, `와 `: `) |
| 순서 | `fields`의 순서대로 쓴다. 문서 공통 필드의 순서(`id, title, status, statuses, updated, owner, tasks, upstream, downstream, summary`, 모르는 키는 그 뒤에 이름순)는 `doc stamp`(묶음 D)가 맞춘다 |

| 필드 | 타입 | 뜻 | 쓰는 주체 |
|---|---|---|---|
| `id` | 글 또는 목록 | 문서 ID. 인박스는 `handover-<역할>`. 산출물 여럿을 가진 문서는 목록이다 | `task new\|reopen`, `doc stamp` |
| `title` | 글 | 인박스는 `<KEY> — <목표>` | 같음 |
| `status` | `draft` \| `review` \| `approved` | 문서 상태(과제 상태가 아니다) | 같음 |
| `statuses` | 객체 | 산출물 ID → 상태. 산출물 하나의 상태는 `statuses[<ID>]`, 없으면 `status`다 | `doc stamp` |
| `updated` | `YYYY-MM-DD` | | 같음 |
| `owner` | 글 | | 같음 |
| `summary` | 글 | | 같음 |
| `tasks`, `upstream`, `downstream` | 목록 | 산출물 문서의 목록 필드(설계 8.12절) | `doc stamp` |
| `key` | `TaskKey` | 과제 키. 인박스 전용 | `task new\|reopen`만 |
| `role` | `RoleId` | 담당 역할. 인박스 전용 | 같음 |
| `goal` | 글 | 한 줄 목표. 인박스 전용 | 같음 |
| `base` | `Sha` | `changed()`의 시작점. 인박스 전용 | 같음 |
| `route` | `direct` \| `design` \| `product` | 분류. 인박스 전용 | 같음 |
| `from` | `lead` 또는 작성한 역할 ID | 인박스 전용 | 같음 |
| `round` | 정수 | `task reopen`마다 1 증가. 인박스 전용 | 같음 |
| `budget` | 정수 | 반복 예산(기본 2). 인박스 전용 | 같음 |
| `test_level` | `TestLevel` | 작성 시점의 설정 값(기록이다). 인박스 전용 | 같음 |
| `parallel` | `Parallel` | 작성 시점의 설정 값(기록이다. 판정과 안내는 작업 트리의 `fullops.toml`을 본다). 인박스 전용 | 같음 |

`fullops_store::read_inbox`는 파일이 없으면 `Yes(None)`, front matter가 없거나 문법 오류이거나 인박스 전용 필드 열 개(`key`~`parallel`) 가운데 하나라도 없거나 타입·어휘가 맞지 않으면 `Unknown("inbox_syntax")`를 돌려준다. 끝난 과제의 인박스는 `done`이 `tasks/<KEY>/<역할>.r<round>.md`로 `git mv` 한다.

인박스 본문의 내장 템플릿은 `assets::HANDOVER_TEMPLATE`다(8절 끝의 자산 표).

### 3.4 `tasks/<KEY>/route.json`

타입은 `Route`다. 쓰는 주체는 `route`와 `dispatch`(tuple 기록)다. `fullops_store::read_route`·`write_route`로 읽고 쓴다.

| 필드 | 타입 | 뜻 |
|---|---|---|
| `v` | 정수 | `1` |
| `key` | `TaskKey` | |
| `class` | `direct` \| `design` \| `product` \| `unresolved` \| `untracked` | `untracked`는 분류 없이 tuple만 기록한 경우(dev 직접 작업의 리뷰 키) |
| `role` | `RoleId` \| null | `unresolved`면 null |
| `deliverables` | 글 목록 | 산출물 ID(`D03` 등) |
| `origin` | `user` \| `bot` | 과제가 들어온 곳 |
| `decided_by` | `jev` \| `fallback` \| `manual` | |
| `reason` | 글 | `--reason` 값. 없으면 빈 글 |
| `request_file` | 글 \| null | `--request-file`로 준 레포 상대 경로(레포 안이고 경로 규칙을 통과한 파일만). `--request "<글>"`이면 그 글을 저장한 `.fullops/tasks/<KEY>/request.md`, 글에 비밀값 무늬가 있으면 `<git-common-dir>/fullops/requests/<KEY>.md`(절대 경로). 옛 실행본의 기록은 `--request`로 만들었으면 null이다(설계 6.1.4절) |
| `request_sha256` | 글 \| null | 요청 글의 sha256 |
| `probabilities` | 객체(이름 → 수) | `scope`, `role` |
| `thresholds` | 객체(이름 → 수) | `scope` 0.8, `role` 0.6, `deliverable` 0.5 |
| `tuples` | 객체(역할 ID 또는 `reviewer` → `{agent, model, effort, why}`) | 기록일 뿐이고 다음 과제의 기본값으로 쓰지 않는다. `agent`는 Orca의 에이전트 ID다 |
| `created_at` | 시각 | |
| `history` | `Route` 목록 | 재분류 전의 객체들. 각 원소의 `history`는 `[]`다 |

### 3.5 `tasks/<KEY>/reviews/<head 12자>/`

쓰는 주체는 `review`와 리뷰어다. `accepted.json`과 `waiver.json`은 묶음 E가 쓰고 묶음 B(X-02)가 읽으므로 타입(`Accepted`, `Waiver`)이 `model`에 있다. 나머지 파일의 타입은 묶음 E가 정의한다.

| 파일 | 필드 |
|---|---|
| `targets.json` | `v`(1), `key`, `base`, `merge_base`, `head`, `policy_sha256`(`config/review.json`의 sha256), `files[]`: `path`, `status`(`A`·`M`·`D`·`R`), `rule`(문장 \| null), `preset`(`"skipped"` \| null), `reason` |
| `result.json` | `v`(1), `key`, `base`, `head`, `reviewer_session`(글 \| null), `implementer_session`, `snapshot_path`, `snapshot_head`, `read_only`(true), `conclusion`(`accept` \| `reject` \| null), `files[]`: `path`, `status`, `review_status`(`reviewed` \| `skipped` \| null), `reason`. `findings[]`: `path`, `content`, `severity`(`critical`·`high`·`medium`·`low`), `resolved`, `start_line`, `end_line` |
| `report.md` | 리뷰 보고서 틀. 기계는 읽지 않는다 |
| `lint.json` | 같은 base·head의 `lint/result.json` 사본(5.6절) |
| `accepted.json` | `v`(1), `key`, `base`, `head`, `result_sha256`, `lint_sha256`, `reviewer_session`, `implementer_session`(글 \| null), `at`. 두 해시가 현재 파일과 같을 때만 유효하다 |
| `waiver.json` | `v`(1), `key`, `base`, `head`, `reason`, `decision`(`DecisionId`), `decided_by`(결정의 `--by`, 없으면 `via` 이름), `via`(`user` \| `bot` \| null. 없으면 null), `human_turn`(불. 결정을 연 뒤의 사람 턴을 확인하고 받은 답인가. 없으면 거짓), `need`(`Need` 객체), `at`. 덮어쓰지 않는다 |
| `superseded.json` | `by`, `old_result_sha256`, `new_result_sha256`, `at` |
| `abandoned.json` | `v`(1), `key`, `head`, `reason`, `at`. `review cleanup --abandon`이 쓴다 |

리뷰의 `base`는 `merge-base(refs/heads/<base>, head)`이고 `targets.json`의 `base`와 `merge_base`가 같은 값이다(dev 직접 작업처럼 merge-base가 `head` 자신이면 통합 기준 ref, 세션 anchor 순으로 넘어간다). `policy_sha256`은 lead 체크아웃 작업 트리의 `config/review.json`을 CRLF → LF로 맞춘 글의 sha256이고 파일이 없으면 빈 글의 해시다. `accepted.json`의 두 해시는 파일 바이트 그대로의 sha256이다. `result.json` 뼈대에서 미리 건너뛴 대상(`preset = "skipped"`)은 `review_status: "skipped"`와 고정 사유가 채워져 있다. 묶음 B(X-02)는 `accepted.json`의 형식과 `head`만 본다(해시 검증은 `review check`).

`Need` 객체: `required`(불), `action`(`skip` \| `ask` \| `review`), `reason`, `code_files`, `code_lines`, `always`(걸린 경로 목록), `base`, `head`, `merge_base`.

### 3.6 `config/lint.json`, `config/review.json`, 산출물 인덱스

1.24.0의 형식을 그대로 읽는다(변환하지 않는다). 사람이 쓴다. 전체 키와 타입은 묶음 D(`lint.json`)와 묶음 E(`rule.json`)가 정했다.

`config/lint.json`(해석은 `fullops_core::lint_rules::parse_lint_config(lint_json, template)`):

| 키 | 타입과 기본값 | 뜻 |
|---|---|---|
| `schema_version` | 정수, `1` | |
| `timeout_seconds` | 정수, 기본 600 | 명령에 값이 없을 때의 제한 시간 |
| `parallel_tests` | 불, 기본 `true` | test 명령을 동시에 실행한다 |
| `exclude` | glob 배열 | lint 제외 경로 |
| `size.severity` | `ERROR` \| `WARNING` | SIZE-001의 심각도 |
| `size.max_code_lines`, `size.max_doc_lines`, `size.max_diff_added` | 정수 | `max_diff_added`가 0이면 SIZE-002를 끈다 |
| `commands[].name` | 글, 필수 | |
| `commands[].run` | 글 배열, 필수, 비어 있지 않음 | 셸을 거치지 않는 인자 배열 |
| `commands[].cwd` | 글, 기본 `.` | 레포 안의 상대 경로 |
| `commands[].kind` | 글, 기본 `lint` | `test`면 시험 명령 |
| `commands[].level` | `TestLevel` | test 명령만 |
| `commands[].required` | 불, 기본 `false` | |
| `commands[].paths` | glob 배열 | 있으면 비어 있지 않아야 한다 |
| `commands[].timeout_seconds` | 정수 | 없으면 최상위 값 |
| `rules[].code`, `description`, `pattern`, `suggestion` | 글 | 레포 정규식 규칙(fancy-regex) |
| `rules[].file_extensions`, `exclude_patterns`, `exclude_paths` | 글 배열 | `exclude_paths`는 glob |
| `rules[].severity` | 기본 `WARNING` | |
| `rules[].multiline` | 불, 기본 `false` | |
| `rules[].enabled` | 불, 기본 `true` | |

읽는 규칙: 내장 템플릿 위에 최상위 키를 덮고 `size`만 한 단계 더 합친다. glob의 `*`는 `/`도 넘는다(1.24.0의 fnmatch와 같다). 형식이 어긋나면 `fullops lint`가 종료 코드 1로 끝난다.

`config/review.json`: `{ "include": …, "exclude": [glob], "rules": [{ "path": glob, "rule": 문장 }] }`. `include`는 읽지 않는다(리뷰 대상은 바뀐 파일 전부다). `exclude`에 맞는 파일은 `targets.json`에서 `preset = "skipped"`가 되고, `rules`에서 `path`가 처음 맞는 `rule` 문장이 그 파일의 `rule`이다.

내장 템플릿은 `assets::LINT_JSON`(1.24.0 템플릿 그대로)이다. `lint_excludes(lint_json, template_lint_json)`이 `exclude`를 `GlobSet`으로 만든다.

산출물 인덱스 `docs/deliverables/README.md`의 표 문법(묶음 D가 확정했다. 설계의 미정-M7을 닫았다). 내장 템플릿 `assets::DELIVERABLES_README`의 표 머리는 `| ID | 단계 | 산출물 | 원천 | 상태 |`다. `parse_deliverable_index(text) -> Vec<DeliverableRow>`가 읽는다.

| 대상 | 규칙 |
|---|---|
| 행 | `\|`로 시작하고 칸이 정확히 다섯이며 첫 칸이 `D`+숫자 두 자리인 줄. 같은 ID가 두 번이면 뒤 행이 이긴다. 칸 안의 `\|`는 다루지 않는다 |
| `id` | 첫 칸 |
| `summary` | 단계 칸(착수·분석·설계…). 인덱스에는 요약 칸이 없다 |
| `title` | 산출물 칸 |
| `status` | 상태 칸의 글 그대로(`미작성`·`범위 밖`·`draft`·`review`·`approved`) |
| `sources` | 원천 칸의 백틱 안 글마다 첫 공백 앞까지에 `.fullops/`를 앞에 붙인 레포 상대 경로. `/`로 끝나는 원천은 폴더이고 그 아래의 `.md` 파일 전부가 원천이다(`lint_rules`의 `owns(row, path)`가 그 판정이다). 백틱 밖의 글(예: '엔티티 절')은 버린다 |

### 3.7 탐색 결과 `<git-common-dir>/fullops/explore/`

2.1.0부터 탐색 결과는 2.0.x까지 커밋하던 `.fullops-squad/docs/evaluations/jev/`가 아니라 Git 공용 디렉터리의 `explore/`에 있다(`Repo::explore_dir`. 모든 워크트리가 함께 보고 커밋되지 않는다). 절 번호는 그대로 둔다. 패킷(`<KEY>-packet.json`)과 처리 기록(`<KEY>-packet-outcomes.json`)은 2.1.0에서 뗐다(`docs/design-changes.md` 9절).

세 파일(`<KEY>-find.json`, `<KEY>-documents-find.json`, `<KEY>-context.json`)은 같은 머리 필드를 가진다. `v`가 없거나 `1`이 아니면(1.24.0이 남긴 파일 포함) 읽는 쪽은 파일이 없는 것으로 본다. 쓰는 주체는 묶음 J의 `find`·`context`다. `history/`는 덮어쓰기 전 사본이다. `--force`의 덮어쓰기뿐 아니라 읽히지 않는 옛 파일(`v`가 다른 파일 포함)도 `history/`로 옮긴다.

| 머리 필드 | 타입 | 뜻 |
|---|---|---|
| `v` | 정수 | `1`(`EXPLORE_V`) |
| `key`, `role` | `TaskKey`, `RoleId` | 과제 키, 지시서를 받는 역할 |
| `round` | 정수 | 인박스 front matter의 `round` |
| `head` | 글 | 만든 시점의 `HEAD` 40자 |
| `instruction_sha256` | 글 | 지시서 지문(`explore::instruction_sha256`. 설계 6.1.5절의 다섯 단계) |
| `options` | 객체 | 그 명령의 입력 옵션(아래) |
| `input_sha256` | 글 | `{key, role, round, head, instruction_sha256, options}`를 키 정렬한 JSON의 sha256 |
| `created_at` | 시각 | |

`<KEY>-find.json`과 `<KEY>-documents-find.json`(타입 `fullops_core::explore::FindResult`):

| 필드 | 타입 | 뜻 |
|---|---|---|
| `options` | `{ scope, limit, max_calls, parser, offline, provider, cache_bypass }` | `scope`는 `code` \| `documents`. `provider`는 `typesafe` \| `openrouter` \| null(오프라인이거나 키가 없다) |
| `files`, `presented_files` | 정수 | 지도(색인의 `searchable` 항목)의 수, 그 가운데 Jev에게 물은 수(로컬 결과면 0) |
| `map_sha256` | 글 | 지도의 `[경로, 설명]` 쌍 배열의 sha256 |
| `candidates[]` | `{ path, summary, probability(수 \| null), basis, batch(정수 \| null), rank }` | `basis`는 `batch` \| `local`. 순서는 `(rank, batch)`다 |
| `seed_paths`, `remaining_candidates`, `unreadable_files` | 글 목록 | 레포 상대 경로 |
| `existence` | `{ status, found_probability(수 \| null) }` | `status`는 `found` \| `unclear` \| `not_confirmed` \| `unknown` |
| `semantic`, `partial`, `input_partial` | 불 | |
| `error` | 글 \| null | Jev를 부르지 못했거나 답을 쓰지 못한 이유. `offline`·`no key`·`request`·`response`(응답 검증 실패), `unsafe`(지시서 본문이나 요청 본문이 글 규칙에 걸려 보내지 않았다) |
| `usage` | `{ calls, input_tokens, output_tokens, cost(수 \| null) }` | `calls`는 실패한 호출도 센다 |
| `model` | 글 \| null | Jev를 한 번도 부르지 못했으면 null |

`<KEY>-context.json`(타입 `explore::ContextResult`):

| 필드 | 타입 | 뜻 |
|---|---|---|
| `options` | `{ paths, required, sources(경로 → sha256 \| "oversized" \| "unavailable"), offline, provider, cache_bypass }` | |
| `candidates[]` | `{ id, path, required, sent, unsent_reason(글 \| null), span({start_line, end_line} \| null), whole, signal({relevant, evidence, contradicts, injection} \| null), decision, decision_reason(글 \| null) }` | `decision`은 `keep` \| `suggest_omit` \| `conflict` \| `caution`(타입 `explore::Decision`). `unsent_reason`은 `too large`·`not utf-8`·`empty`·`sensitive text` |
| `required_paths` | 글 목록 | |
| `refused_paths[]` | `{ path, reason }` | `reason`은 `sensitive path`·`outside repo`·`symlink`·`not a file` |
| `semantic` | 불 | |
| `error`, `usage`, `model` | | find와 같다 |

## 4. Git 공용 디렉터리

### 4.1 `state.json`

타입은 `State`다. 열린 것만 담는다. 쓰기는 `fullops_store::update_state` 하나로만 한다(잠금 → 다시 읽기 → 수정 → `state.json.tmp.<pid>`에 쓰고 `sync_all` → `rename` → `status.json` 교체 → 잠금 해제). 읽기는 `read_state`가 잠그지 않고 한다.

- 파일이 없으면 빈 상태(`State::default()`)다.
- JSON으로 읽히지 않으면 `Unknown("state_broken")`, `v`가 `STATE_V`(1)보다 크거나 JSON 객체로는 읽히는데 타입으로 풀리지 않으면(모르는 enum 값, 타입 오류) `Unknown("state_newer")`다. `update_state`는 각각 `StoreError::Corrupt("state_broken")`·`Corrupt("state_newer")`를 돌려주고 파일을 건드리지 않는다.
- 모든 객체는 모르는 필드를 보존한다(`extra`, `#[serde(flatten)]`). `ModelTuple`만 예외다.
- 값이 없는 필드는 `null`로 쓴다. 읽을 때는 없는 필드를 `null`로 본다. 예외: `dispatches[].round`, `decisions[].repository`·`drop_reason`은 값이 있을 때만 쓴다.

최상위:

| 필드 | 타입 | 뜻 | 쓰는 주체 |
|---|---|---|---|
| `v` | 정수 | `1` | `update_state` |
| `seq` | 정수 | 쓸 때마다 1 증가 | `update_state` |
| `runs` | `Run` 목록 | | `dispatch`, `wait`, Stop의 X-05 효과 |
| `dispatches` | `Dispatch` 목록 | | `dispatch`, `wait`, `done`, hook 효과 |
| `completions` | `Completion` 목록 | | `wait`·T-11, `done`(직접 작업), `merge`·`hold`·`settle`, X-06 효과 |
| `messages` | `Message` 목록 | 처리할 메시지만. 본문은 넣지 않는다 | `wait`·T-11 |
| `tasks` | 객체(`TaskKey` → `TaskCounters`) | | `dispatch`, `task reopen` |
| `sync_pending` | 객체(`RoleId` → `Sha`) | 그 역할 워크트리에 반영해야 할 기본 브랜치 SHA | `dispatch`, `merge`, `sync` |
| `snapshots` | `Snapshot` 목록 | | `review`, S-03 효과 |
| `decisions` | `Decision` 목록 | | `decision`, `merge`(M-04), `dispatch`(D-07) |
| `next_decision` | 정수 | 다음 결정 ID의 번호. 없으면 1 | `decision add` |

`Run` (`runs[]`):

| 필드 | 타입 | 뜻 |
|---|---|---|
| `id` | `RunId` | |
| `owner` | `"lead"` \| `"dispatch:<DispatchId>"` | dispatch 받은 세션이 만든 하위 Run은 뒤쪽이다. 타입은 `RunOwner` |
| `coordinator` | 글 \| null | 그 Run이 Orca에서 묶여 있는 세션의 세션 핸들. `dispatch`가 Run을 만들 때 호출한 세션의 핸들을 쓰고, `wait`가 `run-use`로 다시 묶으면 그 세션의 핸들로 바꾼다. Orca가 깨우는 대상이다(X-05) |
| `opened_at` | 시각 | |
| `closed_at` | 시각 \| null | 미정산 dispatch와 미처리 메시지가 없으면 X-05가 닫는다 |

`Dispatch` (`dispatches[]`):

| 필드 | 타입 | 뜻 | 쓰는 주체 |
|---|---|---|---|
| `id` | `DispatchId` | Orca의 dispatch ID. Orca 응답을 받기 전의 예약 기록은 `pending:<request>`다 | `dispatch` |
| `request` | 글 | `req_` + 16진수 16자. `worker-start --retry-request`의 값 | `dispatch` |
| `task_id` | 글 \| null | Orca task ID. 예약 기록은 null | `dispatch` |
| `run` | `RunId` | | `dispatch` |
| `key` | `TaskKey` | | `dispatch` |
| `role` | `RoleId` | | `dispatch` |
| `purpose` | `build` \| `fix` \| `review` \| `research` | | `dispatch` |
| `tuple` | `ModelTuple` | 항상 tuple의 값이다(Orca에 넘긴 `--effort`가 `ultracode`여도) | `dispatch` |
| `terminal` | 글 \| null | worker 터미널의 핸들. 채팅 방식 worker는 null | `dispatch` |
| `worktree` | 경로 글 | 대상 워크트리(리뷰는 snapshot, 하위 배정은 부모의 워크트리) | `dispatch` |
| `session_mode` | `fresh` \| `retain` | | `dispatch` |
| `reason` | null \| `"limit"` | | `dispatch` |
| `started_at` | 시각 | | `dispatch` |
| `bound_session` | `SessionId` \| null | 이 dispatch를 묶은 세션 | P-01 효과 |
| `sub_of` | `DispatchId` \| null | 하위 배정이면 부모 dispatch의 ID | `dispatch` |
| `brief` | 글 \| null | 하위 배정에 준 `--brief` 글의 sha256 앞 16자. 글은 저장하지 않는다 | `dispatch` |
| `round` | 정수(값이 있을 때만) | 작업 배정이 보낸 인박스의 차수. 인박스 없이 받는 배정과 이 필드가 생기기 전의 기록에는 없다. `derive::last_failed`가 같은 차수의 다른 배정의 성공 보고를 끝난 차수로 칠 때 쓴다 | `dispatch` |
| `reported` | `Reported` \| null | `fullops done`의 기록 | `done` |
| `settled` | `Settled` \| null | null이면 미정산이다 | `wait`·T-11, X-03 효과, `gc` |
| `limit_escalated` | 불 | F-01이 escalation을 보냈다. escalation 메시지는 이미 `done`으로 정산된 dispatch의 `settled`를 바꾸지 않고 이 값만 쓴다 | F-01 효과, `wait` |

`Dispatch`의 판정 메서드: `is_work()`(Build·Fix), `is_reservation()`(`id`가 `pending:`으로 시작한다), `is_sub()`(`sub_of`가 있다), `counts()`(예약 기록도 하위 배정도 아니다). 예약 기록은 dispatch로 세지 않는다: X-03·X-05·D-10·R-11·`stage_of`·`State::sub_open`·`status.json`이 건너뛴다. 하위 배정은 completion을 만들지 않고 `stage_of`·D-03·D-10·지표의 "이어진 배정 수"·`status.json`의 `tasks`에서 건너뛴다.

`Reported`: `branch`(글), `sha`(`Sha`, `done` 시점의 HEAD), `lint_head`(`Sha` \| null, `--no-checks`로 끝냈으면 null), `followups[]`(`{ "role": RoleId, "text": 글 }`), `at`(시각).

`Settled`: `how`(`done` = worker_done 수신 또는 Orca 조회로 확인, `escalated` = escalation 수신, `lost` = worker 사망 감지), `outcome`(`succeeded` \| `failed` \| null. `how`가 `done`이 아니면 null), `message`(`MessageId` \| null), `at`(시각).

`Completion` (`completions[]`):

| 필드 | 타입 | 뜻 | 쓰는 주체 |
|---|---|---|---|
| `id` | `MessageId` | 그 `worker_done` 메시지의 ID. dev 직접 작업은 `direct:<sha 앞 12자>` | `wait`·T-11, `done` |
| `dispatch` | `DispatchId` \| null | 직접 작업이거나 dispatch를 못 찾았으면 null | 같음 |
| `key` | `TaskKey` \| null | 과제 키를 못 찾았으면 null. lead가 `completion set-sha --key`로 채운다 | 같음 |
| `role` | `RoleId` \| null | dispatch를 못 찾았으면 null | 같음 |
| `branch` | 글 \| null | | 같음 |
| `sha` | `Sha` \| null | null이어도 통합 대기로 센다 | 같음, `completion set-sha` |
| `sha_source` | `done` \| `body` \| `branch_tip` \| `manual` \| null | `done` = `fullops done` 기록, `body` = 메시지 본문, `branch_tip` = 역할 브랜치 로컬 ref, `manual` = `completion set-sha` | 같음 |
| `direct` | 불 | dev 주 담당 직접 작업의 기록. 통합 차단(X-06, D-03)에서 빼고 단계와 지표에만 쓴다 | `done` |
| `outcome` | `succeeded` \| `failed` | 타입은 `WorkOutcome`. `failed`는 통합 대기로 세지 않는다 | `wait`·T-11 |
| `received_at` | 시각 | | 같음 |
| `hold` | `Hold` \| null | 보류 | `hold` |
| `released_hold` | `Hold` \| null | 재개했거나 통합된 보류. 지우지 않고 여기로 옮긴다 | `resume`, `merge`·`settle`·X-06 효과 |
| `landed_at` | 시각 \| null | 통합이 처음 확인된 시각. 한 번 쓰면 다시 계산하지 않는다 | `merge`, `settle`, `status`, X-06 효과 |
| `drift` | 정수 \| null | 통합 시점에 기본 브랜치가 보고 SHA 뒤의 역할 브랜치 커밋(기록 전용 제외)을 함께 담았으면 그 수 | 같음 |

`Hold`: `reason`, `owner`, `resume`(재개 조건) 세 글과 `at`(시각). 셋을 다 가져야 한다.

통합 대기(`Completion::is_pending`, `State::pending`): `direct == false`, `outcome == succeeded`, `hold == null`, `landed_at == null`.

하위 작업 남음(`State::sub_open(parent: &DispatchId) -> bool`. 설계 6.6절. 완성본): `owner == "dispatch:<parent>"`이고 `closed_at`이 없는 Run에 `settled`가 없는 dispatch(예약 기록 제외)나 `acked_at`이 없는 메시지가 있다. Stop의 X-01·X-03은 참이면 건너뛰고 `done`의 R-11은 참이면 거절한다. dispatch가 묶이지 않은 세션과 `state.json`을 읽지 못한 호출에서는 거짓으로 본다.

`Message` (`messages[]`): `id`(`MessageId`), `delivery`(글 \| null. 옛 Orca가 준 전달 ID이고 보통 null), `run`(`RunId`), `type`(글. `heartbeat`·`status`가 아닌 모든 종류: `worker_done`, `question`, `escalation`, `dispatch`, `merge_ready`, `handoff`, `decision_gate`, 모르는 종류), `dispatch`(`DispatchId` \| null), `received_at`(시각), `acked_at`(시각 \| null, null이면 미처리). `acked_at`은 fullops 자신의 처리 표시이고 쓰는 곳은 넷이다: `merge`의 M-10, `hold`, `landed_at`을 쓰는 모든 곳, `wait --ack`.

처리 표시를 누가 하는지는 `ack_by(state, message_id) -> AckBy`가 `state.json`만 보고 정한다. `merge`: 종류가 `worker_done`이고 `completions[]`에 `id`가 그 메시지 ID인 통합 대기 기록이 있다. `ack`: 그 밖 전부. 깨움 판정은 `orca_wakes(session_handle, coordinator) -> bool`(둘 다 있고 같으면 참)이다.

`TaskCounters` (`tasks[<KEY>]`): `rounds`(정수, `task reopen`마다 1 증가), `budget`(정수, 기본 2), `limit_redispatch`(정수, `dispatch --reason limit`마다 1 증가), `last_tuple`(`ModelTuple` \| null).

`Snapshot` (`snapshots[]`):

| 필드 | 타입 | 뜻 |
|---|---|---|
| `review` | 글 | `<KEY>@<head 앞 12자>` |
| `path` | 경로 글 | `<레포 부모>/.fullops-review-<무작위 8자>` |
| `head`, `base` | `Sha` | |
| `state` | `creating` \| `active` \| `removed` \| `abandoned` \| `missing` | |
| `implementer_session` | 글 \| null | `review snapshot`이 정한 구현자 세션(`--implementer-session` → 보고의 dispatch의 `bound_session` → dev 직접 작업의 자기 세션). `review check` 4번이 이 값과 견준다. 정하지 못하면 snapshot이 거절된다(RV-00) |
| `reviewer_dispatch` | `DispatchId` \| null | |
| `owner` | `"lead"` \| `"dispatch:<DispatchId>"` | |
| `created_at` | 시각 | |
| `hold` | `{ "reason": 글, "at": 시각, "abandon"?: 글 }` \| null | 정리하지 못한 사유(타입 `SnapshotHold`). `abandon`은 `--abandon`으로 청했을 때의 중단 사유다. `hold`가 남은 snapshot은 `merge`·`settle`·`review prune`이 다시 정리해 본다 |

`Decision` (`decisions[]`, 뜻은 설계 8.10절):

| 필드 | 타입 | 뜻 |
|---|---|---|
| `id` | `DecisionId` | |
| `status` | `open` \| `answered` \| `dropped` | |
| `key` | `TaskKey` \| null | |
| `question` | 글 | |
| `options` | 글 목록 | |
| `blocks` | 글 목록 | 멈출 과제 키 또는 메시지 ID |
| `asked_at` | 시각 | |
| `origin` | `user` \| `bot` | |
| `sha` | `Sha` \| null | 있으면 답도 같은 SHA를 내야 한다 |
| `risks` | 글 목록 | `--risk` |
| `allow` | 글 목록 | 있으면 답하는 사람이 이 안에 있어야 한다 |
| `expires_at` | 시각 \| null | `sha`가 있으면 `asked_at + 24시간` |
| `answer` | 글 \| null | |
| `decided_by` | 글 \| null | `--by` |
| `via` | `user` \| `bot` \| null | |
| `closed_at` | 시각 \| null | |
| `repository` | 글 | `--repository`. 값이 있을 때만 있는 필드다 |
| `drop_reason` | 글 | `drop --reason`. 값이 있을 때만 있는 필드다 |
| `human_turn` | 불 | 답한 결정에만 있다. 결정을 연 뒤의 사람 턴을 확인하고 받은 답이면 참(DC-04. 설계 8.10절). 타입의 필드가 아니라 `extra`에 쓴다 |
| `card` | 불 | 질문 카드로 물은 결정에만 있다(hook의 T-16이 쓴다). `sha`가 없는 결정은 이 표시가 있으면 사람 턴 없이도 답을 받는다(설계 8.10절). `extra`에 쓴다 |

### 4.2 `archive.jsonl`

추가만 한다(`fullops_store::append_archive`). 쓰는 주체는 `merge`·`settle`·`gc`·`migrate`다. 타입은 `ArchiveLine`이고 줄은 둘 가운데 하나다.

| 줄 | 필드 |
|---|---|
| 과제 | `key`(`TaskKey`), `role`(`RoleId` \| null), `sha`(`Sha` \| null), `received_at`(시각), `landed_at`(시각 \| null), `rounds`(정수), `dispatches[]`(`purpose`, `started_at`, `reason`, `sub`(불. 하위 배정이었다)), `direct`(불) |
| 결정 | `{"decision": <Decision 객체>}` |

`stage_of`의 `archive` 인자는 `landed_at`이 있는 과제 줄의 키 집합이다. `fullops_store::archive_keys(repo) -> Known<BTreeSet<TaskKey>>`가 줄마다 `key`와 `landed_at`만 뽑아 만든다(줄 전체를 타입으로 풀지 않는다. 풀리지 않는 줄은 건너뛴다). 줄 전체를 읽는 `read_archive(repo)`는 `status`와 `merge`가 지표를 계산할 때 잠금 밖에서만 부른다.

### 4.3 `turns.jsonl`

`{"at": 시각, "sid": SessionId, "kind": 글}` 한 줄. 타입은 `Turn`이다. `O_APPEND`로 `write` 한 번에 쓰고 잠그지 않는다. 줄은 200바이트 이하다.

| `kind` | `TurnKind` | 뜻 | 쓰는 주체 |
|---|---|---|---|
| `human` | `Human` | 사람 입력. 사람 턴으로 세는 것은 이것뿐이다 | P-02 |
| `machine:orca-message` | `OrcaMessage` | Orca가 세션에 넣은 알림 | P-02 |
| `machine:task-notification` | `TaskNotification` | 호스트의 백그라운드 작업 알림 | P-02 |
| `subagent` | `Subagent` | 호스트 서브에이전트 도구 호출 하나 | T-13(lead가 아닌 세션도 쓴다) |
| `workflow` | `Workflow` | Workflow 도구 호출 하나 | T-13 |

### 4.4 `status.json`

타입은 `Status`다. `update_state`가 같은 잠금 안에서 다시 만든다(다른 곳에서 쓰지 않는다). Orca 패널과 봇이 읽는다. token, 이슈 본문, 댓글, 프롬프트는 넣지 않는다. 교체가 실패해도 `update_state`는 성공을 돌려준다.

필드는 두 갈래다. **state에서 바로 나오는 것**은 쓸 때마다 방금 쓴 `State`와 작업 트리의 `fullops.toml`, 이전 `status.json`만으로 다시 만든다(`turns.jsonl`·`archive.jsonl`을 읽지 않고 프로세스를 띄우지 않는다). **유도값**(`ready`, `pending[].landing`, `metrics`)은 `update_state(repo, wait, derived, f)`의 `derived: Option<&Derived>`로 넘겨받은 것만 바꾸고, 넘겨받지 않은 것은 이전 `status.json`의 값을 잇는다.

`Derived { ready: Option<Vec<TaskKey>>, landing: BTreeMap<String /* completion ID */, Landing>, metrics: Option<Metrics> }`. 넘기는 쪽: `ready`는 lead 문맥(lead의 hook이나 lead가 실행한 CLI)이 lead 체크아웃에서 계산해 넘긴다. `landing`은 그 값을 계산한 호출(X-06의 효과, `status`, `settle`, `merge`)이 넘긴다. `metrics`는 `fullops status`와 `merge`(M-13)가 잠금 밖에서 `turns.jsonl`·`archive.jsonl`을 읽어 계산해 넘긴다. hook은 `metrics`를 계산하지 않는다.

| 필드 | 갈래 | 타입 | 뜻 |
|---|---|---|---|
| `v` | state | 정수 | `1` |
| `generated_at` | state | 시각 | |
| `mode` | state | `coor` \| `dev` \| null | 설정을 읽지 못했으면 이전 값, 그것도 없으면 null |
| `version` | state | 글 | 이 파일을 쓴 실행본의 버전 |
| `tasks[]` | state | | `state.json`에 하위 배정이 아닌 dispatch(예약 기록 제외)나 completion이 있는 키마다 하나 |
| `tasks[].key` | | `TaskKey` | |
| `tasks[].role` | | `RoleId` \| null | 가장 늦은 dispatch의 역할, 없으면 가장 늦은 completion의 역할 |
| `tasks[].stage` | | `assigned` \| `reported` \| `reviewing` \| `merged` | `stage_of(key, state, 빈 집합, true)`. `archive.jsonl`을 읽지 않으므로 `done`은 나오지 않는다 |
| `tasks[].rounds`, `tasks[].budget` | | 정수 | `tasks[<KEY>]`의 값. 없으면 0과 2 |
| `tasks[].sha` | | 글 \| null | 가장 늦은 completion의 SHA 앞 7자 |
| `tasks[].held` | | 불 | 그 키의 completion에 hold가 있다 |
| `dispatches[]` | state | | 예약 기록이 아닌 dispatch마다 `key`, `role`, `purpose`, `agent`, `model`, `started_at`, `settled`(불), `sub`(불. 하위 배정이다) |
| `pending[]` | state | | 직접 작업이 아니고 성공했고 통합되지 않은 completion(보류 포함) |
| `pending[].id` | | `MessageId` | 그 completion의 ID(`completions[].id`). 유도값 `landing`을 잇는 키다 |
| `pending[].key` | | `TaskKey` \| null | |
| `pending[].sha` | | 글 \| null | SHA 앞 7자 |
| `pending[].landing` | 유도 | `merge_left` \| `push_left` \| null | `derived.landing[<그 completion의 ID>]`. 넘겨받지 않았으면 이전 `status.json`의 `pending[]`에서 `id`가 같은 건의 값(SHA가 null인 건도 이어진다), 없으면 null |
| `pending[].hold_reason` | | 글 \| null | |
| `decisions[]` | state | | 열린 결정만: `id`, `key`, `question`, `options`, `asked_at`, `expires_at` |
| `ready` | 유도 | `TaskKey` 목록 | `derived.ready`. 넘겨받지 않았으면 이전 값, 없으면 `[]` |
| `sync_pending` | state | 객체(`RoleId` → SHA 앞 7자) | |
| `metrics` | 유도 | 객체 | `at`(계산한 시각), `since`(첫 사람 턴의 시각), `human_turns`, `done_tasks`, `turns_per_task`(수 \| null), `unattended_dispatches`, `unattended_tasks`, `subagent_calls`, `workflow_runs`, `sub_dispatches`. 넘겨받지 않았으면 이전 값, 계산된 적이 없으면 모든 수가 0이고 `turns_per_task`가 null이다(`at == 0`이 "아직 계산하지 않았다"다). 계산은 `fullops_core::metrics`(묶음 C) |
| `issue` | state | null | v1에서는 항상 null(이슈 자동 모드 자리) |

### 4.5 `waiters/<run>.json`

`{"pid": 정수, "sid": SessionId | null, "heartbeat": 시각}`. 타입은 `Waiter`다. `fullops wait`가 Orca가 깨울 수 없는 세션에서만 쓰고 30초마다 `heartbeat`를 갱신하며 끝날 때 지운다. X-05가 `heartbeat`가 90초 이내인지 본다.

## 5. 작업 트리별 Git 디렉터리

### 5.1 `sessions/<sid>.json`

타입은 `Session`이다. hook과 `route`가 쓴다. `create_session`(없을 때만 만든다), `update_session`(잠금 `sessions/<sid>.json.lock` 안에서 다시 읽어 고친다. 파일이 없으면 `StoreError::Missing`), `read_session`으로 다룬다. `v`가 `SESSION_V`(1)보다 크면 `Unknown("session_newer")`다. 모르는 필드는 보존한다(`Session`, `dispatch`, `blocks[]`의 `extra`).

| 필드 | 타입 | 뜻 | 쓰는 주체 |
|---|---|---|---|
| `v` | 정수 | `1` | S-02 |
| `sid` | `SessionId` | | S-02 |
| `host` | `claude` \| `codex` \| `grok` \| `agy` | hook 인자로 고른 어댑터의 ID | S-02 |
| `started_at` | 시각 | | S-02 |
| `last_seen` | 시각 | 14일 지나면 lead의 SessionStart가 지운다 | P-00 |
| `terminal` | 글 \| null | 세션 파일을 만들 때의 세션 핸들(`ORCA_AGENT_SESSION_ID`, 없으면 `ORCA_TERMINAL_HANDLE`) | S-02 |
| `mode_at_start` | `[mode, primary_role \| null, primary_branch \| null]` | 현재 값과 다르면 정책 오류다 | S-02 |
| `anchor` | `Sha` \| null | 첫 SessionStart의 HEAD. 처음 만들 때 한 번만 쓰고 resume·compact에서 유지한다. 커밋이 없는 레포에서는 null | S-02 |
| `turn` | 정수 | 사용자 입력마다 1 증가 | P-00 |
| `prompt_fp` | 글 \| null | 프롬프트 이벤트가 없는 호스트(agy)만 쓴다. 마지막으로 본 사용자 입력의 sha256 앞 16자 | `Effect::SetPromptFp` |
| `dispatch` | `{ "id": DispatchId, "task": 글 \| null, "bound_by": "preamble" \| "terminal" }` \| null | 묶인 dispatch | P-01, 터미널 결합 |
| `routed` | `TaskKey` 목록 | 이 세션이 분류한 키. 합치기 연산(`Session::add_routed`) | `route` |
| `blocks` | 객체(규칙 ID → `{ "fp": 글, "turn": 정수 }`) | 한 번 막기 기록. 새 dispatch가 묶이면 비운다(`Session::bind`) | Stop의 BlockOnce |
| `noticed` | 글 목록 | 세션당 한 번 낸 안내·경고의 키. 합치기 연산(`Session::notice`) | hook |

### 5.2 `sessions/<sid>.seen/<이벤트>`

빈 파일이다. 그 세션에서 이벤트 종류(`SessionStart` 등 `EventKind`의 이름)가 처음 실행됐다는 표식이다. hook이 쓰고 `fullops why`가 읽는다.

### 5.3 `current.json`

`{"sid": SessionId, "terminal": 글 | null, "at": 시각}`. 타입은 `Current`다. PreToolUse(명령에 `fullops`가 있을 때, T-12)가 쓰고 CLI가 10분 이내의 값으로 자기 세션을 찾는다(`fullops_store::read_current`·`write_current`).

### 5.4 `trace/<sid>.jsonl`

타입은 `TraceLine`이다. hook이 통과가 아닌 판정, 오류, 시간 초과에 한 줄을 쓴다(`FULLOPS_TRACE=1`이면 모든 호출). 1MB를 넘으면 앞 절반을 버린다. 명령 문자열·프롬프트·파일 내용·환경 변수 값은 쓰지 않는다.

| 필드 | 타입 | 뜻 |
|---|---|---|
| `t` | 시각 | |
| `v` | 정수 | `1` |
| `bin` | 글 | 실행본 버전 |
| `host` | `claude` \| `codex` \| `grok` \| `agy` | |
| `ev` | `SessionStart` \| `UserPromptSubmit` \| `PreToolUse` \| `Stop` \| `StopFailure` | |
| `actor` | 글 \| null | `Actor::label()`: `lead`, `role:<id>`, `reviewer`, `other`. dispatch가 묶였으면 뒤에 `+dispatch`, 하위 worker면 `+sub`가 붙는다 |
| `intents` | 글 목록 | `Intent` 변형 이름(`OrcaWorkerStart` 등) |
| `rules[]` | | `id`(규칙 ID), `verdict`(`context`·`warn`·`deny`·`block`·`effect`·`unknown`), `fp`(지문 \| null) |
| `git`, `orca` | 정수 | 띄운 프로세스 수 |
| `ms` | 정수 | 걸린 시간 |
| `err` | 글 \| null | 오류 종류(`lock_busy`, `timeout`, `panic`, `io` 등) |

### 5.5 `search/<scope>.json`

탐색 색인이다. 타입은 `fullops_core::explore::SearchIndex`다. 워크트리마다 따로 두고 커밋된 `HEAD`의 메타데이터만 담는다. 묶음 J의 `fullops_store::update_index`가 `search/lock`을 `File::try_lock`으로 한 번만 시도해 쓰고, 지우면 다음 실행이 다시 만든다.

| 필드 | 타입 | 뜻 |
|---|---|---|
| `v` | 정수 | `1` |
| `parser` | 글 | `map-v2`(글 규칙을 바꾸면 올린다. 값이 다르면 색인을 다시 만든다) |
| `config_sha256` | 글 | `{scope, exclude(HEAD의 lint.json), max_blob: 65536}`의 sha256. `parser`나 이 값이 다르면 전부 다시 만든다 |
| `files_sha256` | 글 | `files`를 키 정렬한 JSON의 sha256. 읽을 때 다르면 색인을 버린다 |
| `files[]` | `IndexEntry` | `path`, `blob`(blob OID), `summary`(240자), `headings`(160자 24개), `definitions`(240자 24개), `searchable`(불), `read_error`(글 \| null. UTF-8이 아니면 `not utf-8`, blob을 읽지 못하면 `unreadable`. 값이 있는 항목은 다음 실행에서 다시 읽는다) |

### 5.6 `lint/result.json`

타입은 `LintResult`다. `lint`(묶음 D)가 쓰고 묶음 B·C·E가 읽는다.

| 필드 | 타입 | 뜻 |
|---|---|---|
| `v` | 정수 | `1` |
| `base` | `Sha` | `--from`을 푼 값 |
| `merge_base` | `Sha` | `merge-base(from, HEAD)`. 설정을 읽는 기준 시점 |
| `head` | `Sha` | |
| `config_sha256` | 글 | 기준 시점 `config/lint.json` blob 바이트의 sha256. 기준 시점에 파일이 없으면 내장 템플릿 글의 sha256이다. `fullops.toml`은 넣지 않는다 |
| `test_level` | `TestLevel` | 적용 레벨 |
| `started_at` | 시각 | |
| `seconds` | 수 | |
| `commands[]` | | `name`, `kind`, `run[]`, `cwd`, `status`(`passed` \| `failed` \| `timeout` \| `unrunnable` \| `skipped`), `exit_code`(정수 \| null), `seconds`, `reason`, `output_tail`(끝 40줄), `log`(로그 경로. 출력이 없으면 빈 글). skipped 항목은 `exit_code: null`, `seconds: 0`이다. 컴파일하지 못한 정규식 규칙은 `{name: <규칙 코드>, kind: "rule", status: "unrunnable", reason}` 항목으로 들어간다 |
| `findings[]` | | `code`, `severity`(`ERROR` \| `WARNING`), `path`, `line`(정수 \| null), `message`, `suggestion`. SIZE-002의 `path`는 빈 글이다 |
| `errors`, `warnings`, `unrunnable` | 정수 | `failed`·`timeout`은 ERROR로 센다 |
| `passed` | 불 | `errors == 0 && unrunnable == 0` |

### 5.7 `lint/pass.json`

타입은 `LintPass`다. 통과했을 때만 있다(`fullops_store::read_lint_pass`). 필드는 `v`(1), `base`(`Sha`, merge-base), `head`(`Sha`), `tree`(`Sha`), `config_sha256`(글), `test_level`(`TestLevel`), `at`(시각)이다.

## 6. 사용자 홈 `~/.fullops/`

묶음 F가 쓴다. 홈은 `fullops_store::home()`(Windows는 `USERPROFILE`, 그 밖은 `HOME`)이다.

| 경로 | 내용 |
|---|---|
| `bin/fullops[.exe]` | 고정 위치 실행본 |
| `bin/fullops.old-<시각>[.exe]` | 교체 전 실행본. `install --prune`만 지운다 |
| `bin/fullops.new-<pid>` | 설치 중의 임시 파일 |
| `install.json` | `version`, `sha256`, `source`(복사해 온 통합 패키지 폴더의 절대 경로), `at`, `previous`, `pinned`(불. `install --rollback`이 참으로 쓰고 실행본 단계가 기록을 새로 쓰면서 없앤다. 참이면 S-04가 올리지 않는다) |
| `deps.toml` | 실행본을 복사할 때 통합 패키지에서 함께 복사한 의존 목록(아래) |
| `deps-state.json` | 의존 설치 기록(아래) |
| `package/<버전>/grok/`, `package/<버전>/agy/` | `install`의 `plugin` 단계가 통합 패키지의 `hosts/` 재료로 조립한 패키지 |
| `package/<버전>/<호스트>.old-<시각>/` | 다시 조립하기 전에 있던 패키지 폴더 |
| `package/<버전>/source/`, `package/<버전>/source.old-<시각>/` | 실행본 단계가 보관한 통합 패키지(`install.json`의 `source`)와 바꿔 넣기 전의 것 |
| `skill-backups/<호스트>/<스킬 이름>/<폴더 sha256 앞 12자>/` | 스킬을 다시 설치하기 전에 있던 폴더의 사본. `install --prune`은 지우지 않는다 |
| `skill-backups/…/<앞 12자>/.fullops-origin.json` | `{"origins":[원래 경로…],"at":<처음 백업한 시각>}`. `install --restore-skills`가 되돌릴 자리 |
| `py-retired/<시각>/<번호>-<이름>`, `py-retired/<시각>/moved.json` | `install --retire-python --apply`가 옮겨 둔 옛 판 파일과 `[{"from","to"}]` 기록 |
| `cache/jev/<요청 sha256>.json` | Jev 응답 캐시(분류와 탐색이 함께 쓴다) |

`~/.fullops/` 밖에 fullops가 직접 쓰는 파일은 하나다: `<GROK_HOME 또는 ~/.grok>/hooks/fullops.json`(grok의 hook 등록 파일. 내용은 `fullops_core::deps::grok_hook_json`. `install`이 쓰고 `install --remove-hooks`가 지운다).

`deps.toml`(타입 `Deps`. 설계 10.7절). `fullops_store::parse_deps(text)`가 해석과 필드 규칙 검증을 하고 `load_deps(plugin_root, builtin)`이 찾는다.

| 표 | 필드 | 규칙 |
|---|---|---|
| 최상위 | `schema` | `1`. 다른 값이면 `DepsError::Schema(값)`이고 내장 사본으로 넘어가지 않는다 |
| 최상위 | `skill_cli` | 문자열 배열, 1개 이상. 생략하면 `["npx", "--yes", "skills@latest"]` |
| `[self]` | `plugin`, `marketplace`, `source`, `hosts` | 넷 다 필수. `hosts`의 값은 `HostId`(Rust 필드 이름은 `own`) |
| `[marketplaces]` | 이름 = 출처 | `[[plugins]].id`의 `@` 뒤 이름은 여기나 `[self].marketplace`에 있어야 한다 |
| `[[plugins]]` | `id`, `hosts`, `notice`(선택) | `hosts`에 `agy`는 쓸 수 없다 |
| `[[skills]]` | `source`, `names`, `hosts`(선택. 생략하면 네 호스트), `license`(선택) | `names`는 이름 목록 또는 `["*"]` |
| `[[mcp]]` | `name`, `url` | `url`은 `https://`로 시작해야 한다 |
| `[[optional]]` | `kind`(`tool` \| `skill`), `name`, `source`, `note` | 설치하지 않는다 |

모르는 표와 필드는 무시한다. 오류는 `DepsError { Parse(글), Schema(u32), Invalid(글) }`다: 읽기 오류, TOML 오류, 필수 필드 누락, 모르는 호스트 ID는 `Parse`, 위 표의 규칙 위반은 `Invalid`다.

`load_deps(plugin_root: Option<&Path>, builtin: &str) -> Result<(Deps, PathBuf), DepsError>`: (1) 플러그인 루트의 `deps.toml` (2) `~/.fullops/deps.toml` (3) `builtin`(바이너리에 내장한 빌드 시점 사본의 글. store는 바이너리의 자산을 모르므로 부르는 쪽이 넘긴다. 그 글은 묶음 F가 더하는 `fullops::assets::DEPS_TOML`이고 `plugin/deps.toml`을 `include_str!`로 읽은 것이다) 순으로 찾는다. 내장 사본을 썼으면 돌려주는 경로는 비어 있다. 찾은 파일이 규칙에 맞지 않으면 다음 자리로 넘어가지 않고 오류를 돌려준다.

`deps-state.json`(타입 `DepsState`. `read_deps_state()`·`write_deps_state(state)`): `v`(1), `manifest_sha256`(설치한 시점의 `deps.toml`의 sha256), `hosts`(객체. `HostId` → `{ at, self_version, complete(불), failed(실패한 명령의 `Step.what` 목록) }`). 파일이 없으면 빈 기록이다. `complete`는 실패가 없어도 건너뛴 단계가 있으면(npx가 없어 스킬을 건너뜀, 패키지 폴더가 없어 grok·agy 플러그인을 건너뜀) `false`다. `--only`로 일부 단계만 한 실행은 이전 값을 잇고, `deps.toml`의 sha256이 바뀌면 이번에 설치하지 않은 호스트의 값도 내린다.

레포의 엔트리 문서(`AGENTS.md`·`CLAUDE.md`·`GEMINI.md`)에 넣는 관리 블록의 표식은 `<!-- fullops:start -->`와 `<!-- fullops:end -->`다(`fullops::assets::BLOCK_START`·`BLOCK_END`).

## 7. hook 입력과 출력

명령은 `fullops hook <claude|codex|grok|agy> [--event <이름>] [--plugin-root <경로>] [--in-place]`다(설계 7.1절). 종료 코드는 항상 0이고 stderr에 쓰지 않는다. `fullops`를 인자 없이 실행하면 stdout은 비고 사용법을 stderr에 낸 뒤 종료 코드 0이다.

어댑터의 trait과 고르는 함수는 `fullops/src/hook/mod.rs`에 있다.

```rust
pub trait Host {                       // 구현: host_claude, host_codex, host_grok, host_agy
    fn id(&self) -> HostId;
    fn decode(&self, raw: &[u8], event_arg: Option<&str>) -> Result<HookInput, DecodeError>;
    fn encode(&self, event: EventKind, v: &Verdict) -> String;
    fn pass(&self, event_arg: Option<&str>) -> &'static str;
    fn caps(&self) -> Caps;            // caps_of(self.id())
}
pub fn host_for(arg: &str) -> Option<Box<dyn Host>>;   // 인자만 본다
```

통과의 글(`Host::pass`. 뼈대의 완성본)은 호스트 인자와 `--event`만으로 정해진다. 해석 실패, 활성 표식 없음, panic에서 모두 이 글을 낸다.

| 경우 | stdout |
|---|---|
| 호스트 인자가 없거나 넷이 아니다 | `{}` |
| `claude`·`codex`인데 환경 변수 `GROK_HOOK_EVENT`가 있다(grok이 다른 호스트의 등록 파일을 읽어 실행했다) | `{}` |
| `claude`·`codex`·`grok`의 통과 | `{}` |
| `agy --event PreToolUse`의 통과 | `{"decision":"ask"}` |
| `agy --event Stop`의 통과 | `{"decision":""}` |
| `agy`의 그 밖(`--event SessionStart`, `--event`가 없거나 값이 빠졌다) | `{}` |

`HookInput`(어댑터의 `decode`가 채운다. 입력 대응 표는 설계 7.2절):

| 필드 | 타입 | `claude`·`codex` | `grok` | `agy` |
|---|---|---|---|---|
| `event` | `EventKind` | `hook_event_name` | `hook_event_name` | `--event` 인자. Stop의 `terminationReason`이 `ERROR`면 `StopFailure`로 옮긴다 |
| `sid` | `SessionId` | `session_id` | `session_id` | `conversationId` |
| `cwd` | 경로 | `cwd` | `cwd` | `workspacePaths[0]`. 없으면 해석 실패다 |
| `source` | 글 \| 없음 | `source`(`startup`·`clear`·`resume`·`compact`·`fork`) | `new` → `startup`, `load` → `resume` | 항상 `startup` |
| `prompt` | 글 \| 없음 | `prompt` | `prompt` | 모든 이벤트의 `lastUserInput` |
| `tool` | `ToolCall` \| 없음 | `tool_name`, `tool_input` | `tool_name`, `tool_input`, `toolInputTruncated` | `toolCall.name`, `toolCall.args` |
| `stop_reason` | 글 \| 없음 | 없다 | `reason` | `terminationReason`(`NO_TOOL_CALL`은 `end_turn`으로 옮긴다) |
| `error` | 글 \| 없음 | `error`(오류 종류 이름. Codex는 StopFailure가 없다) | `error` | `"error"` |
| `error_details` | 글 \| 없음 | `error_details`(없으면 `last_assistant_message`)의 앞 300자 | `error_details` | 입력의 `error` 글 |
| `agent_id` | 글 \| 없음 | `agent_id` | `subagentType` | 없다 |

`ToolCall`(도구 대응 표는 설계 7.2절. 7.4·7.5절은 이것만 본다):

| 필드 | 타입 | 채우는 값 |
|---|---|---|
| `name` | 글 | 호스트가 준 도구 이름(추적용) |
| `kind` | `ToolKind` | `Shell(Bash\|PowerShell)`, `Write`, `Edit`, `Patch`, `SubAgent`, `Workflow`, `Ask`(질문 카드), `Other` |
| `command` | 글 \| 없음 | `Shell`의 명령 글, `Patch`의 패치 본문 |
| `file_path` | 글 \| 없음 | `Write`·`Edit`의 대상 경로 |
| `content` | 글 \| 없음 | `Write`의 전체 내용 |
| `edits` | `EditOp { old, new, replace_all }` 목록 | `Edit`. 여러 곳 편집은 여러 원소 |
| `text` | 글 \| 없음 | `SubAgent`의 지시 글(인자가 여럿이면 줄바꿈으로 이은 것) |
| `truncated` | 불 | 호스트가 입력을 잘랐다(grok의 `toolInputTruncated`). 참이면 셸 분류는 `Unparsed`이고 편집 대상의 `after`는 `Unknown`이다 |
| `cards` | `Card { question, options }` 목록 | `Ask`의 질문들. `options`는 선택지 label(적힌 순서). 모양을 읽지 못하면 비어 있다 |

판정 출력(`Host::encode`. 묶음 B가 채운다. 표본은 `scenarios/fixtures/host/output/`, `host/grok/output/`, `host/agy/output/`). 문구는 `fullops_core::messages`의 `refusal`·`block`·`warn`으로 만든다.

| `Verdict` | `claude`·`codex` | `grok` | `agy` |
|---|---|---|---|
| `Pass` | `{}` | `{}` | PreToolUse `{"decision":"ask"}`, Stop(StopFailure로 옮긴 Stop 포함) `{"decision":""}`, SessionStart `{}` |
| `Context(t)` | `{"hookSpecificOutput":{"hookEventName":"<이벤트>","additionalContext":t}}` | 규칙이 만들지 않는다. 만들어졌다면 `{}` | SessionStart: `{"injectSteps":[{"systemMessage":{"systemMessage":t}}]}` |
| `Warn(t)` | `{"systemMessage":"FullOps: " + t}` | SessionStart에서만 같은 형태. 그 밖은 `{}` | SessionStart에서만 `{"injectSteps":[{"errorMessage":"FullOps: " + t}]}`. 그 밖은 통과의 글 |
| `Deny { rule, reason }` | `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"FullOps[<규칙>]: <사유> 다음: <탈출구>"}}` | 같은 형태 | `{"decision":"deny","reason":"<같은 글>"}` |
| `Block { rule, reason, also }` | `{"decision":"block","reason":"FullOps[<규칙>]: <사유> 다음: <탈출구> 함께 남은 것: <also>"}` | 같은 형태 | `{"decision":"continue","reason":"<같은 글>"}` |

묶음 B가 `decode`·`encode`와 규칙의 `check`를 채웠다. 해석 실패, 활성 표식 없음, panic, 기한 초과에서는 그 호스트의 통과 글을 낸다.

묶음 B가 더한 공개 항목(시그니처가 동결된 `Rule.check`와 `Facts`는 그대로다):

| 항목 | 자리 | 내용 |
|---|---|---|
| `Scene { root, orca_names, self_install, missing_tools }` | `fullops_core::rules` | 규칙에 넘기는 호출 문맥: 레포 루트, `ORCA_CLI_COMMAND`의 이름, S-04의 조건, PATH에 없는 도구. `rules/call.rs`의 thread-local에 둔다 |
| `decide_in(scene, input, facts) -> Outcome` | `fullops_core::rules` | 바이너리가 부르는 진입점. 동결된 `decide(input, facts)`는 루트를 `path_exists`로 찾아 이것을 부른다 |
| `NO_SESSION`(`"no_session"`) | `fullops_core::rules` | `Facts::session()`이 `Unknown(NO_SESSION)`이면 세션 파일이 없다는 뜻이다(읽지 못한 것과 가른다) |
| `RECORD_PATHS`, `is_record_path`, `is_change_target`, `landing_of` | `fullops_core::derive` | 기록 경로 다섯(설계 6.6절)과 `changed()`·`landing()`의 순수한 부분 |
| `repo_relative(path, cwd, root, real)` | `fullops_core::patch` | 편집 경로를 레포 루트 기준으로 맞춘다. `real: &mut dyn FnMut(&Path) -> PathBuf`는 실제 경로로 푸는 함수(`Facts::real_path`)이고 `root`는 이미 푼 루트다(검증 뒤 수정 R3-2) |
| `Facts::real_path(path) -> PathBuf`(기본 구현은 받은 그대로), `Facts::qa_commit_candidates(&CommitScope)`, `CommitScope { tracked, added }`·`takes(index, path)` | `fullops_core::facts` | 검증 뒤 수정에서 바뀐 `Facts`의 둘: 경로 맞추기의 실제 경로(설계 7.4절), T-09가 git이 실제로 담을 항목만 보는 범위(`tracked`는 `commit -a`·`add -u`, `added`는 레포 상대 경로이고 빈 글은 레포 전체. 설계 7.5절). 전에는 `qa_commit_candidates(include_unstaged: bool)`였다 |
| `Intent::GitAdd { paths, update }` | `fullops_core::model` | `paths`는 적힌 경로(작업 위치 기준) 그대로이고 레포 전체는 `:/` 하나다(`-A`·`--all`·`--pathspec-from-file`·`git -C`·풀리지 않은 확장·경로 없는 add). `update`는 `-u`. `stage`도 같다. `--inject`·`--dry-run` 없는 `orchestration dispatch`는 `OrcaWorkerStart`다(설계 7.4절) |
| `command_name(token)` | `fullops_core::shell` | 셸 낱말에서 명령 이름을 뽑는다 |
| `read_trace(repo, sid) -> Vec<TraceLine>` | `fullops_store::trace` | `fullops why`가 읽는다 |

규칙 표(`fullops_core::rules::RULES`. 37개. 표 순서가 평가 순서다)의 타입:

| 타입 | 필드 |
|---|---|
| `Rule` | `id`, `event`, `applies: fn(&Actor) -> bool`, `strength`(`Context` \| `Effect` \| `Deny` \| `BlockOnce` \| `BlockAlways`), `on_unknown`(`Pass`), `check: fn(&HookInput, &mut dyn Facts) -> Finding`, `message: MsgId`(값은 규칙 ID), `escapes: &[(경우, 탈출구 문장)]`, `origin`(설계 11.1절의 1.24.0 기능 이름 또는 `없음`) |
| `Finding` | `None` \| `Unknown` \| `Hit { fp, args, case, always }`. `case`는 걸린 경우(T-05의 `a`~`d`, T-08의 `e`~`g`, X-05의 `a`·`b`. 없으면 빈 글. X-01과 X-03은 경우를 나누지 않고 탈출구 문장 하나가 두 쪽을 함께 적는다), `always`가 참이면 `BlockOnce` 규칙을 이번만 `BlockAlways`로 다룬다(X-01의 dev 직접 작업) |
| `Effect` | `CreateSession`, `TouchSession`, `BumpTurn`, `BindDispatch { id, task, by }`, `RecordBlock { rule, fp }`, `RecordNotice(글)`, `AppendRouted(TaskKey)`, `RecordTurn(TurnKind)`, `StampCurrent`, `MarkLanded(Vec<MessageId>)`, `MarkSettled(DispatchId)`, `CloseRuns(Vec<RunId>)`, `CloseLostSnapshots`, `GcSessions`, `SelfInstall`, `PeekCompletions { run }`, `SendLimitEscalation { dispatch, task, error }`, `SetPromptFp(글)`, `RecordCards(Vec<(Card, Option<TaskKey>)>)` |

`Rule::escape(case)`가 경우에 맞는 탈출구를 고른다(그 경우가 없으면 첫 탈출구). 경우가 하나뿐인 규칙의 경우 이름은 빈 글이고 안내·효과 규칙의 `escapes`는 비어 있다. `fullops rules --json`은 `{"ok":true,"rules":[{"id","event","strength","escapes":[{"case","escape"}],"origin"}]}`(`RULES`의 순서 그대로)를, `fullops rules [--markdown]`은 Markdown 표를 낸다. ID·이벤트·강도가 설계 7.5절의 표와 같은지는 `crates/fullops/tests/skeleton.rs`가 대조한다.

| 이벤트 | 규칙 ID |
|---|---|
| SessionStart | S-00, S-01, S-02, S-03, S-04, S-05, S-06 |
| UserPromptSubmit | P-00, P-01, P-02, P-03 |
| PreToolUse | T-00 ~ T-16 |
| Stop | X-00 ~ X-07 |
| StopFailure | F-01 |

CLI의 거절 ID(`Refusal.rule`)는 hook 규칙 ID와 겹치지 않는다: `route`는 RT-01~RT-05, `task`는 H-01~H-04, `dispatch`는 D-01~D-13, `done`은 R-01~R-11, `merge`와 통합 명령은 M-01~M-13, 탐색은 J-01·J-02·J-04(J-03과 J-10~J-17, `dispatch`의 D-12, `done`의 R-10, `review`의 RV-10은 2.1.0에서 패킷과 함께 뗐고 번호를 다시 쓰지 않는다), `review`는 RV-00~RV-09(RV-01~RV-09는 `review check`의 실패 항목 번호, RV-00은 그 밖의 거절), `resume`·`completion set-sha`는 HD-01·HD-02, `decision`은 DC-00~DC-04이다(DC-04는 결정을 연 뒤의 사람 턴이 없다). `sync`가 반영하지 못한 역할은 M-11, `hold`의 대상 없음은 M-02로 낸다. 번호 없이 단계 이름을 쓰는 거절도 있다: `publish`·`sync`(`dispatch` 3·4단계), `wait`(열린 Run이 여럿), `LINT`(`lint`의 더러운 작업 트리), `install`(마켓플레이스 출처 충돌), `migrate`(전제 위반), 검증 뒤 수정에서 더한 `reopen`(병합 전 `task reopen`이 기록을 덮어쓰게 되는 경우)과 `init`(Python판 레포의 `init`). `wait`는 남의 Run을 `--run`으로 준 경우에도 쓴다. CLI의 거절 ID는 `fullops rules`의 표에 나오지 않는다(그 표는 hook 규칙 34개다). 정본은 설계 8절의 명령별 절이다.

검증 뒤 수정에서 바뀐 CLI 꼴(설계 8절): `fullops hold --completion <메시지 ID> --reason … --owner … --resume …`는 KEY 없이 받는다(키 없는 보고. `merge`·`resume`은 KEY가 필요하다). `fullops route`에 `--env-file <경로>`가 있다. `fullops merge --json`의 성공 출력에 `ungated`(불. 병합 기록 커밋 없이 이미 통합 기준 ref에 들어가 리뷰·lint를 판정하지 못한 SHA)가 있다. `task new`·`task reopen`의 다음 명령 안내는 `fullops dispatch <KEY> --role <R>`(reopen이면 `--purpose fix`)다.

## 8. CLI 공통 규약과 묶음 사이 인터페이스

구현은 `fullops/src/cli.rs`다(설계 8.1절).

| 항목 | 규약 |
|---|---|
| 형태 | `fullops <명령> [인자] [--repo <경로>] [--json]`. 두 옵션은 명령 이름 뒤 어디에나 올 수 있고 `cli::run`이 먼저 떼어 낸다. 다른 옵션의 값이 `--json`·`--repo`와 같은 글이면 `--옵션=값` 꼴로 준다 |
| 종료 코드 | 0 성공(`EXIT_OK`), 1 내부 오류·사용법 오류(`EXIT_ERROR`), 2 규칙에 의한 거절·정책 오류·활성 표식 없음(`EXIT_REFUSED`), 3 사람 손이 필요함(`EXIT_HUMAN`), 4 바깥 도구 실패(`EXIT_EXTERNAL`). 인자 없는 실행은 0, 모르는 하위 명령은 1이다 |
| 출력 자리 | 명령의 결과는 stdout(`Ctx::print`), 거절·오류·알림 문구는 stderr(`Ctx::note`와 오류 보고). `--json`이면 stdout에는 JSON 객체 하나만 쓴다. 거절·오류의 사람용 문구는 `--json`이어도 stderr에 함께 낸다(JSON을 읽는 쪽은 stdout만 본다) |
| 성공(`--json`) | `{"ok":true, …명령별 필드}`. `Ctx::print(human, fields)`가 `ok`를 붙인다 |
| 거절 | 사람용은 건마다 `FullOps[<규칙>]: <사유> 다음: <탈출구>` 한 줄. `--json`은 `{"ok":false,"refused":[{"rule","reason","escape"}]}`. 종료 코드는 걸린 `Refusal.exit`의 최댓값이다(`CliError::Refused`) |
| 사람 손이 필요한 종료 | `--json`은 `{"ok":false,"human":{"decision":"<결정 ID>"\|null,"message":"<글>"}}`, 종료 코드 3(`CliError::Human { decision, message }`) |
| 그 밖의 오류 | `--json`은 `{"ok":false,"error":{"kind":"usage"\|"internal"\|"external"\|"policy"\|"inactive","message":"<글>"}}` |
| 활성 표식 없음 | "활성 레포가 아닙니다", 종료 코드 2. `init`·`install`·`doctor`·`migrate`·`rules`는 제외 |
| 정책 오류 | 종료 코드 2. `doctor`·`mode`·`init`·`migrate`·`upgrade`·`status`·`why`·`install`·`rules`는 받는다(T-00의 통과 목록과 같다) |
| 미구현 명령 | 종료 코드 1과 "아직 구현되지 않았습니다"(`CliError::unimplemented`). 명령 표의 명령은 모두 구현됐다 |
| 명령별 `--json`의 예외 | `lint`는 미통과여도 `{"ok": <passed>, "result": <경로>, …result.json의 필드}`를 낸다. `doc check`는 `{ok, checked, unwritten, errors[], warnings[]}`다 |
| 오프라인 | `FULLOPS_OFFLINE=1`이면 Jev 호출, `merge`·`settle`의 fetch, `doctor`·`install --check`의 호스트 CLI 조회를 건너뛴다. `merge`의 M-09 push는 오프라인에서도 한다 |
| 호출자 판정 | `Ctx::caller()`. `current.json`(10분 이내)의 `sid`로 세션을 찾고 `facts::resolve_actor`(설계 7.3절)로 정한다. 세션이 없으면 브랜치만으로 좌석을 정하고 dispatch는 세션 핸들로만 묶는다 |
| 세션 핸들 | `cli::session_handle()`: `ORCA_AGENT_SESSION_ID`, 없으면 `ORCA_TERMINAL_HANDLE`. hook(`Facts::session_handle`)과 CLI가 함께 쓴다 |
| 플러그인 루트 | `cli::plugin_root()`: (1) 자기 실행 파일이 `<X>/bin/` 아래에 있고 `<X>/VERSION`이 있으면 X(`plugin_root_of_exe()`. hook의 추론도 이 함수다) (2) `install.json`의 `source` 폴더에 `VERSION`이 있으면 그 폴더 (3) 없음. CLI는 `--plugin-root` 인자를 받지 않는다 |
| 상태 쓰기 | `Ctx::update_state(derived, f)`. 잠금 대기 10초. `state_broken`이면 `state.json.broken-<시각>`으로 옮기고 빈 상태로 시작한 뒤 종료 코드 1로 알린다 |
| 명령 추가 | `cmd/mod.rs`의 `COMMANDS` 표에 모든 명령이 있다(`route`, `find`, `context`, `task`, `plan`, `dispatch`, `wait`, `done`, `merge`, `hold`·`resume`·`settle`·`completion`, `sync`, `review`, `decision`, `lint`, `doc`, `env`, `gc`, `status`, `why`, `mode`, `upgrade`, `rules`, `init`, `migrate`, `install`, `doctor`). 각 묶음은 자기 `cmd/<이름>.rs`의 `run(ctx, args)`만 채운다. 한 파일이 여러 명령을 맡으면 `ctx.command`로 가른다(`hold.rs`: `hold`·`resume`·`settle`·`completion`) |

호출자 판정 `resolve_actor(&ActorInput) -> ResolvedActor`(`fullops_core::facts`. hook과 CLI가 함께 쓴다): 세션 파일의 dispatch 또는 세션 핸들이 같은 미정산 dispatch(예약 기록 제외, 정확히 하나, 같은 워크트리)로 묶고, 좌석은 묶였든 아니든 설계 7.3절 3~6번으로 정한다(리뷰 사본에서는 dispatch가 묶여도 `Reviewer`). `Actor { seat, dispatch, direct_dev, sub }`의 `sub`는 묶인 dispatch가 `state.json`에서 `sub_of`를 가질 때 참이다(`state.json`을 읽지 못하거나 그 dispatch가 없으면 거짓).

`fullops wait`의 반환(JSON, 설계 8.6절): `run`, `idle_timeout`, `check_minutes`, `actionable[]`(`id`, `delivery`, `type`, `ack_by`(`merge` \| `ack`), `from`, `subject`, `key`, `role`, `sha`, `sha_source`, `outcome`, `body`, `notes[]`), `absorbed`(`heartbeats`, `status`), `lost[]`(`dispatch`, `key`, `role`), `orca_wakes`(불), `next`. 본문(`body`)과 제목(`subject`)은 이번 호출에서 받은 것만 있고 저장하지 않는다.

묶음 사이에 주고받는 값(`fullops_core::model`):

| 타입 | 필드 | 쓰는 곳 |
|---|---|---|
| `DispatchRequest` | `key`, `role`, `purpose`, `run`, `terminal`, `tuple`, `sha`, `reason`, `brief`(`--brief`의 글), `sub`(하위 배정이다: 호출자가 dispatch가 묶인 세션이다. 부르는 쪽이 `Ctx::caller`의 결과로 채운다) | `check_dispatch`(C). `--task`는 없다 |
| `TaskNewRequest` | `role`, `key`, `goal`, `reopen` | `check_task_new`(C) |
| `OrcaMessage` | `id`, `kind`(Orca의 `type`. 없으면 `status`), `from`, `subject`, `body`, `task_id`, `dispatch_id`, `outcome`(넷은 `payload`가 객체일 때만) | `OrcaCli::check` → `record_message` |
| `MessageNote` | `absorbed`(`heartbeat`·`status`라 세기만 했다), `completion: Option<CompletionNote>`, `notes` | `record_message(state, run, msg, at, resolve, branch_tip, is_ancestor)`(C)의 결과. `at`은 받은 시각, `is_ancestor(older, newer)`는 조상 판정이다. 부르는 쪽은 `wait`(C)와 T-11의 효과(B)다 |
| `CompletionNote` | `key`, `role`, `sha`, `sha_source`, `outcome` | completion을 만들었을 때의 기록 |
| `Baseline` | `review`, `code`, `test_level`, `fell_back` | 3.1절 |
| `BriefAt` | `Session` \| `Spec { command, key, sub }`(`command`는 spec에 적는 실행 명령, `key`는 과제 키, `sub`는 하위 배정인가) | `parallel_brief` |
| `SkillText` | `path`, `body` | `Facts::user_skill`(S-06). 조건과 읽기는 `rules::start_skill(input, caps, read)`이고 hook 진입이 비활성 레포의 SessionStart에서도 부른다(설계 7.1절) |
| `TreeEntry` | `mode`, `kind`(`blob` \| `commit`), `oid`, `path` | `Git::ls_tree`, `update_index`. store도 받으므로 ext가 아니라 core에 둔다 |

병렬 방침의 안내 문구는 `fullops_core::messages::parallel_brief(parallel, host: Option<HostId>, at: BriefAt) -> String` 하나가 만든다(설계 8.15절의 고정 문구. 완성본). `<도구>`는 `caps_of(host).subagent_tool`이고 호스트를 모르면 "하위 에이전트 도구"다. `ultra`이고 호스트가 `claude`면 `messages::ULTRA_HINT`가 붙는다. `at`이 `Spec`이고 `sub`가 거짓이면 `on`·`ultra`의 문장 끝에 하위 배정 문장("다른 모델이 맡아야 하는 부분만 `<command> dispatch <key> --brief …`로 하위 worker에 맡기고, 보고를 받아 확인한 뒤 `<command> done`을 한다.")이 붙는다. `sub`가 참이면 붙지 않는다. `dispatch`(C)는 돌려받은 글을 떼거나 바꿔 적지 않는다.

spec에 적는 실행 명령은 `fullops_core::messages::spec_command(host: Option<HostId>, fixed_bin: &str) -> String`이 정한다(설계 8.5절. 완성본). 호스트가 `claude`면 `fullops`, 그 밖(모르는 agent 포함)이면 `fixed_bin`이고 공백이 있으면 큰따옴표로 감싼다. `fixed_bin`은 `fullops_store::fixed_bin()`을 `path_text`로 적은 글이다. spec에서 이 값으로 적는 자리는 끝 문장의 `<실행 명령> done`과 하위 배정 문장의 두 곳뿐이다.

문구는 `fullops_core::messages`에 둔다. `messages/mod.rs`(A)는 `MsgId`, `reason(id, args)`, `refusal`, `block`, `warn`, `parallel_brief`, `spec_command`, `ULTRA_HINT`, `INACTIVE_REPO`, `UNIMPLEMENTED`를 가진다. 규칙별 사유 문구는 묶음별 파일의 `reason(id: &str, args: &[String]) -> Option<String>`에 둔다: `hook.rs`(B), `flow.rs`(C), `merge.rs`(E), `lint.rs`(D), `explore.rs`(J), `install.rs`(F). `messages::reason`이 여섯 파일을 차례로 묻고 어느 파일에도 없으면 `args`를 공백으로 이어 낸다(구현 묶음이 문구를 채우기 전에도 출력이 비지 않는다).

뼈대가 뒀던 임시 구현(설계 10.2절 끝의 표). 여덟 묶음이 합쳐진 지금은 모두 구현으로 바뀌었고, 표의 "임시 구현" 열은 뼈대 시절의 값이다.

| 함수 | 자리 | 구현 | 임시 구현 |
|---|---|---|---|
| `header_summary`, `lint_excludes`, `parse_deliverable_index` | `fullops_core::lint_rules` | D | 빈 글, 빈 `GlobSet`, 빈 목록 |
| `install_plan`, `grok_hook_json` | `fullops_core::deps` | F | 빈 `Plan`, `{}` |
| `JevCurl`(`Judge`) | `fullops_ext::jev` | C | 항상 `Err(JudgeError::Offline)` |
| `metrics` | `fullops_core::flow` | C | 모든 값이 0 |
| `orca_effort` | `fullops_core::flow` | C | tuple의 `effort` |
| `record_message`, `check_dispatch`, `check_task_new` | `fullops_core::flow` | C | 빈 `MessageNote`, 빈 목록 |
| `write_explore_result`, `update_index` | `fullops_store::search` | J | `Unsupported` 오류, 빈 목록 |
| `explore`의 그 밖(`instruction_sha256`, `sendable_path`, `sendable_text`, `index_entry`, `batches`, `triage`) | `fullops_core::explore` | J(J의 파일끼리만 주고받는다) | 빈 값 |
| `host_available`, `host_version`, `host_marketplaces`, `host_plugins`, `run_step` | `fullops_ext::hosts` | F | 거짓, `Unknown`, 띄우지 않음 |
| `append_turn`, `append_trace` | `fullops_store` | B | `Unsupported` 오류, 아무것도 안 함 |
| `classify_shell`, `edit_targets`, `decide`, 규칙의 `check`, 어댑터의 `decode`·`encode` | `fullops_core`, `fullops/src/hook` | B | `Unparsed`, 빈 목록, 통과 |
| `review_need` | `fullops_core::review` | E | `required: true`(리뷰를 조용히 건너뛰지 않는다) |

뼈대의 완성본: `GitCli`의 메서드 전부, `OrcaCli`의 메서드 전부, `proc::run`(기본 구현), `caps_of`·`host_of_agent`, `parallel_brief`·`spec_command`, `baseline`(store와 core의 두 함수), `resolve_actor`, `State::sub_open`, `orca_wakes`·`ack_by`, `fixed_bin`, `load_deps`·`parse_deps`, `skill_roots`, `path_text`·`same_path`, `stage_of`·`ready`, `parse_plan`, `parse_front_matter`·`FrontMatter::render`, `assets`의 네 상수, 네 어댑터의 `pass`.

`fullops_ext`의 인터페이스(시그니처는 설계 10.2절. 타입 정의는 코드가 정본이다):

| 대상 | 내용 |
|---|---|
| `run(argv, cwd, timeout, env, stdin) -> ProcResult` | `ProcResult { status: Exited(i32) \| TimedOut \| NotFound, stdout, stderr }`. 제한 시간이 지나면 프로세스 트리를 끝낸다(묶음 D): Windows는 `taskkill /T /F /PID`(직접 띄운 프로세스가 먼저 끝난 뒤 남은 손자는 찾지 못한다. 그때도 5초만 더 기다린다), POSIX는 `process_group(0)`으로 띄운 그룹에 `kill -KILL -- -<pgid>`. 출력의 EOF를 먼저 기다리고, 직접 띄운 프로세스가 끝났는데 자손이 파이프를 쥔 채 남으면 200ms만 더 읽고 `Exited(코드)`와 그때까지의 출력을 돌려주며(그 자손은 끝내지 않는다), `TimedOut`의 stdout·stderr에는 끝내기 전까지의 출력이 남는다. `run`으로 띄우는 모든 프로세스(git·orca·curl 포함)가 자기 프로세스 그룹을 가진다 |
| `ExtError` | `NotFound`, `TimedOut`, `Failed { code, stderr }`, `Parse(글)` |
| `GitCli { repo_root, timeout }` | `GitCli::hook(root)`(호출당 2초), `GitCli::cli(root)`(30초. `fetch`·`push`·`merge`는 120초). 모든 호출에 `GIT_TERMINAL_PROMPT=0`, `GIT_NO_LAZY_FETCH=1`, `LC_ALL=C`, `-c core.quotepath=false`. 읽기는 실패하면 `Known::Unknown("git")`이다. 탐색용 읽기(`ls_tree`·`blob_prefixes`·`grep_files`)는 오프라인(`FULLOPS_OFFLINE=1` 또는 `FULLOPS_JEV_ENABLED=0`)이면 `GIT_ALLOW_PROTOCOL=`을 더한다. 검증 뒤 수정(R2-1·R1-2): 값이 `-`로 시작하면 부르지 않는다(읽기 `Unknown("git_arg")`, 쓰기 `Err`)고 `--`를 넣는다. `fetch`·`push`는 등록된 원격 이름과 온전한 ref(`refs/heads/x`, `refs/heads/x:refs/heads/x`)만 받고 `+` refspec을 거절한다. diff 계열 넷은 `-c core.attributesFile= --attr-source=<빈 트리> --no-ext-diff --no-textconv --no-relative`를 고정하고 `diff_u0`는 `--inter-hunk-context=0 --src-prefix=a/ --dst-prefix=b/`도 고정한다. `numstat`의 `FileStat`에 `binary`(git이 줄 수를 세지 못함)가 있다 |
| `Git`의 읽기 | `is_ancestor`, `merge_base`, `rev_parse`(`rev-parse --verify`. 커밋으로 풀려면 `<rev>^{commit}`), `diff_names`·`numstat`(`--no-renames`. 이름 변경은 삭제와 추가로 낸다), `status(pathspec)`(`--untracked-files=all`), `show_blobs(&[(rev, path)])`, `worktrees`, `ls_tree`, `blob_prefixes(oids, max)`, `grep_files(rev, literals)`, `name_status`(`NameStatus { status, path, old_path }`. 이름 변경을 찾는다), `diff_u0` |
| `Git`의 쓰기 | `commit_only(paths, message)`(바뀐 경로만 골라 인덱스에 넣고 `commit --only`. 새 파일과 삭제도 된다. 바뀐 것이 없으면 `None`), `merge(cwd, rev, FfOnly \| NoEdit) -> Done \| Conflict`(충돌이면 병합이 진행 중으로 남는다), `push`, `fetch`, `mv`(대상 폴더를 만든다), `merge_abort`, `reset_keep`, `update_ref(name, new, old)`, `worktree_add_detached`, `worktree_remove` |
| `OrcaCli { program, cwd, timeout }` | `OrcaCli::new(cwd, timeout)`이 실행 파일을 찾는다(`orca_program()`: `ORCA_CLI_COMMAND` → `orca-dev` → `orca-ide`(Linux) → `orca`). 모든 호출에 `--json`을 붙인다. ID·tuple·핸들·요청 ID·제목·본문·cursor가 `-`로 시작하면 부르지 않고 실패로 돌려준다(검증 뒤 수정 R2-1). `timeout`은 호출당 제한 시간이고 부르는 쪽이 설계 9.2절 표의 값으로 만든다 |
| `Orca::run_create(objective)`, `run_use(run)` | `runId`를 읽는다 |
| `Orca::worker_start(&StartRequest) -> Result<StartOutcome, ExtError>` | `StartRequest { run, spec, worktree, target, ready_timeout, request }`, `StartTarget { NewTerminal { agent, model, effort } \| Existing { terminal } }`. spec은 stdin(`--spec-file -`), 프로세스 제한 시간은 `ready_timeout` + 30초. `StartOutcome::Started(Started { dispatch, task, run, terminal, state, run_created })`: 종료 코드 0. `Failed(StartFailure { dispatch, failed_stage, last_error, next, residual })`: 종료 코드가 0이 아니고 `result`가 있다. `Unknown(글)`: 제한 시간 초과, JSON이 아닌 출력, 필수 필드가 없는 영수증. `Err`는 시작되지 않은 것이 확실한 실패다(실행 파일 없음은 `NotFound`, Orca가 `result` 없이 `error`로 거절한 것은 `Failed`). `dispatch`는 이때 예약 기록을 지우고 종료 코드 4로 끝난다(설계 8.5절 8단계) |
| `Orca::check(run: Option<&RunId>, mode) -> CheckResult` | `CheckMode { Peek \| Consume \| PeekWait { types, timeout } \| AckLegacy(deliveryId) }`. `CheckResult { messages, delivery_id, timed_out }`. `cancelled`·`connectionLost`는 `Err`다. `id`가 없는 메시지는 `ExtError::Parse`다 |
| `Orca::worker_show`, `worker_list` | `WorkerView { dispatch, state, stage, outcome, liveness(첫 낱말), terminal }`. `settled()`: `state`가 `succeeded`·`failed`이거나 `stage`가 `settled`·`released`. `failed()`: `state` 또는 `outcome`이 `failed`. `worker_list`는 `nextCursor`를 따라 이어 읽는다 |
| `Orca::send_escalation`, `terminals_in` | `orchestration send --type escalation …`, `terminal list --worktree path:<경로>`의 `connected`인 핸들 |
| `Judge::ask(state, questions) -> Result<Judged, JudgeError>` | `Question { id, kind: Choice { options } \| Noul, instructions }`, `Answer { Choice { id, choice, probabilities } \| Noul { id, value } }`, `Judged { answers, usage: Usage { input_tokens, output_tokens, cost }, model, cached }`, `JudgeError { Offline, NoKey, Unsafe, Request(글), Response(글) }`. 구현은 `fullops_ext::jev`의 `JevCurl`(환경 변수, 그다음 작업 디렉터리에서 위로 찾은 `.fullops/.env`에서 키를 찾는다)과 `JevCurl::at(repo_root, env_file) -> JevCurlAt`(`--env-file`과 `--repo`를 지킨다. `provider()`가 `typesafe` \| `openrouter` \| 없음을 돌려준다)다. 전송 형식은 설계 9.3절이고 `curl`은 PATH에서 찾은 전체 경로로 띄운다 |
| `find_program(name) -> Option<PathBuf>`, `program_version(name) -> Known<String>` | `find_program`의 정본은 `fullops_ext::proc`이고 `fullops_ext::hosts`가 다시 내보낸다. PATH의 절대 경로 항목만 보고 상대 항목은 보지 않는다. `proc::run`이 이름만 준 실행 파일(git·orca·curl·kill·taskkill)을 이것으로 찾은 전체 경로로 띄운다(검증 뒤 수정 R2-7) |
| `skill_roots(host) -> Vec<PathBuf>` | 사용자 스킬 폴더(설계 9.5절 표, 그 순서). `claude`: `$CLAUDE_CONFIG_DIR`(없으면 `~/.claude`)의 `skills`. `codex`: `~/.agents/skills`, `$CODEX_HOME`(없으면 `~/.codex`)의 `skills`. `grok`: `~/.agents/skills`, `$GROK_HOME`(없으면 `~/.grok`)의 `skills`. `agy`: `~/.gemini/config/skills`, `~/.gemini/antigravity-cli/skills`, `~/.gemini/skills` |

설치 계획의 타입은 `fullops_core::deps`에 있다(`PLANS.md`의 `model::Plan`과 이름이 같으므로 경로로 가린다. `explore::Decision`과 `model::Decision`도 같다): `Plan { per_host: BTreeMap<HostId, Result<Vec<Step>, Refusal>>, skills: Vec<Step> }`, `Step { stage: InstallStage(Plugin \| Deps), kind: StepKind, argv, what, hosts }`, `StepKind { MarketAdd, MarketUpdate, PluginInstall, SelfInstall, SkillInstall, Assemble, HookFile }`. 같은 모듈에 묶음 F가 더한 것: `HostCommands`와 `HOST_COMMANDS`(호스트별 명령 표), `host_commands(host)`, `ALL_HOSTS`, `HOOK_TIMEOUTS`와 `pre_tool_matcher(host)`(hook 등록 파일의 타임아웃과 매처. `xtask dist`와 `grok_hook_json`이 함께 쓴다), `package_items(host)`(조립하는 패키지의 항목), `normalize_source(source)`, 자리표시자 `PACKAGE_ROOT`·`GROK_HOOK_FILE`·`FIXED_BIN`과 그것을 실제 경로로 바꾸는 `Plan::bind(package_root, grok_hook_file, fixed_bin)`(`install_plan`은 순수 함수라 경로를 모른다).

묶음이 더한 그 밖의 진입점: `fullops::cmd::runtime_stage(plugin_root: Option<&Path>) -> io::Result<Runtime>`(F. 실행본 단계. `install --only runtime`과 S-04가 같은 일을 한다).

내장 자산의 상수는 `fullops::assets`(`crates/fullops/src/assets.rs`)에 있다. 네 상수의 이름과 파일 자리, 네 파일의 표 머리와 안내 문장은 동결 뒤 바꾸지 않는다.

| 상수 | 파일(`crates/fullops/assets/repo/` 아래) | 쓰는 곳 |
|---|---|---|
| `PLANS_MD` | `PLANS.md` | `parse_plan`·`read_plan`의 `template` |
| `HANDOVER_TEMPLATE` | `templates/handover.md` | `task new`, `done`의 R-04 |
| `LINT_JSON` | `config/lint.json` | `lint_excludes`의 `template_lint_json` |
| `DELIVERABLES_README` | `docs/deliverables/README.md` | 산출물 인덱스 표의 기준 |

## 9. 시험 도구(`fullops-testkit`)

### 9.1 임시 레포와 임시 홈

`Sandbox::with(&Setup)`은 임시 루트 아래에 다음을 만든다. 모든 프로세스는 격리된 환경으로 띄운다(`Sandbox::command`): `HOME`·`USERPROFILE`·`GIT_CONFIG_GLOBAL`은 임시 홈, `ORCA_CLI_COMMAND`는 fake-orca, `PATH` 맨 앞은 `bin/`이고 실제 호스트 CLI(`claude`·`codex`·`grok`·`agy`)나 `npx`가 든 폴더는 PATH에서 뺀다, `FULLOPS_OFFLINE=1`. `ORCA_*`·`GROK_*`·`CODEX_HOME`·`CLAUDE_CONFIG_DIR`·API 키 환경 변수는 지운다. 실제 호스트 CLI, 네트워크, 사용자 홈은 건드리지 않는다.

| 경로 | 내용 |
|---|---|
| `lead/` | 주 체크아웃. 브랜치 `main`. `.fullops/fullops.toml`과 `PLANS.md`가 커밋돼 있다 |
| `origin.git/` | 원격(bare). `remote = true`일 때 |
| `<역할 ID>/` | 역할 워크트리. 브랜치 `fullops/<역할 ID>` |
| `home/` | 임시 홈(`sb.home`) |
| `orca/`, `host/`, `curl/` | fake-orca·fake-host·fake-curl의 각본과 호출 기록(`FAKE_ORCA_DIR`, `FAKE_HOST_DIR`, `FAKE_CURL_DIR`) |
| `bin/` | `install_fakes(names)`가 가짜 실행 파일을 그 이름으로 놓는 폴더 |

`Setup`의 필드: `mode`(기본 `coor`), `roles`(역할 ID → kind, 기본 `{dev = "worker"}`), `remote`(기본 true), `primary_role`, `routing`(기본 `legacy`), `tech_plan_role`, `test_level`(기본 `lite`), `parallel`(기본 `on`), `host`(기본 `claude`. `claude`·`codex`·`grok`·`agy`).

`bin(name)`은 `target/<profile>/`의 바이너리(`fullops`, `fake-orca`, `fake-host`, `fake-curl`)를 돌려주고, 시험 프로세스마다 이름당 한 번 `cargo build`로 최신인지 맞춘다.

### 9.2 가짜 실행 파일

셋 모두 같은 각본 규약을 쓴다(`fullops_testkit::fake`). 각본 폴더는 환경 변수가 가리키는 폴더이고, 환경 변수가 없으면 작업 디렉터리다.

| 파일 | 형식 |
|---|---|
| `script.json` | `{"rules":[{"when":[낱말…],"times":정수,"sleep_ms":정수,"exit":정수,"stdout":값,"stderr":글}]}`. `when`의 낱말이 argv에 그 순서로 모두 있으면 맞는다. 위에서부터 처음 맞고 `times`(없으면 무제한)를 다 쓰지 않은 규칙의 응답을 낸다. `stdout`이 글이면 그대로, 그 밖이면 JSON 한 줄로 낸다 |
| `calls.jsonl` | 호출마다 `{"argv":[…],"cwd":글,"rule":번호 \| null,"stdout":값,"exit":정수, …실행 파일별 필드}` 한 줄 |

| 실행 파일 | 놓이는 이름 | 각본에 없는 호출 | 더 남기는 필드 |
|---|---|---|---|
| `fake-orca` | `ORCA_CLI_COMMAND` | `worker-start --retry-request <ID>`: 같은 요청 ID로 각본의 답을 낸 적이 있으면 그 답을 다시 낸다(각본보다 먼저 본다. `"replayed": true`). `orchestration check`: 우편함으로 답한다(아래). 그 밖: `{"ok":false,"error":{"code":"unscripted",…}}`와 종료 코드 1 | `terminal`(`ORCA_TERMINAL_HANDLE`), `request`, `replayed`, `stdin`(`--spec-file -`의 spec) |
| `fake-host` | `claude`·`codex`·`grok`·`agy`·`npx` | `--version`에는 `<이름> 0.0.0-fake`, 그 밖에는 빈 출력과 종료 코드 0(아무것도 설치하지 않는다). `when`은 `[<실행 파일 이름>, <인자>…]`와 맞춘다 | `program` |
| `fake-curl` | `curl` | 종료 코드 7(연결 실패) | `stdin`(`curl --config -`로 받은 글 전부) |

fake-orca의 우편함: `mail.jsonl`의 줄은 `{"run":"<Run>","message":{…}}`다(`Sandbox::orca_mail(run, message)`). `check --run <R> --peek`는 그 Run의 안 읽은 메시지를 읽음으로 바꾸지 않고 내고, 소비형 `check`는 돌려준 묶음을 읽음으로 바꾼다(`mail.read`). `--wait`인데 없으면 `timedOut: true`, `--types`가 있으면 그 종류만, `--ack`면 빈 묶음을 낸다.

응답과 preamble의 모양은 `orca_shapes.rs`에 있고 `scenarios/fixtures/orca/`의 표본과 같은 필드를 쓴다(시험이 견준다). 호스트별 hook 입력의 모양·등록 인자·출력 읽기는 `host_shapes.rs`에 있고 `scenarios/fixtures/host/<호스트>/`의 표본과 견준다.

`ScriptJudge`(`fullops_testkit::judge`)는 각본의 답을 돌려주는 `Judge` 구현이다. 각본은 JSON 배열이고 `ask`마다 앞에서부터 하나씩 쓴다. 다 쓰면 `JudgeError::Offline`이다. 원소는 `{"answers":[{"id","choice","probabilities"} | {"id","value"}],"usage":{"input_tokens","output_tokens","cost"},"model","cached"}` 또는 `{"error":"offline"|"no_key"|"unsafe"|"request"|"response"}`다. `ScriptJudge::new(Vec<Value>)`, `ScriptJudge::load(path)`, 받은 호출은 `asked`에 남는다. 프로세스 안의 시험에만 쓴다: 시나리오는 `fullops`를 프로세스로 돌리므로 `ScriptJudge`를 쓰지 못하고, `fake-curl`에 요청이 가야 성립하는 단계는 묶음 C의 `JevCurl`이 합쳐질 때까지 `pending = "C"`로 둔다(설계 10.2절 끝의 표, 12.5절).

### 9.3 시나리오 파일

`scenarios/<ID 소문자>.toml` 하나가 시나리오 하나다(설계 12.2절의 ID마다 하나. 경우가 많으면 `<id>-<경우>.toml`). `scenario::load`·`parse`, `run_scenario`로 읽고 돌린다. 상태 파일을 직접 주입하지 않는다. 파일의 주인은 설계 18.2절 표의 "시나리오" 칸에 그 ID가 있는 묶음이다.

| 키 | 타입 | 뜻 |
|---|---|---|
| `name` | 글 | 필수 |
| `origin` | 글 | 근거(설계 12.2절의 ID 등) |
| `[setup]` | `Setup` | 9.1절. 모르는 키는 오류다 |
| `[[step]] act` | 글 | 동사(9.4절) |
| `[[step]] <인자>` | | 동사의 인자 |
| `[[step]] env` | 표(이름 → 글) | hook과 CLI를 부르는 동사에 더할 환경 변수 |
| `[[step]] hook_args` | 문자열 배열 | hook을 부르는 동사가 등록 인자 뒤에 붙인다. `rs` 구동기만 쓴다 |
| `[[step]] df` | 글(`"DF-n"`) | 그 단계의 판정 차이를 설명하는 설계 12.3절 ID. `xtask compare`만 읽는다 |
| `[[step]] py` | 글 | `py` 구동기가 그 단계에서 내야 하는 판정. `xtask compare`만 읽는다. 두 구동기는 모르는 단계 키를 무시한다 |
| `[[step]] pending` | 글 | 그 단계가 기대는 다른 묶음의 글자. 있으면 구동기가 그 단계를 실행하지 않고 판정 `pending`("보류")으로 적는다. G가 마지막에 0건인지 확인한다 |
| `[[step]] expect` | 표 | 적은 항목만 비교한다. 모르는 키는 오류다 |
| `expect.verdict` | 글 | hook: `pass`·`context`·`warn`·`deny`·`block`. CLI: `pass`(0)·`deny`(2)·`human`(3)·`external`(4)·`error`(그 밖). 호스트마다 다른 출력 글은 `host_shapes::verdict_of`가 이 어휘로 옮긴다 |
| `expect.rule` | 글 | 출력의 `FullOps[<ID>]` 가운데 이 ID가 있어야 한다 |
| `expect.flow` | `continues` \| `stopped` | 통과·안내·경고는 `continues`, 그 밖은 `stopped` |
| `expect.exit` | 정수 | CLI 종료 코드 |
| `expect.escape` | 글 | 출력에 이 글(탈출구 명령 이름)이 있어야 한다. `rs` 구동기에서만 단언한다 |

구동기는 `[setup] host`의 호스트가 주는 입력 모양과 등록 인자로 hook을 부른다: `claude`·`codex`·`grok`은 `fullops hook <host>`, `agy`는 `fullops hook agy --event <이름>`. 그 호스트에 없는 이벤트(Codex의 StopFailure, agy의 UserPromptSubmit)와 도구(Claude Code 밖의 Workflow)는 hook을 부르지 않고 `pass`로 적는다. agy는 `user_prompts`의 글을 다음 hook 입력의 `lastUserInput`에 싣는다.

### 9.4 동사 목록

정본은 `fullops_testkit::VERBS`다. `who`의 기본값은 `lead`이고 `lead` 또는 역할 ID다. `session`을 주면 세션 ID가 `sid-<who>-<session>`이 된다(기본은 `sid-<who>`). 동결 뒤 `driver.rs`는 A 담당만 고친다. 새 동사가 꼭 필요하면 A 담당에게 요청한다.

| 동사 | 인자(`?`는 선택) | `rs` 구동기가 하는 일 |
|---|---|---|
| `session_starts` | who?, session?, source? | SessionStart hook |
| `user_prompts` | who?, session?, text | UserPromptSubmit hook(agy는 다음 hook 입력의 `lastUserInput`) |
| `preamble_arrives` | who, session?, dispatch?, task? | Orca preamble을 실은 사용자 입력 |
| `runs_shell` | who?, session?, command, dialect? | 셸 도구의 PreToolUse hook. 명령은 실행하지 않는다 |
| `writes_file` | who?, session?, path, content | 쓰기 도구(Codex는 `apply_patch` Add File)의 PreToolUse hook. 막히지 않으면 파일을 쓴다 |
| `edits_file` | who?, session?, path, old, new | 편집 도구(Codex는 `apply_patch` Update File)의 PreToolUse hook. 막히지 않으면 파일을 고친다 |
| `deletes_file` | who?, session?, path | 빈 내용의 쓰기(Codex는 `apply_patch` Delete File)의 PreToolUse hook. 막히지 않으면 파일을 지운다 |
| `spawns_subagent` | who?, session?, prompt | 하위 에이전트 도구(`Agent`·`spawn_agent`·`spawn_subagent`·`invoke_subagent`)의 PreToolUse hook |
| `runs_workflow` | who?, session?, script? | Workflow 도구의 PreToolUse hook(Claude Code만) |
| `stops` | who?, session?, reason? | Stop hook. `reason`은 호스트의 종료 이유 값(grok·agy) |
| `turn_fails` | who?, session?, error? | StopFailure hook(agy는 오류로 끝난 Stop. Codex에는 없다) |
| `runs_fullops` | who?, session?, args, raw? | 셸 도구의 PreToolUse hook 뒤에 `fullops <args>` CLI. `raw = true`면 CLI만 |
| `file` | who?, path, content | hook 없이 파일을 쓴다(준비) |
| `git` | who?, args | hook 없이 git을 실행한다(준비) |
| `commits` | who?, message? | hook 없이 `git add -A`와 `git commit`(준비) |
| `orca_replies` | when, stdout?, times?, exit?, sleep_ms?, stderr? | fake-orca 각본에 규칙을 덧붙인다 |
| `orca_sends` | run?, id?, type?, dispatch?, task?, subject?, body?, outcome? | fake-orca의 우편함에 메시지를 넣는다 |
| `host_replies` | when, stdout?, times?, exit?, sleep_ms?, stderr? | fake-host 각본에 규칙을 덧붙인다 |
| `curl_replies` | when, stdout?, times?, exit?, sleep_ms?, stderr? | fake-curl 각본에 규칙을 덧붙인다 |
| `fakes` | names | 가짜 실행 파일(`claude`·`codex`·`grok`·`agy`·`npx`·`curl`)을 PATH 앞에 놓는다 |
| `lead_routes` | key, role?, class? | `fullops route <KEY> --request … --class <class> --role <R>`와 tuple |
| `lead_plans` | key, role, goal?, source?, status?, after? | `fullops plan add` |
| `lead_writes_task` | key, role, goal? | `fullops task new` |
| `lead_dispatches` | key, role, purpose?, reason?, sha?, terminal?, run?, agent?, model?, effort?, bare?, chat? | `fullops dispatch`. `bare`가 아니면 아직 안 한 route와 task new를 먼저 한다. `chat = true`면 터미널 없는 worker |
| `worker_subdispatches` | role, key, brief, agent?, model?, effort? | worker가 `fullops dispatch <KEY> --brief …`로 하위 worker를 띄운다 |
| `worker_works` | role, key, path?, lines? | 배정받은 세션을 시작하고 코드 파일을 고쳐 커밋한다 |
| `worker_lints` | role | `fullops lint` |
| `worker_finishes` | role, failed?, followup?, no_checks? | `fullops done` |
| `worker_reports` | role, key | 배정(아직이면) → worker_works → worker_lints → worker_finishes → fake-orca의 우편함에 worker_done → lead_waits |
| `lead_waits` | run?, ack?, no_wait? | `fullops wait --run <R> [--ack <메시지 ID>]… [--no-wait]` |
| `lead_merges` | key | `fullops merge <KEY>` |

에이전트가 실행하는 `fullops` 명령은 실제 흐름처럼 셸 도구의 PreToolUse hook(T-12가 `current.json`을 쓴다)을 먼저 거친다. 흐름 동사(`lead_*`, `worker_*`)는 아직 하지 않은 앞 단계를 스스로 먼저 한다. `lead_routes`·`lead_writes_task`·`lead_dispatches`·`lead_waits`·`worker_subdispatches`·`worker_reports`는 단계의 `env`를 받는다(통합에서 바꿨다). `worker_reports`는 인박스의 완료 보고 절을 채워 커밋한다. 바이너리 폴더는 환경 변수 `FULLOPS_TESTKIT_BIN_DIR`로 줄 수 있다.
