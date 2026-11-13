# fullops 파일 형식

이 문서는 [design.md](design.md) 6절의 형식을 필드 단위로 옮겨 적은 것이다. 다른 묶음은 설계 문서 전체를 다시 읽지 않고 이 문서로 형식을 본다. 설계 문서가 정본이고, 둘이 어긋나면 이 문서가 틀린 것이다.

- Rust 타입은 모두 `fullops_core::model`에 있다. 이 문서의 표는 그 타입의 serde 형식과 같다.
- 시각은 모두 에폭 초(`i64`)다. 날짜 문자열을 쓰지 않는다.
- "쓰는 주체" 열이 비어 있으면 그 객체를 쓰는 주체와 같다.
- † 표시는 설계 문서에 값이나 필드가 없어 뼈대(묶음 A)가 정한 것이다. 설계 문서를 고칠 때 함께 확정한다.
- 형식을 바꿔야 하면 `fullops-core::model`과 이 문서를 함께 고친다. 묶음 A 담당에게 요청한다(설계 18.3절).

## 1. 식별자와 공통 타입

| 타입 | 문법 | 비고 |
|---|---|---|
| `TaskKey` | `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$` | 과제 키 |
| `RoleId` | `^[a-z][a-z0-9_-]{0,31}$` | 역할 ID. `reviewer`도 이 문법에 맞는다 |
| `Sha` | 40자 소문자 hex | `Sha::short(7)`은 표시용, `Sha::short(12)`는 리뷰 폴더 이름 |
| `SessionId` | `[A-Za-z0-9_-]{1,100}` | 호스트 값은 `SessionId::from_host`가 밖의 글자를 `_`로 바꾸고 100자로 자른다 |
| `RunId`, `DispatchId`, `MessageId` | 비어 있지 않고 200바이트 이하이며 공백·제어 문자가 없다 † | Orca가 준 값이다. `DispatchId`는 `pending:<run>:<started_at>`일 수 있다 |
| `DecisionId` | `D` + 3자리 이상 숫자 | `DecisionId::from_number(next_decision)` |
| `ModelTuple` | `{ "agent": 글, "model": 글, "effort": 글 }` | 세 값이 모두 있어야 한다 |
| 경로 글 | `/` 구분, Windows의 `\\?\` 접두어와 끝의 `/` 없음 † | `path_text()`로 만들고 `same_path()`로 비교한다(Windows는 대소문자 무시) |

파일에서 읽은 식별자가 문법에 안 맞으면 그 파일 전체를 읽지 못한 것으로 본다(hook은 `Unknown`).

## 2. 파일의 자리

경로는 `fullops_store::Repo`의 메서드로 얻는다. 문자열로 직접 잇지 않는다.

| 자리 | 경로 | `Repo` 메서드 |
|---|---|---|
| 커밋되는 파일 | `<작업 트리>/.fullops-squad/` | `squad()` |
| | `fullops.toml` | `config_path()` |
| | `PLANS.md` | `plan_path()` |
| | `handovers/to_<역할>.md` | `inbox_path(role)` |
| | `handovers/logs/<KEY>.<역할>.r<차수>.md` | `handover_log_path(key, role, round)` |
| | `routes/<KEY>.json` | `route_path(key)` |
| | `reviews/<KEY>/<head 12자>/` | `review_dir(key, head)` |
| Git 공용 디렉터리(모든 워크트리 공유) | `<git-common-dir>/fullops/` | `shared()` |
| | `state.json`, `state.json.lock`, `state.json.tmp.<pid>`, `state.json.broken-<시각>` | `state_path()` |
| | `status.json` | `status_path()` |
| | `archive.jsonl` | `archive_path()` |
| | `turns.jsonl` | `turns_path()` |
| | `waiters/<run>.json` | `waiter_path(run)` |
| | `py-backup/` | (묶음 F) |
| 작업 트리별 Git 디렉터리 | `<git-dir>/fullops/` | `local()` |
| | `sessions/<sid>.json`, `sessions/<sid>.json.lock` | `session_path(sid)` |
| | `sessions/<sid>.seen/<이벤트>` | `seen_path(sid, event)` |
| | `current.json` | `current_path()` |
| | `trace/<sid>.jsonl` | `trace_path(sid)` |
| | `lint/result.json`, `lint/pass.json`, `lint/logs/<명령>.log`, `lint/lock` | `lint_dir()` |
| 사용자 홈 | `~/.fullops/` (Windows는 `USERPROFILE`, 그 밖은 `HOME`) | (묶음 F) |

`waiters/<run>.json`의 파일 이름은 Run ID에서 `[A-Za-z0-9_.-]` 밖 글자를 `_`로 바꾼 것이다 †.

## 3. 커밋되는 파일

### 3.1 `fullops.toml`

활성 표식이자 `Config`다. 쓰는 주체는 `init`·`mode`·`upgrade`·`migrate`와 사람이다. 해석은 `fullops_store::parse_config`(글)와 `load_config`(작업 트리의 파일)가 한다.

| 키 | 타입 | 필수 | 뜻 |
|---|---|---|---|
| `schema` | 정수 | 필수 | `2`여야 한다 |
| `applied_version` | 글 | 필수 | 이 레포에 적용한 fullops 버전. `upgrade`가 올린다 |
| `mode` | `"coor"` \| `"dev"` | 필수 | |
| `primary_role` | `RoleId` | dev 모드에서 필수 | 등록 역할이어야 한다 |
| `primary_branch` | 글 | dev 모드에서 필수 | |
| `routing` | `"legacy"` \| `"split"` | 필수 | |
| `tech_plan_role` | `RoleId` | split에서 필수 | design 역할과 달라야 한다 |
| `test_level` | `"minimal"` \| `"lite"` \| `"standard"` \| `"full"` \| `"exhaustive"` | 필수 | 뒤로 갈수록 높다 |
| `subagent_level` | `"off"` \| `"lite"` \| `"standard"` \| `"full"` | 필수 | 뒤로 갈수록 높다 |
| `[git] base` | 글 | 필수 | 기본 브랜치 |
| `[git] remote` | 글 | 선택 | 없으면 로컬 전용이다 |
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

모르는 키는 무시한다. `[roles]`가 없으면 역할이 없는 것으로 본다 †.

정책 오류(`PolicyError`)의 종류는 셋이다.

| `PolicyKind` | `as_str()` | 언제 |
|---|---|---|
| `Unreadable` | `config_unreadable` | 파일을 읽지 못했다. TOML이 아니다. 필수 키가 없다. 값이 어휘나 식별자 문법에 안 맞는다 |
| `Invalid` | `config_invalid` | `schema != 2`, 역할 브랜치 중복, kind 중복, dev 모드의 `primary_*` 누락이나 미등록 역할, split의 `tech_plan_role` 누락이나 design 역할과 같음 |
| `ModeChanged` | `mode_changed` | 세션 파일의 `mode_at_start`가 현재 `[mode, primary_role, primary_branch]`와 다르다(`config::mode_changed`) |

신원에 쓰는 값은 작업 트리의 파일에서, 검사 기준에 쓰는 값(`[review]`, `[code]`, lint 명령 선택의 `test_level`)은 merge-base 커밋의 blob에서 읽는다. 그 커밋에 파일이 없으면 `ReviewPolicy::default()`와 `CodeDef::default()`를 쓴다.

### 3.2 `PLANS.md`

쓰는 주체는 lead, `plan add|set`, `merge`다. 해석은 `fullops_core::parse_plan(text, template)`이 하고 결과는 `Plan { rows, delegation, parse_errors }`다.

| 대상 | 규칙 |
|---|---|
| 과제 표 | 헤더 줄 다음 줄이 구분선(`\|---\|…`)이고, 헤더에 `과제` 또는 `키`가 든 열과 `상태`가 든 열이 모두 있는 표. 여럿이면 모두 읽고 파일 순서를 유지한다 |
| 열 찾기 | 키: `과제`·`키`. 상태: `상태`. 목표: `목표`·`결과`·`다음`·`내용`. 담당: `담당`·`책임`. 원천: `지시서`·`원천`. 이 순서로 찾고 이미 정한 열은 다시 쓰지 않는다 † |
| 칸 정규화 | `[글](주소)` → `글`, `*`와 `` ` `` 삭제, 앞뒤 공백 제거. `\|`는 칸 안의 `\|`다 † |
| 키 칸 | 빈 행은 버린다. `TaskKey` 문법에 안 맞으면 `parse_errors`에 줄 번호(1부터)를 넣고 버린다 |
| 상태 칸 | `보류(`로 시작 → `Held(괄호 안)`. `대기` 뒤 첫 비공백 글자가 `(`·`（` → `WaitHuman(괄호 안)`. 그 밖의 `대기…` → `Ready`. 나머지 → `Other(원문)`. 괄호 안은 짝이 맞는 닫는 괄호 전까지다 † |
| 선행 | 상태 칸의 `선행:` 뒤를 쉼표와 공백으로 나눠, 끝의 마침표를 떼고 `TaskKey` 문법에 맞는 것만 `after`에 넣는다 |
| 담당 | 담당 칸 맨 앞의 `[A-Za-z0-9_-]+`. `RoleId` 문법(소문자)에 안 맞으면 `role = None`이다 † |
| `## 위임 범위` | 제목 다음부터 다음 `# `·`## ` 제목 전까지의 비어 있지 않은 줄 가운데 `template`의 줄(앞뒤 공백 제거 뒤 같음)이 아닌 것. 최대 10줄 |
| `## 완료 보고 후속 요청` | `- [ ] <역할>: <글> (<KEY>)` 줄. `merge`가 덧붙인다. `Plan`에는 담지 않는다 |
| `## 배정 기록` | `- <YYYY-MM-DD HH:MM> <KEY> → <역할> \| <agent>/<model>/<effort> \| <fresh\|retain> \| run <id> \| 재배정 <n> \| <이유>` 줄. `merge`가 덧붙인다. `Plan`에는 담지 않는다 |

`PlanRow`의 필드는 `key`, `goal`, `role`, `status`, `after`, `source`, `line`(1부터 세는 줄 번호)이다.

### 3.3 인박스 `handovers/to_<역할>.md`와 문서 front matter

front matter는 파일 맨 앞의 `---` 줄과 다음 `---` 줄 사이다. 해석은 `fullops_core::parse_front_matter`가 하고, 인박스 전용 필드는 `frontmatter::inbox_of`가 `Inbox`로 바꾼다. 산출물 문서도 같은 파서로 읽는다.

| 문법 | 규칙 |
|---|---|
| `이름: 값` | 값은 첫 `:` 뒤부터 줄 끝까지이고 앞뒤 공백을 뗀다. 따옴표와 `#`은 값의 일부다(주석 문법이 없다) |
| `이름: [a, b]` | 쉼표로 나눠 앞뒤 공백을 뗀 목록. `[]`는 빈 목록 |
| 이름 | `[A-Za-z0-9_-]+` † |
| 빈 줄 | 건너뛴다 † |
| 그 밖의 줄 | 오류(`FrontMatterError::BadLine(줄 번호)`). 맨 앞 `---`가 없으면 `Missing`, 닫는 `---`가 없으면 `Unclosed` |
| BOM, CRLF | 맨 앞의 BOM과 줄 끝의 CR은 무시한다 † |

| 필드 | 타입 | 뜻 | 쓰는 주체 |
|---|---|---|---|
| `id` | 글 | 문서 ID. 인박스는 `handover-<역할>` | `task new\|reopen`, `doc stamp` |
| `title` | 글 | 인박스는 `<KEY> — <목표>` | 같음 |
| `status` | `draft` \| `review` \| `approved` | 문서 상태(과제 상태가 아니다) | 같음 |
| `updated` | `YYYY-MM-DD` | | 같음 |
| `owner` | 글 | | 같음 |
| `summary` | 글 | | 같음 |
| `tasks`, `upstream`, `downstream` | 목록 | 산출물 문서의 목록 필드(설계 8.12절). 쓰는 순서는 `id, title, status, statuses, updated, owner, tasks, upstream, downstream, summary` | `doc stamp` |
| `key` | `TaskKey` | 과제 키. 인박스 전용 | `task new\|reopen`만 |
| `role` | `RoleId` | 담당 역할. 인박스 전용 | 같음 |
| `goal` | 글 | 한 줄 목표. 인박스 전용 | 같음 |
| `base` | `Sha` | `changed()`의 시작점. 인박스 전용 | 같음 |
| `route` | `direct` \| `design` \| `product` | 분류. 인박스 전용 | 같음 |
| `from` | `lead` 또는 작성한 역할 ID | 인박스 전용 | 같음 |
| `round` | 정수 | `task reopen`마다 1 증가. 인박스 전용 | 같음 |
| `budget` | 정수 | 반복 예산(기본 2). 인박스 전용 | 같음 |
| `test_level` | `TestLevel` | 작성 시점의 설정 값. 인박스 전용 | 같음 |
| `subagent_level` | `SubagentLevel` | 작성 시점의 설정 값. 인박스 전용 | 같음 |

`fullops_store::read_inbox`는 파일이 없으면 `Yes(None)`, 인박스 전용 필드 열 개 가운데 하나라도 없거나 문법에 안 맞으면 `Unknown("inbox_syntax")`를 돌려준다 †. 끝난 과제의 인박스는 `done`이 `handovers/logs/<KEY>.<역할>.r<round>.md`로 `git mv` 한다.

### 3.4 `routes/<KEY>.json`

타입은 `Route`다. 쓰는 주체는 `route`와 `dispatch`(tuple 기록)다. `fullops_store::read_route`·`write_route`로 읽고 쓴다.

| 필드 | 타입 | 뜻 |
|---|---|---|
| `v` | 정수 | `1` |
| `key` | `TaskKey` | |
| `class` | `direct` \| `design` \| `product` \| `unresolved` \| `untracked` | `untracked`는 분류 없이 tuple만 기록한 경우(dev 직접 작업의 리뷰 키) |
| `role` | `RoleId` \| null | `unresolved`면 null † |
| `deliverables` | 글 목록 | 산출물 ID(`D03` 등). 확률 0.5 이상을 높은 순으로 최대 3개 |
| `origin` | `user` \| `bot` | 과제가 들어온 곳 |
| `decided_by` | `jev` \| `fallback` \| `manual` | |
| `reason` | 글 | `--reason` 값. 없으면 빈 글 |
| `request_file` | 글 \| null | `--request-file` 경로 |
| `request_sha256` | 글 \| null | 요청 글의 sha256 |
| `probabilities` | 객체(이름 → 수) | `scope`, `role` |
| `thresholds` | 객체(이름 → 수) | `scope` 0.8, `role` 0.6, `deliverable` 0.5 |
| `tuples` | 객체(역할 ID 또는 `reviewer` → `{agent, model, effort, why}`) | 기록일 뿐이고 다음 과제의 기본값으로 쓰지 않는다 |
| `created_at` | 시각 | |
| `history` | `Route` 목록 | 재분류 전의 객체들. 각 원소의 `history`는 `[]`다 |

### 3.5 `reviews/<KEY>/<head 12자>/`

쓰는 주체는 `review`와 리뷰어다. `accepted.json`과 `waiver.json`은 묶음 E가 쓰고 묶음 B(X-02)가 읽으므로 타입(`Accepted`, `Waiver`)이 `model`에 있다. 나머지 파일의 타입은 묶음 E가 정의한다.

| 파일 | 필드 |
|---|---|
| `targets.json` | `v`(1), `key`, `base`, `merge_base`, `head`, `policy_sha256`(`review/rule.json`의 sha256), `files[]`: `path`, `status`(`A`·`M`·`D`·`R`), `rule`(문장 \| null), `preset`(`"skipped"` \| null), `reason` |
| `result.json` | `v`(1), `key`, `base`, `head`, `reviewer_session`(글 \| null), `implementer_session`, `snapshot_path`, `snapshot_head`, `read_only`(true), `conclusion`(`accept` \| `reject` \| null), `files[]`: `path`, `status`, `review_status`(`reviewed` \| `skipped` \| null), `reason`. `findings[]`: `path`, `content`, `severity`(`critical`·`high`·`medium`·`low`), `resolved`, `start_line`, `end_line` |
| `report.md` | 리뷰 보고서 틀. 기계는 읽지 않는다 |
| `lint.json` | 같은 base·head의 `lint/result.json` 사본(5.5절) |
| `accepted.json` | `v`(1), `key`, `base`, `head`, `result_sha256`, `lint_sha256`, `reviewer_session`, `implementer_session`(글 \| null), `at`. 두 해시가 현재 파일과 같을 때만 유효하다 |
| `waiver.json` | `v`(1), `key`, `base`, `head`, `reason`, `decision`(`DecisionId`), `decided_by`(`"user"`), `need`(`Need` 객체), `at`. 덮어쓰지 않는다 |
| `superseded.json` | `by`, `old_result_sha256`, `new_result_sha256`, `at` |
| `abandoned.json` | 설계가 필드를 정하지 않았다. 묶음 E가 정하고 이 표에 적는다 |

`Need` 객체: `required`(불), `action`(`skip` \| `ask` \| `review`), `reason`, `code_files`, `code_lines`, `always`(걸린 경로 목록), `base`, `head`, `merge_base`.

### 3.6 `lint/lint.json`, `review/rule.json`

1.24.0의 형식을 그대로 읽는다(변환하지 않는다). 사람이 쓴다. 설계가 이름을 든 키는 다음과 같고, 전체 키와 타입은 묶음 D(`lint.json`)와 묶음 E(`rule.json`)가 1.24.0 템플릿을 읽고 이 절에 적는다.

| 파일 | 설계가 든 키 |
|---|---|
| `lint/lint.json` | `commands[]`: `name`, `run[]`, `cwd`, `kind`, `level`, `required`, `paths`, `timeout_seconds`. `exclude`(glob 목록), `timeout_seconds`(기본 600), `parallel_tests`, `size`: `severity`, `max_code_lines`, `max_doc_lines`, `max_diff_added`. `rules[]`: `code`, `file_extensions`, `exclude_paths`, `exclude_patterns`, `multiline`, `enabled` |
| `review/rule.json` | `include`, `exclude`, `rules[]`: `path`(glob)와 규칙 문장 |

산출물 인덱스 `docs/deliverables/README.md`의 표 문법(미정-M7)은 묶음 D가 이 문서에 적는다.

## 4. Git 공용 디렉터리

### 4.1 `state.json`

타입은 `State`다. 열린 것만 담는다. 쓰기는 `fullops_store::update_state` 하나로만 한다(잠금 → 다시 읽기 → 수정 → `state.json.tmp.<pid>`에 쓰고 `sync_all` → `rename` → `status.json` 교체 → 잠금 해제). 읽기는 `read_state`가 잠그지 않고 한다.

- 파일이 없으면 빈 상태(`State::default()`)다.
- JSON으로 읽히지 않으면 `Unknown("state_broken")`, `v`가 `STATE_V`(1)보다 크면 `Unknown("state_newer")`다. `update_state`는 각각 `StoreError::Corrupt("state_broken")`·`Corrupt("state_newer")`를 돌려주고 파일을 건드리지 않는다.
- 모든 객체는 모르는 필드를 보존한다(`extra`, `#[serde(flatten)]`). `ModelTuple`만 예외다 †.
- 값이 없는 필드는 `null`로 쓴다. 읽을 때는 없는 필드를 `null`로 본다.

최상위:

| 필드 | 타입 | 뜻 | 쓰는 주체 |
|---|---|---|---|
| `v` | 정수 | `1` | `update_state` |
| `seq` | 정수 | 쓸 때마다 1 증가 † | `update_state` |
| `runs` | `Run` 목록 | | `dispatch`, Stop의 X-05 효과 |
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
| `opened_at` | 시각 | |
| `closed_at` | 시각 \| null | 미정산 dispatch와 미처리 메시지가 없으면 X-05가 닫는다 |

`Dispatch` (`dispatches[]`):

| 필드 | 타입 | 뜻 | 쓰는 주체 |
|---|---|---|---|
| `id` | `DispatchId` | Orca 결과에 ID가 없으면 `pending:<run>:<started_at>`으로 쓰고 P-01·터미널 결합 때 실제 값으로 바꾼다 | `dispatch` |
| `task_id` | 글 \| null | Orca task ID | `dispatch` |
| `run` | `RunId` | | `dispatch` |
| `key` | `TaskKey` | | `dispatch` |
| `role` | `RoleId` | | `dispatch` |
| `purpose` | `build` \| `fix` \| `review` \| `research` | | `dispatch` |
| `tuple` | `ModelTuple` | | `dispatch` |
| `terminal` | 글 \| null | Orca 터미널 핸들 | `dispatch` |
| `worktree` | 경로 글 | 대상 워크트리(리뷰는 snapshot) | `dispatch` |
| `session_mode` | `fresh` \| `retain` | | `dispatch` |
| `reason` | null \| `"limit"` | | `dispatch` |
| `started_at` | 시각 | | `dispatch` |
| `bound_session` | `SessionId` \| null | 이 dispatch를 묶은 세션 | P-01 효과 |
| `reported` | `Reported` \| null | `fullops done`의 기록 | `done` |
| `settled` | `Settled` \| null | null이면 미정산이다 | `wait`·T-11, X-03 효과 |
| `limit_escalated` | 불 | F-01이 escalation을 보냈다 | F-01 효과, `wait` |

`Reported`: `branch`(글), `sha`(`Sha`, `done` 시점의 HEAD), `lint_head`(`Sha` \| null, `--no-checks`로 끝냈으면 null †), `followups[]`(`{ "role": RoleId, "text": 글 }`), `at`(시각).

`Settled`: `how`(`done` = worker_done 수신 또는 Orca 조회로 확인, `escalated` = escalation 수신, `lost` = worker 사망 감지), `outcome`(`succeeded` \| `failed` \| null. `how == done`일 때만 있다 †), `message`(`MessageId` \| null), `at`(시각).

`Completion` (`completions[]`):

| 필드 | 타입 | 뜻 | 쓰는 주체 |
|---|---|---|---|
| `id` | `MessageId` | 보고 메시지의 ID. dev 직접 작업은 `direct:<sha 앞 12자>` † | `wait`·T-11, `done` |
| `dispatch` | `DispatchId` \| null | 직접 작업이거나 dispatch를 못 찾았으면 null † | 같음 |
| `key` | `TaskKey` \| null | 과제 키를 못 찾았으면 null. lead가 `completion set-sha --key`로 채운다 | 같음 |
| `role` | `RoleId` \| null | dispatch를 못 찾았으면 null † | 같음 |
| `branch` | 글 \| null | | 같음 |
| `sha` | `Sha` \| null | null이어도 통합 대기로 센다 | 같음, `completion set-sha` |
| `sha_source` | `done` \| `body` \| `branch_tip` \| `manual` \| null | `done` = `fullops done` 기록, `body` = 메시지 본문, `branch_tip` = 역할 브랜치 로컬 ref, `manual` = `completion set-sha` | 같음 |
| `direct` | 불 | dev 주 담당 직접 작업의 기록. 통합 차단(X-06, D-03)에서 빼고 단계와 지표에만 쓴다 | `done` |
| `outcome` | `succeeded` \| `failed` | 타입은 `WorkOutcome`. `failed`는 통합 대기로 세지 않는다 | `wait`·T-11 |
| `received_at` | 시각 | | 같음 |
| `hold` | `Hold` \| null | 보류 | `hold` |
| `released_hold` | `Hold` \| null | 재개했거나 통합된 보류. 지우지 않고 여기로 옮긴다 | `resume`, `merge`·`settle`·X-06 효과 |
| `landed_at` | 시각 \| null | 통합이 처음 확인된 시각. 한 번 쓰면 다시 계산하지 않는다 | `merge`, `settle`, X-06 효과 |
| `drift` | 정수 \| null | 통합 시점에 기본 브랜치가 보고 SHA 뒤의 역할 브랜치 커밋(기록 전용 제외)을 함께 담았으면 그 수 | 같음 |

`Hold`: `reason`, `owner`, `resume`(재개 조건) 세 글과 `at`(시각 †). 셋을 다 가져야 한다.

통합 대기(`Completion::is_pending`, `State::pending`): `direct == false`, `outcome == succeeded`, `hold == null`, `landed_at == null`.

`Message` (`messages[]`): `id`(`MessageId`), `delivery`(글 \| null, Orca 전달 ID), `run`(`RunId`), `type`(`worker_done` \| `question` \| `escalation`), `dispatch`(`DispatchId` \| null), `received_at`(시각), `acked_at`(시각 \| null, null이면 미처리).

`TaskCounters` (`tasks[<KEY>]`): `rounds`(정수, `task reopen`마다 1 증가), `budget`(정수, 기본 2), `limit_redispatch`(정수, `dispatch --reason limit`마다 1 증가), `last_tuple`(`ModelTuple` \| null).

`Snapshot` (`snapshots[]`):

| 필드 | 타입 | 뜻 |
|---|---|---|
| `review` | 글 | `<KEY>@<head 앞 12자>` |
| `path` | 경로 글 | `<레포 부모>/.fullops-review-<무작위 8자>` |
| `head`, `base` | `Sha` | |
| `state` | `creating` \| `active` \| `removed` \| `abandoned` \| `missing` | |
| `implementer_session` | 글 \| null | |
| `reviewer_dispatch` | `DispatchId` \| null | |
| `owner` | `"lead"` \| `"dispatch:<DispatchId>"` | |
| `created_at` | 시각 | |
| `hold` | 글 \| null | 정리하지 못한 사유 |

`Decision` (`decisions[]`, 뜻은 설계 8.10절):

| 필드 | 타입 | 뜻 |
|---|---|---|
| `id` | `DecisionId` | |
| `status` | `open` \| `answered` \| `dropped` † | |
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
| `repository` | 글 | `--repository`. 값이 있을 때만 쓴다 † |
| `drop_reason` | 글 | `drop --reason`. 값이 있을 때만 쓴다 † |

### 4.2 `archive.jsonl`

추가만 한다(`fullops_store::append_archive`). 쓰는 주체는 `merge`·`settle`·`gc`·`migrate`다. 타입은 `ArchiveLine`이고 줄은 둘 가운데 하나다.

| 줄 | 필드 |
|---|---|
| 과제 | `key`(`TaskKey`), `role`(`RoleId` \| null), `sha`(`Sha` \| null), `received_at`(시각), `landed_at`(시각 \| null), `rounds`(정수), `dispatches[]`(`purpose`, `started_at`, `reason`), `direct`(불) |
| 결정 | `{"decision": <Decision 객체>}` |

`stage_of`의 `archive` 인자는 `landed_at`이 있는 과제 줄의 키 집합이다(`fullops_store::archive_keys`).

### 4.3 `turns.jsonl`

`{"at": 시각, "sid": SessionId, "kind": 글}` 한 줄. 타입은 `Turn`이다. P-02가 `O_APPEND`로 `write` 한 번에 쓰고 잠그지 않는다. 줄은 200바이트 이하다.

| `kind` | 뜻 |
|---|---|
| `human` | 사람 입력 |
| `machine:orca-message` | Orca가 세션을 깨운 입력 |
| `machine:task-notification` | 호스트의 백그라운드 작업 알림 |

### 4.4 `status.json`

타입은 `Status`다. `update_state`가 같은 잠금 안에서 다시 만든다(다른 곳에서 쓰지 않는다). Orca 패널과 봇이 읽는다. token, 이슈 본문, 댓글, 프롬프트는 넣지 않는다.

| 필드 | 타입 | 뜻 |
|---|---|---|
| `v` | 정수 | `1` |
| `generated_at` | 시각 | |
| `mode` | `coor` \| `dev` \| null | 설정을 읽지 못했으면 이전 값, 그것도 없으면 null † |
| `version` | 글 | 이 파일을 쓴 실행본의 버전 † |
| `tasks[]` | | `state.json`에 dispatch·completion·`tasks` 항목이 있는 키마다 하나 † |
| `tasks[].key` | `TaskKey` | |
| `tasks[].role` | `RoleId` \| null | 가장 늦은 dispatch의 역할, 없으면 가장 늦은 completion의 역할 † |
| `tasks[].stage` | `none` \| `routed` \| `assigned` \| `reported` \| `reviewing` \| `merged` \| `done` | `stage_of` |
| `tasks[].rounds`, `tasks[].budget` | 정수 | `tasks[<KEY>]`의 값. 없으면 0과 2 |
| `tasks[].sha` | 글 \| null | 가장 늦은 completion의 SHA 앞 7자 |
| `tasks[].held` | 불 | 통합되지 않은 completion에 보류가 있다 |
| `dispatches[]` | | `key`, `role`, `purpose`, `agent`, `model`, `started_at`, `settled`(불) |
| `pending[]` | | 직접 작업이 아니고 성공했고 통합되지 않은 completion(보류 포함) † |
| `pending[].key` | `TaskKey` \| null | |
| `pending[].sha` | 글 \| null | SHA 앞 7자 |
| `pending[].landing` | `merge_left` \| `push_left` \| null | 같은 키·SHA의 이전 값을 잇는다. 계산된 적이 없으면 null † (5절의 열린 문제) |
| `pending[].hold_reason` | 글 \| null | |
| `decisions[]` | | 열린 결정만: `id`, `key`, `question`, `options`, `asked_at`, `expires_at` |
| `ready` | `TaskKey` 목록 | `update_state`에 `lead_plan`을 줬을 때만 다시 계산하고 그 밖에는 이전 값을 유지한다 |
| `sync_pending` | 객체(`RoleId` → SHA 앞 7자) | |
| `metrics` | 객체 | `since`(첫 사람 턴의 시각), `human_turns`, `done_tasks`, `turns_per_task`(수 \| null), `unattended_dispatches`, `unattended_tasks`. 계산은 `fullops_core::metrics`(묶음 C) |
| `issue` | null | v1에서는 항상 null(이슈 자동 모드 자리) |

### 4.5 `waiters/<run>.json`

`{"pid": 정수, "sid": SessionId | null, "heartbeat": 시각}`. 타입은 `Waiter`다. `fullops wait`가 쓰고 30초마다 `heartbeat`를 갱신하며 끝날 때 지운다. X-05가 `heartbeat`가 90초 이내인지 본다.

## 5. 작업 트리별 Git 디렉터리

### 5.1 `sessions/<sid>.json`

타입은 `Session`이다. hook과 `route`가 쓴다. `create_session`(없을 때만 만든다), `update_session`(잠금 `sessions/<sid>.json.lock` 안에서 다시 읽어 고친다. 파일이 없으면 `StoreError::Missing`), `read_session`으로 다룬다. `v`가 `SESSION_V`(1)보다 크면 `Unknown("session_newer")`다. 모르는 필드는 보존하지 않는다 †.

| 필드 | 타입 | 뜻 | 쓰는 주체 |
|---|---|---|---|
| `v` | 정수 | `1` | S-02 |
| `sid` | `SessionId` | | S-02 |
| `host` | `claude` \| `codex` | | S-02 |
| `started_at` | 시각 | | S-02 |
| `last_seen` | 시각 | 14일 지나면 lead의 SessionStart가 지운다 | P-00 |
| `terminal` | 글 \| null | `ORCA_TERMINAL_HANDLE` | S-02 |
| `mode_at_start` | `[mode, primary_role \| null, primary_branch \| null]` | 현재 값과 다르면 정책 오류다 | S-02 |
| `anchor` | `Sha` \| null | 첫 SessionStart의 HEAD. 처음 만들 때 한 번만 쓰고 resume·compact에서 유지한다. 커밋이 없는 레포에서는 null † | S-02 |
| `turn` | 정수 | UserPromptSubmit마다 1 증가 | P-00 |
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
| `host` | `claude` \| `codex` | |
| `ev` | `SessionStart` \| `UserPromptSubmit` \| `PreToolUse` \| `Stop` \| `StopFailure` | |
| `actor` | 글 \| null | `Actor::label()`: `lead`, `role:<id>`, `reviewer`, `other`. dispatch가 묶였으면 뒤에 `+dispatch` † |
| `intents` | 글 목록 | `Intent` 변형 이름(`OrcaWorkerStart` 등) |
| `rules[]` | | `id`(규칙 ID), `verdict`(`context`·`warn`·`deny`·`block`·`effect`·`unknown` †), `fp`(지문 \| null) |
| `git`, `orca` | 정수 | 띄운 프로세스 수 |
| `ms` | 정수 | 걸린 시간 |
| `err` | 글 \| null | 오류 종류(`lock_busy`, `timeout`, `panic`, `io` 등) |

### 5.5 `lint/result.json`

타입은 `LintResult`다. `lint`(묶음 D)가 쓰고 묶음 B·C·E가 읽는다.

| 필드 | 타입 | 뜻 |
|---|---|---|
| `v` | 정수 | `1` |
| `base` | `Sha` | `--from`을 푼 값 |
| `merge_base` | `Sha` | `merge-base(from, HEAD)`. 설정을 읽는 기준 시점 |
| `head` | `Sha` | |
| `config_sha256` | 글 | 기준 시점 설정의 sha256 |
| `test_level` | `TestLevel` | 적용 레벨 |
| `started_at` | 시각 | |
| `seconds` | 수 | |
| `commands[]` | | `name`, `kind`, `run[]`, `cwd`, `status`(`passed` \| `failed` \| `timeout` \| `unrunnable` \| `skipped`), `exit_code`(정수 \| null), `seconds`, `reason`, `output_tail`(끝 40줄), `log`(로그 경로) |
| `findings[]` | | `code`, `severity`(`ERROR` \| `WARNING`), `path`, `line`(정수 \| null), `message`, `suggestion` |
| `errors`, `warnings`, `unrunnable` | 정수 | `failed`·`timeout`은 ERROR로 센다 |
| `passed` | 불 | `errors == 0 && unrunnable == 0` |

### 5.6 `lint/pass.json`

타입은 `LintPass`다. 통과했을 때만 있다(`fullops_store::read_lint_pass`). 필드는 `v`(1), `base`(`Sha`, merge-base), `head`(`Sha`), `tree`(`Sha`), `config_sha256`(글), `test_level`(`TestLevel`), `at`(시각)이다.

## 6. 사용자 홈 `~/.fullops/`

묶음 F가 쓴다.

| 경로 | 내용 |
|---|---|
| `bin/fullops[.exe]` | 고정 위치 실행본 |
| `bin/fullops.old-<시각>[.exe]` | 교체 전 실행본. `install --prune`만 지운다 |
| `bin/fullops.new-<pid>` | 설치 중의 임시 파일 |
| `install.json` | `version`, `sha256`, `source`, `at`, `previous` |
| `cache/jev/<요청 sha256>.json` | 분류 응답 캐시(묶음 C) |

## 7. hook 입력과 출력

호스트 입력의 해석(`Host::decode`)은 `fullops/src/hook/host_claude.rs`와 `host_codex.rs`만 한다. 결과는 `HookInput`이고 `fullops-core`는 JSON을 모른다.

| 입력 필드(snake_case만) | `HookInput` | 비고 |
|---|---|---|
| `hook_event_name` | `event: EventKind` | 모르는 이름이면 `{}` |
| `session_id` | `sid: SessionId` | `SessionId::from_host` |
| `cwd` | `cwd: PathBuf` | |
| `source` | `source` | SessionStart: `startup`·`clear`·`resume`·`compact` |
| `prompt` | `prompt` | UserPromptSubmit |
| `tool_name`, `tool_input` | `tool: ToolCall` | PreToolUse |
| `reason` | `stop_reason` | Stop |
| `error` | `error` | StopFailure. 앞 300자 |

`ToolCall`은 어댑터가 `tool_input`에서 필요한 값만 뽑아 채운다 †.

| 필드 | 타입 | 채우는 값 |
|---|---|---|
| `name` | 글 | `tool_name` |
| `kind` | `ToolKind` | `Host::tool_kind(name)`: `Shell(Bash\|PowerShell)`, `Write`, `Edit`, `Patch`, `SubAgent`, `Other` |
| `command` | 글 \| 없음 | 셸 명령(`tool_input.command`). `Patch`에서는 패치 본문 |
| `file_path` | 글 \| 없음 | `Write`·`Edit`·`MultiEdit`의 `file_path` |
| `content` | 글 \| 없음 | `Write`의 `content` |
| `edits` | `EditOp { old, new, replace_all }` 목록 | `Edit`는 하나(`old_string`, `new_string`, `replace_all`), `MultiEdit`는 `edits[]` |
| `text` | 글 \| 없음 | `SubAgent`: `prompt`·`message`·`description`을 줄바꿈으로 이은 것 |

출력은 두 호스트에 같은 형태로 낸다(`Host::encode`). 문구는 `fullops_core::messages`의 `refusal`·`block`·`warn`으로 만든다.

| `Verdict` | stdout |
|---|---|
| `Pass` | `{}` |
| `Context(t)` | `{"hookSpecificOutput":{"hookEventName":"<이벤트>","additionalContext":t}}` |
| `Warn(t)` | `{"systemMessage":"FullOps: " + t}` |
| `Deny { rule, reason }` | `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"FullOps[<규칙>]: <사유> 다음: <탈출구>"}}` |
| `Block { rule, reason, also }` | `{"decision":"block","reason":"FullOps[<규칙>]: <사유> 다음: <탈출구> 함께 남은 것: <also>"}` |

종료 코드는 항상 0이다. 뼈대의 hook은 어떤 입력에도 `{}`를 낸다.

## 8. CLI 공통 규약

구현은 `fullops/src/cli.rs`다.

| 항목 | 규약 |
|---|---|
| 형태 | `fullops <명령> [인자] [--repo <경로>] [--json]`. 두 옵션은 명령 이름 뒤 어디에나 올 수 있고 `cli::run`이 먼저 떼어 낸다 † |
| 종료 코드 | 0 성공(`EXIT_OK`), 1 내부 오류·사용법 오류(`EXIT_ERROR`), 2 규칙에 의한 거절(`EXIT_REFUSED`), 3 사람 손이 필요함(`EXIT_HUMAN`), 4 바깥 도구 실패(`EXIT_EXTERNAL`) |
| 거절 출력 | 사람용은 stderr에 건마다 `FullOps[<규칙>]: <사유> 다음: <탈출구>` 한 줄 †. `--json`이면 stdout에 `{"ok":false,"refused":[{"rule","reason","escape"}]}`. 종료 코드는 걸린 `Refusal.exit`의 최댓값이다 † |
| 그 밖의 오류 | 사람용은 stderr에 글 한 줄. `--json`이면 stdout에 `{"ok":false,"error":"<글>"}` † |
| 활성 표식 없음 | "활성 레포가 아닙니다", 종료 코드 2. `init`·`install`·`doctor`·`migrate`·`rules`는 제외 † |
| 정책 오류 | 종료 코드 2 †. `doctor`·`mode`·`init`·`migrate`·`upgrade`·`status`·`why`·`install`·`rules`는 받는다 |
| 호출자 판정 | `Ctx::caller()`. `current.json`(10분 이내)의 `sid`로 세션을 찾고 `facts::resolve_actor`(설계 7.3절)로 정한다. 세션이 없으면 브랜치와 터미널 핸들로만 정한다 |
| 상태 쓰기 | `Ctx::update_state()`. 잠금 대기 10초. `state_broken`이면 `state.json.broken-<시각>`으로 옮기고 빈 상태로 시작한 뒤 종료 코드 1로 알린다 |
| 명령 추가 | `cmd/mod.rs`의 `COMMANDS` 표에 이미 모든 명령이 있다. 각 묶음은 자기 `cmd/<이름>.rs`의 `run(ctx, args)`만 채운다. 한 파일이 여러 명령을 맡으면 `ctx.command`로 가른다(`hold.rs`: `hold`·`resume`·`settle`·`completion`) |

`fullops wait`의 반환(JSON, 설계 8.6절): `run`, `idle_timeout`, `check_minutes`, `actionable[]`(`delivery`, `id`, `type`, `key`, `role`, `sha`, `sha_source`, `outcome`, `body`, `notes[]`), `absorbed`(`heartbeats`, `status`), `lost[]`(`dispatch`, `key`, `role`), `next`. 본문(`body`)은 출력에만 있고 저장하지 않는다.

## 9. 시험 도구(`fullops-testkit`)

### 9.1 임시 레포

`Sandbox::with(&Setup)`은 임시 루트 아래에 다음을 만든다. 모든 프로세스는 격리된 환경(`HOME`·`USERPROFILE`·`GIT_CONFIG_GLOBAL`, `ORCA_CLI_COMMAND`=fake-orca, `FULLOPS_OFFLINE=1`)으로 띄운다.

| 경로 | 내용 |
|---|---|
| `lead/` | 주 체크아웃. 브랜치 `main`. `.fullops-squad/fullops.toml`과 `PLANS.md`가 커밋돼 있다 |
| `origin.git/` | 원격(bare). `remote = true`일 때 |
| `<역할 ID>/` | 역할 워크트리. 브랜치 `fullops/<역할 ID>` |
| `home/` | 격리된 홈 |
| `orca/` | fake-orca의 각본과 호출 기록(`FAKE_ORCA_DIR`) |

`Setup`의 필드: `mode`(기본 `coor`), `roles`(역할 ID → kind, 기본 `{dev = "worker"}`), `remote`(기본 true), `primary_role`, `routing`(기본 `legacy`), `tech_plan_role`, `test_level`(기본 `lite`), `subagent_level`(기본 `lite`), `host`(기본 `claude`).

### 9.2 fake-orca

별도 실행 파일(`crates/fullops-testkit/src/bin/fake-orca.rs`)이다.

| 파일 | 형식 |
|---|---|
| `script.json` | `{"rules":[{"when":[낱말…],"times":정수,"sleep_ms":정수,"exit":정수,"stdout":값,"stderr":글}]}`. `when`의 낱말이 argv에 그 순서로 모두 있으면 맞는다. 위에서부터 처음 맞고 `times`(없으면 무제한)를 다 쓰지 않은 규칙의 응답을 낸다. `stdout`이 글이면 그대로, 그 밖이면 JSON 한 줄로 낸다 |
| `calls.jsonl` | 호출마다 `{"argv":[…],"cwd":글,"terminal":글 \| null,"rule":번호 \| null,"stdout":값}` 한 줄 |
| 각본에 없는 호출 | stdout `{"ok":false,"error":{"code":"unscripted","message":…}}`, 종료 코드 1 |

응답과 preamble의 모양은 `orca_shapes.rs` 한 곳에 있다. 실제 Orca 계약(미정-M3)이 확정되면 그 파일만 고친다.

### 9.3 시나리오 파일

`scenarios/<묶음>/<이름>.toml` 하나가 시나리오 하나다(`scenario::load`, `run_scenario`). 상태 파일을 직접 주입하지 않는다.

| 키 | 타입 | 뜻 |
|---|---|---|
| `name` | 글 | 필수 |
| `origin` | 글 | 근거(설계 12.2절의 ID 등) |
| `[setup]` | `Setup` | 9.1절. 모르는 키는 오류다 |
| `[[step]] act` | 글 | 동사(9.4절) |
| `[[step]] <인자>` | | 동사의 인자 |
| `[[step]] expect` | 표 | 적은 항목만 비교한다. 모르는 키는 오류다 |
| `expect.verdict` | 글 | hook: `pass`·`context`·`warn`·`deny`·`block`. CLI: `pass`(0)·`deny`(2)·`human`(3)·`external`(4)·`error`(그 밖) |
| `expect.rule` | 글 | 출력의 `FullOps[<ID>]` 가운데 이 ID가 있어야 한다 |
| `expect.flow` | `continues` \| `stopped` | 통과·안내·경고는 `continues`, 그 밖은 `stopped` |
| `expect.exit` | 정수 | CLI 종료 코드 |
| `expect.escape` | 글 | 출력에 이 글(탈출구 명령 이름)이 있어야 한다. `rs` 구동기에서만 단언한다 |

설계 12.1절의 예시는 한 줄에 `;`로 여러 키를 적었지만 TOML 문법이 아니다. 실제 파일은 키마다 줄을 나눈다.

### 9.4 동사 목록

정본은 `fullops_testkit::VERBS`다. `who`의 기본값은 `lead`이고 `lead` 또는 역할 ID다. `session`을 주면 세션 ID가 `sid-<who>-<session>`이 된다(기본은 `sid-<who>`).

| 동사 | 인자(`?`는 선택) | `rs` 구동기가 하는 일 |
|---|---|---|
| `session_starts` | who?, session?, source? | SessionStart hook |
| `user_prompts` | who?, session?, text | UserPromptSubmit hook |
| `preamble_arrives` | who, session?, dispatch?, task? | Orca preamble을 실은 UserPromptSubmit hook |
| `runs_shell` | who?, session?, command, dialect? | 셸 도구의 PreToolUse hook. 명령은 실행하지 않는다 |
| `writes_file` | who?, session?, path, content | Write(Codex는 `apply_patch` Add File)의 PreToolUse hook. 막히지 않으면 파일을 쓴다 |
| `edits_file` | who?, session?, path, old, new | Edit(Codex는 `apply_patch` Update File)의 PreToolUse hook. 막히지 않으면 파일을 고친다 |
| `deletes_file` | who?, session?, path | 빈 내용의 Write(Codex는 `apply_patch` Delete File)의 PreToolUse hook. 막히지 않으면 파일을 지운다 |
| `spawns_subagent` | who?, session?, prompt | 하위 에이전트 도구의 PreToolUse hook |
| `stops` | who?, session?, reason? | Stop hook |
| `turn_fails` | who?, session?, error? | StopFailure hook |
| `runs_fullops` | who?, session?, args, raw? | 셸 도구의 PreToolUse hook 뒤에 `fullops <args>` CLI. `raw = true`면 CLI만 |
| `file` | who?, path, content | hook 없이 파일을 쓴다(준비) |
| `git` | who?, args | hook 없이 git을 실행한다(준비) |
| `commits` | who?, message? | hook 없이 `git add -A`와 `git commit`(준비) |
| `orca_replies` | when, stdout?, times?, exit?, sleep_ms? | fake-orca 각본에 규칙을 덧붙인다 |
| `lead_routes` | key, role?, class? | `fullops route <KEY> --request … --class <class> --role <R>`와 tuple |
| `lead_plans` | key, role, goal?, source?, status?, after? | `fullops plan add` |
| `lead_writes_task` | key, role, goal? | `fullops task new` |
| `lead_dispatches` | key, role, purpose?, reason?, sha?, terminal?, run?, bare? | `fullops dispatch`. `bare`가 아니면 아직 안 한 route와 task new를 먼저 한다 |
| `worker_works` | role, key, path?, lines? | 배정받은 세션을 시작하고 코드 파일을 고쳐 커밋한다 |
| `worker_lints` | role | `fullops lint` |
| `worker_finishes` | role, failed?, followup?, no_checks? | `fullops done` |
| `worker_reports` | role, key | 배정(아직이면) → worker_works → worker_lints → worker_finishes → fake-orca가 worker_done을 전달 → lead_waits |
| `lead_waits` | run?, ack? | `fullops wait` |
| `lead_merges` | key | `fullops merge <KEY>` |

에이전트가 실행하는 `fullops` 명령은 실제 흐름처럼 셸 도구의 PreToolUse hook(T-12가 `current.json`을 쓴다)을 먼저 거친다. 흐름 동사(`lead_*`, `worker_*`)는 묶음 C·E의 명령이 구현된 뒤에야 끝까지 돈다. 새 동사는 `driver.rs`의 `VERBS`와 `Driver::step`에 덧붙인다.

## 10. 열린 문제

뼈대를 만들며 드러난, 형식에 영향을 주는 미결 사항이다. 설계 문서를 고친 뒤 이 문서의 † 표시와 함께 정리한다.

| 항목 | 지금의 처리 |
|---|---|
| `status.json`의 `pending[].landing` | `update_state`는 git을 부를 수 없어(`store`는 `ext`를 모르고 바깥 호출은 잠금 밖) 같은 건의 이전 값만 잇는다. 값을 새로 계산해 넣을 길이 `update_state`의 시그니처에 없다 |
| `read_plan`의 템플릿 | 설계의 시그니처는 `read_plan(repo)`지만 내장 템플릿은 바이너리의 `assets/`에 있어 `read_plan(repo, template)`으로 받는다 |
| front matter의 따옴표 | 1.24.0이 쓴 문서에는 `title: "…"`처럼 JSON 따옴표로 감싼 값과 `statuses: {…}` 객체가 있다. 설계 문법대로 따옴표를 떼지 않고 값의 일부로 둔다 |
| direct completion의 `id` | 메시지가 없어 `direct:<sha 12자>`로 정했다 |
| `Decision`의 `repository`·`drop_reason` | `decision add --repository`와 `decision drop --reason`의 값을 둘 필드가 설계 예시에 없어 선택 필드로 더했다 |
| 세션 파일의 모르는 필드 | 설계가 보존을 요구하지 않아 보존하지 않는다 |
