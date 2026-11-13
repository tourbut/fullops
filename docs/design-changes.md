# 설계 개정 기록 (2026-11-14)

- 대상: `docs/design.md`.
- 반영한 것: (1) 계약 확인 결과 `docs/contracts/design-delta.md`(Claude Code 2.1.323, Codex 0.163.0, Orca 1.4.256). (2) 레포 주인의 결정(2026-11-13~14): 이름과 버전 확정, 이슈 자동 모드 미룸 확정, Jev 탐색을 v1에, 의존 플러그인·스킬 설치를 v1에, 네 호스트(Claude Code·Codex·grok·agy) 모두에 설치, 그 밖의 버리는 기능 확정. (3) 마무리 개정(같은 날): grok·agy 계약 확인 결과 `docs/contracts/hosts-delta.md`, 뼈대 담당이 보고한 설계 빈틈 48건, 모션그래픽 스킬 `fullops-motion`의 포함, 병렬 진행 방침(레포 주인의 새 요구). (4) 동결 전 점검(같은 날): 다른 모델이 동결 직전의 설계와 뼈대를 함께 점검해 낸 지적 16건과, 뼈대 담당이 `docs/formats.md`에 †로 남긴 두 번째 빈틈 20건. 설계와 뼈대를 함께 고쳤다(6절).
- 읽는 순서: 뼈대(묶음 A) 담당은 0절과 `docs/design.md`만 보고 고치면 된다. 0절은 앞 개정과 마무리 개정을 합친 완전한 목록이다. 1절은 앞 개정의 절 번호 순 목록, 2절은 계약 확인(`design-delta.md`)이 "결정 필요"로 남긴 것을 어떻게 정했는지, 4절은 개정본을 다른 모델이 점검해 낸 지적 19건의 처리다. 5절이 마무리 개정이고 6절이 동결 전 점검 반영이다. 6절이 바꾼 타입·시그니처·형식은 0절의 표에도 넣었다(0절은 여전히 완전한 목록이다). 1~4절의 서술 가운데 마무리 개정이 뒤집은 것(agy에 hook이 없다, `host_for(arg, raw)`, `subagent_level`, `status.json`은 그대로다, `~/.fullops/package/<버전>`이 통합 패키지의 사본이다)은 0절과 5절이 정본이다.
- 표기: "절"은 `docs/design.md`의 절 번호다.

## 0. 뼈대가 따라 바꿔야 하는 것

뼈대(커밋 `1b87273`)는 개정 전 설계로 만들어졌다. 그 뒤의 개정 전부(앞 개정 + 마무리 개정)에서 타입·trait·상태 형식·규칙 표·CLI 표면·파일 자리 가운데 바뀐 것을 모두 적는다. 여기에 없는 것은 그대로다. "절"은 `docs/design.md`의 절 번호다. 뼈대가 임시로 정했고 설계가 그대로 받아들인 것은 0.8절에 모았다(고칠 것이 없다).

### 0.1 타입 (5절, 6절, 7.2절, 7.4절, 7.5절, 10.2절)

| 타입 | 바뀐 내용 | 절 |
|---|---|---|
| `HostId` (새로) | `enum HostId { Claude, Codex, Grok, Agy }`. 문자열 `claude`·`codex`·`grok`·`agy`. 뼈대의 `HostKind`를 이 타입으로 바꾼다 | 5 |
| `SubagentLevel` → `Parallel` | `enum Parallel { Off, Read, On, Ultra }`. 문자열 `off`·`read`·`on`·`ultra`. `SubagentLevel`은 지운다 | 5, 8.15 |
| `Config` | `subagent_level: SubagentLevel` → `parallel: Parallel` | 5, 6.1.1 |
| `Actor` | 필드 `sub: bool` 추가(묶인 dispatch가 하위 배정이다). `resolve_actor`가 `state.json`의 `sub_of`로 채운다 | 5, 7.3 |
| `Caps` | 필드 아홉: `stop_failure`, `background_wake`, `prompt_event`, `context_at_start`, `context_at_prompt`, `user_warn`, `subagent_tool: &'static str`, `workflow`, `orca_agent: &'static str`. 자리는 `model/hook.rs`. 값은 7.2절의 능력 표 | 5, 7.2 |
| `Route` | 필드 `requires_packet: bool` 추가. serde 기본값 `false` | 5, 6.1.4 |
| `PacketCategory`, `PacketCheck`, `Packet`, `PacketOutcomes` (새로) | 6.1.5절의 필드 그대로. `PacketCheck`는 `Delivery`·`Completion` | 5, 6.1.5, 8.14 |
| `Deps` (새로) | 10.7절의 `deps.toml` 필드 그대로. 자리는 `model` | 5, 10.7 |
| `PlanRow.role` | 담당 칸의 토큰을 소문자로 바꾼 뒤 `RoleId` 문법을 본다(뼈대는 바꾸지 않고 `None`으로 뒀다) | 5, 6.1.2 |
| `FmValue`, `FrontMatter` (새로) | `enum FmValue { Text(String), List(Vec<String>), Map(BTreeMap<String, String>) }`, `struct FrontMatter { fields: Vec<(String, FmValue)> }` | 6.1.3 |
| `HookInput.source` | 어휘는 `startup`·`clear`·`resume`·`compact`·`fork`. 어댑터가 호스트의 값을 이 어휘로 옮긴다(grok의 `new` → `startup`, `load` → `resume`. agy는 항상 `startup`) | 7.2 |
| `HookInput.prompt` | agy는 모든 이벤트의 `lastUserInput`을 넣는다 | 7.2 |
| `HookInput.stop_reason` | 유지한다. 정상 종료는 `None` 또는 `"end_turn"`. agy의 `NO_TOOL_CALL`은 어댑터가 `end_turn`으로 옮긴다 | 7.2 |
| `HookInput.error` | 뜻이 바뀐다: 오류 **종류 이름**(`rate_limit` 등. agy는 `"error"`) | 7.2 |
| `HookInput.error_details`, `agent_id` (새로) | 둘 다 `Option<String>`. `error_details`는 오류 글의 앞 300자. `agent_id`는 읽기만 한다 | 7.2 |
| `ToolCall` | 필드를 설계가 정했다: `name`, `kind`, `command`, `file_path`, `content`, `edits: Vec<EditOp>`, `text`, `truncated: bool`. `EditOp { old, new, replace_all }`. 뼈대의 구조체에 `truncated`를 더하고 `edits`의 원소 타입을 맞춘다 | 7.2 |
| `ToolKind` | `Shell(Dialect)`·`Write`·`Edit`·`Patch`·`SubAgent`·`Workflow`·`Other`. `Workflow`가 새로 생겼다 | 7.2 |
| `Intent::OrcaTaskCreate { spec }` | 타입은 그대로. `--spec-file`이면 `Known::Unknown` | 7.4 |
| `Rule` | `message: MsgId`(뼈대의 `messages::MsgId` 그대로). `escape: &'static str` → `escapes: &'static [(&'static str, &'static str)]`(경우, 탈출구). `origin` 유지 | 7.5 |
| `Finding::Hit` | 필드 `case: &'static str`와 `always: bool` 추가 | 7.5 |
| `Strength` | `Context`·`Effect`·`Deny`·`BlockOnce`·`BlockAlways`. 뼈대가 더한 `Warn`은 지운다(P-03이 `Context`가 됐다) | 7.5 |
| `Effect` | 변형 `SetPromptFp(String)` 추가 | 7.5, 10.2 |
| `TurnKind` | 변형 `Subagent`·`Workflow` 추가. 문자열 `subagent`·`workflow` | 6.2.2 |
| 모르는 필드 보존용 타입 | 뼈대의 `Json` enum과 `Extra`를 `serde_json::Value`의 맵(`BTreeMap<String, Value>`)으로 바꾼다. core가 `serde_json`을 쓰게 됐다 | 5, 10.1 |
| `Derived` (새로) | `struct Derived { ready: Option<Vec<TaskKey>>, landing: BTreeMap<String, Landing>, metrics: Option<Metrics> }`. `update_state`의 인자 | 6.2.3, 10.2 |
| `Metrics` | 필드 `at`, `subagent_calls`, `workflow_runs`, `sub_dispatches` 추가 | 6.2.3, 6.7 |
| `Baseline` (새로) | `{ review, code, test_level, fell_back: bool }`. `baseline`의 반환 | 6.1.1 |
| `BriefAt` (새로) | `enum BriefAt<'a> { Session, Spec { command: &'a str, key: &'a TaskKey, sub: bool } }`. `parallel_brief`의 인자. `Spec`의 값은 spec에 적는 실행 명령, 과제 키, 하위 배정인가다(6절) | 8.5, 8.15 |
| `StatusPending` | 필드 `id: MessageId` 추가(그 completion의 ID. 유도값 `landing`을 잇는 키다. 6절) | 6.2.3 |
| `StartRequest` | `{ run, spec, worktree, target: StartTarget, ready_timeout, request }`. `task` 필드 없음 | 10.2 |
| `StartTarget`, `StartOutcome`, `StartFailure` (새로), `Started` | 10.2절 그대로. `Started.terminal`이 `Option<String>` | 10.2 |
| `CheckMode`, `CheckResult` (새로) | `Peek`·`Consume`·`PeekWait { types, timeout }`·`AckLegacy(String)`. `{ messages, delivery_id, timed_out }` | 10.2 |
| `OrcaMessage` | `{ id, kind, from, subject, body, task_id, dispatch_id, outcome }`. `run_id`와 전달 ID 필드 없음 | 10.2 |
| `WorkerView` | `{ dispatch, state, stage: Option, outcome: Option, liveness(첫 낱말), terminal: Option }`와 `settled()` | 10.2 |
| `Question`, `QuestionKind`, `Answer`, `Judged`, `Usage`, `JudgeError` (새로) | 10.2절 그대로 | 10.2 |
| `Plan`, `Step`, `InstallStage` (새로) | 10.2절 그대로. 설치 계획 | 10.2, 10.3 |
| `StepKind` (새로) | `MarketAdd`·`MarketUpdate`·`PluginInstall`·`SelfInstall`·`SkillInstall`·`Assemble`·`HookFile` | 10.2 |
| `DepsState`, `HostInstall`, `DepsError` (새로) | 10.2절 그대로 | 10.2, 10.3 |
| `TreeEntry`, `NameStatus` (새로) | `TreeEntry { mode, kind, oid, path }`. `NameStatus { status: char, path, old_path: Option<String> }` | 10.2 |
| `DeliverableRow`, `AckBy`, `MessageNote`, `SkillText` (새로) | 10.2절 그대로 | 8.6, 10.2 |
| `Scope`, `IndexEntry`, `Signal`, `Decision`, `FindResult`, `ContextResult`, `PacketInput`, `PacketDecision`, `Source`, `SourceBasis` (새로) | 10.2절 그대로. 탐색 | 6.1.5, 6.3, 10.2 |

### 0.2 trait과 함수 시그니처 (7.2절, 10.2절)

| 대상 | 뼈대에 있는 것 | 바뀐 뒤 |
|---|---|---|
| `Host` trait | `decode`, `encode`, `caps`, `tool_kind`, `kind` | `fn id(&self) -> HostId`, `fn decode(&self, raw: &[u8], event_arg: Option<&str>) -> Result<HookInput, DecodeError>`, `fn encode(&self, event: EventKind, v: &Verdict) -> String`, `fn pass(&self, event_arg: Option<&str>) -> &'static str`, `fn caps(&self) -> Caps`. `tool_kind`와 `kind`는 지운다(도구 대응은 `decode`가 `ToolCall`로 옮긴다). 구현이 넷: `host_claude`, `host_codex`, `host_grok`, `host_agy`. 뼈대는 네 어댑터의 `pass`를 완성본으로 둔다(agy: `PreToolUse` → `{"decision":"ask"}`, `Stop` → `{"decision":""}`, 그 밖과 다른 호스트는 `{}`) |
| `host_for` (새로) | - | `fn host_for(arg: &str) -> Option<Box<dyn Host>>`. 인자만 본다(앞 개정의 `host_for(arg, raw)`와 입력 모양 판별은 없앴다) |
| `caps_of`, `host_of_agent` (새로) | - | `fn caps_of(host: HostId) -> Caps`, `fn host_of_agent(orca_agent: &str) -> Option<HostId>`. `model/hook.rs`. 완성본 |
| `parallel_brief` (새로) | - | `fn parallel_brief(parallel: Parallel, host: Option<HostId>, at: BriefAt) -> String`. `messages/mod.rs`. 8.15절의 고정 문구. 완성본. `Spec`이고 `sub`가 거짓이면 `on`·`ultra`의 문장 끝에 하위 배정 문장을 붙이고 `command`와 `key`를 채운다(6절) |
| `spec_command` (새로) | - | `fn spec_command(host: Option<HostId>, fixed_bin: &str) -> String`. `messages/mod.rs`. spec에 적는 실행 명령. `claude`면 `fullops`, 그 밖이면 `fixed_bin`(공백이 있으면 큰따옴표). 완성본(6절) |
| `State::sub_open` (새로) | - | `fn sub_open(&self, parent: &DispatchId) -> bool`. `model/state.rs`. 6.6절 "하위 작업 남음". X-01·X-03과 R-11이 함께 쓴다. 완성본(6절) |
| `fullops-store::fixed_bin` (새로) | - | `fn fixed_bin() -> Option<PathBuf>`. 고정 위치 실행본 `~/.fullops/bin/fullops[.exe]`. 완성본(6절) |
| `Facts` | - | `fn host(&self) -> HostId`, `fn caps(&self) -> Caps`, `fn session_handle(&self) -> Option<String>`, `fn user_skill(&mut self, name: &str) -> Known<Option<SkillText>>` 추가 |
| `resolve_actor` | 7.3절 1·2번 뒤 4~6번으로 좌석을 정한다 | 1·2번 뒤 **3~6번**으로 정한다(dispatch가 묶인 리뷰어가 `Reviewer`가 된다). `Actor.sub`를 채운다 |
| `baseline` (새로) | `parse_config`만 있다 | 함수 둘이다. `fullops-store::baseline(toml_text: Option<&str>) -> Baseline`이 글을 `parse_config`로 해석해 `fullops-core::config::baseline(parsed: Option<Result<Config, PolicyError>>) -> Baseline`에 넘긴다(core는 `toml`을 쓰지 않는다). 글이 없으면 내장 기본값과 `fell_back = false`, 읽히지 않거나 검증을 어기면 내장 기본값과 `fell_back = true`. 완성본(6절) |
| `update_state` | `update_state(repo, wait, lead_plan: Option<&Plan>, f)` | `update_state(repo, wait, derived: Option<&Derived>, f)`. `status.json`은 6.2.3절의 두 갈래로 만든다: state에서 나오는 부분은 매번, 유도값(`ready`·`landing`·`metrics`)은 넘겨받은 것만 바꾸고 나머지는 이전 값을 잇는다. 잠금 안에서 `turns.jsonl`·`archive.jsonl`을 읽지 않는다 |
| `read_plan` | `read_plan(repo, template)` | 그대로(설계가 뼈대를 따랐다) |
| `archive_keys` | 줄 전체를 타입으로 푼다 | 줄마다 `key`와 `landed_at`만 뽑는 줄 훑기 |
| `Orca::run_create` | `fn run_create(&self)` | `fn run_create(&self, objective: &str)` |
| `Orca::worker_start` | `-> Result<Started, ExtError>` | `-> Result<StartOutcome, ExtError>` |
| `Orca::check` | `check(&self, run: &RunId, peek: bool, wait: Option<Duration>) -> Result<Vec<OrcaMessage>, ExtError>` | `check(&self, run: Option<&RunId>, mode: CheckMode) -> Result<CheckResult, ExtError>` |
| `Orca::ack` | 있음 | **삭제** |
| `OrcaCli` | 빈 골격 | **메서드 전부를 완성본으로**(9.2절의 응답 해석. `scenarios/fixtures/orca/`로 시험한다) |
| `Judge::ask` | `-> Result<Vec<Answer>, JudgeError>` | `-> Result<Judged, JudgeError>`. `JevCurl`의 임시 구현은 `Err(JudgeError::Offline)` |
| `Git` | 읽기 8개, 쓰기 10개 | `ls_tree(rev)`, `blob_prefixes(oids, max)`, `grep_files(rev, literals)`, `name_status(from, to)`, `diff_u0(from, to)` 추가 |
| `GitCli` | 빈 골격 | **메서드 전부를 완성본으로**(9.1절의 환경 변수·제한 시간. 임시 레포로 시험한다) |
| `record_completion` | `record_completion(state, msg, resolve, branch_tip) -> CompletionNote` | 이름과 시그니처 변경: `record_message(state, run: &RunId, msg, resolve, branch_tip) -> MessageNote`. 모든 종류의 메시지를 받는다. 자리는 `flow.rs`(C가 채운다) |
| `orca_effort` (새로) | - | `fn orca_effort(tuple: &ModelTuple, parallel: Parallel, new_terminal: bool) -> String`. `flow.rs`. 임시 구현은 tuple의 `effort`(C가 채운다) |
| `metrics` | `flow.rs`의 빈 골격 | 그대로 두되 10.2절 끝의 표에 올랐다(C 구현, E의 `merge`가 부른다). 임시 구현은 모든 값 0 |
| `fullops-core::explore` (새로) | - | `instruction_sha256`, `source_sha256(text, is_instruction)`, `sendable_path`, `sendable_text`, `index_entry`, `batches`, `triage`, `packet_source_paths`, `build_packet`, `check_packet`. `check_packet`의 임시 구현은 "항상 빈 목록" |
| `rebuild_packet` (새로) | - | `fullops::cmd::packet`의 `fn rebuild_packet(repo, key, role, extra_seeds: &[String]) -> Result<Option<PathBuf>, Refusal>`. 임시 구현은 "항상 `Ok(None)`" |
| `parse_packet`, `parse_packet_outcomes`, `orca_wakes`, `ack_by` (새로) | - | `fullops-core::model`. 10.2절의 시그니처. 완성본 |
| `header_summary`, `lint_excludes`, `parse_deliverable_index` (새로 드러냄) | - | `fullops-core::lint_rules`. 10.2절의 시그니처. 임시 구현은 각각 빈 글, 빈 `GlobSet`, 빈 목록 |
| `fullops-core::deps` (새로) | - | `install_plan(deps, hosts, registered, package_dir) -> Plan`(임시 구현은 빈 `Plan`), `grok_hook_json(fullops_bin: &Path) -> String`, 데이터 `HOST_COMMANDS`. F가 채운다 |
| `fullops-store` | - | `read_packet`, `read_packet_outcomes`, `write_explore_result`, `update_index`, `load_deps(plugin_root, builtin)`(오류는 `DepsError`. `builtin`은 내장 사본의 글이다. 6절), `read_deps_state`, `write_deps_state` 추가. 세션 파일은 모르는 필드를 보존한다 |
| `fullops-ext::hosts` (새로) | - | `host_available`, `host_version`, `host_marketplaces`, `host_plugins`, `skill_roots`, `run_step`. `skill_roots`는 9.5절 표대로 완성본(agy의 세 폴더 포함) |
| `fullops::assets` (새로) | - | `crates/fullops/src/assets.rs`. 상수 `PLANS_MD`, `HANDOVER_TEMPLATE`, `LINT_JSON`, `DELIVERABLES_README`와 그 원본 파일 넷(`crates/fullops/assets/repo/` 아래. 10.1절의 표) |
| `fullops-testkit::ScriptJudge` (새로) | - | 각본 파일의 답을 돌려주는 `Judge` 구현. J가 C의 `JevCurl` 전에 쓴다 |

### 0.3 상태와 파일 형식 (6절)

| 파일 | 바뀐 내용 | 절 |
|---|---|---|
| `fullops.toml` | `subagent_level` → `parallel`(`off`·`read`·`on`·`ultra`). 필수 키 목록을 설계가 정했다(뼈대의 목록에서 `subagent_level`을 `parallel`로 바꾼 것과 같다). 검증 추가: `tech_plan_role`이 등록 역할이고 design 역할과 다르다, `remote`가 빈 문자열이 아니다 | 6.1.1 |
| `PLANS.md` 해석 | 담당 칸을 소문자로 바꿔 읽는다. `선행:` 토큰 끝의 `.`·`)`·`）`를 뗀다(뼈대는 마침표만). 코드 펜스 안의 줄은 읽지 않는다(뼈대는 읽는다). 열 우선순위(키·상태·목표·담당·원천), `\|`는 글자, 짝이 맞는 닫는 괄호는 뼈대 그대로다 | 6.1.2 |
| front matter | 읽는 문법을 1.24.0의 `deliverables.py`와 같게 바꾼다: 이름 문법, 공백 뒤 `#` 주석, 양끝 따옴표 떼기, `"…"`는 JSON 문자열, `[…]`는 목록(JSON 배열 또는 쉼표 나눔), `{…}`는 JSON 객체, BOM 떼기. 쓰는 규칙(JSON으로 감싸는 조건)도 같다. 뼈대의 "값은 줄 끝까지" 파서를 고친다 | 6.1.3 |
| 인박스 front matter | `subagent_level` → `parallel`. 인박스 전용 필드 열 개가 문법 오류의 기준이다(뼈대 그대로, 이름만 바뀐다) | 6.1.3 |
| `routes/<KEY>.json` | `"requires_packet": false` 추가 | 6.1.4 |
| `docs/evaluations/jev/<KEY>-find.json`, `-documents-find.json`, `-context.json`, `-packet.json`, `-packet-outcomes.json`, `history/` (새로) | 6.1.5절의 필드 단위 형식. `history/`는 커밋하지 않는다(템플릿 `.gitignore`에 줄 추가) | 6.1, 6.1.5 |
| `state.json` `runs[]` | `"coordinator": "<세션 핸들>\|null"` 추가 | 6.2.1 |
| `state.json` `dispatches[]` | `"request": "req_<16진수 16자>"`, `"sub_of": "<부모 dispatch ID>\|null"`, `"brief": "<sha256 앞 16자>\|null"` 추가. 예약 기록의 `id`는 `"pending:<request>"`. `terminal`이 `null`일 수 있다 | 6.2.1 |
| `state.json` `messages[]` | `delivery`가 보통 `null`. 넣는 종류가 "heartbeat·status가 아닌 모든 종류". `acked_at`을 쓰는 곳이 넷(M-10, `hold`, `landed_at`을 쓰는 곳, `wait --ack`) | 6.2.1 |
| `archive.jsonl` | `dispatches[]`의 원소에 `"sub": bool` 추가 | 6.2.1 |
| `turns.jsonl` | `kind`에 `subagent`·`workflow` 추가. 쓰는 주체에 T-13 | 6.2.2 |
| `status.json` | 두 갈래로 만든다(0.2절의 `update_state`). `dispatches[]`에 `"sub": bool`, `metrics`에 `at`·`subagent_calls`·`workflow_runs`·`sub_dispatches`. `tasks`는 state에 하위 배정이 아닌 dispatch나 completion이 있는 키이고 `stage`는 `stage_of(key, state, 빈 집합, true)`. `pending[].landing`은 `null`일 수 있다. `pending[]`에 `id`(completion ID)를 더하고 `landing`을 그 값으로 잇는다(6절) | 6.2.3 |
| 세션 파일 | `host` 값에 `grok`·`agy`. `terminal`은 세션 핸들(`ORCA_AGENT_SESSION_ID` → `ORCA_TERMINAL_HANDLE`). `"prompt_fp": "<sha256 앞 16자>\|null"` 추가. 모르는 필드를 보존한다(뼈대는 보존하지 않는다) | 6.3 |
| 추적 줄 | `actor`에 `+sub` 접미사. `host`에 `grok`·`agy` | 6.3 |
| `<git-dir>/fullops/search/{code,documents}.json`, `search/lock` (새로) | 탐색 색인 | 6.3 |
| `~/.fullops/deps.toml`, `deps-state.json`, `skill-backups/` (새로) | 의존 목록 사본, 설치 기록, 스킬 백업 | 6.4, 10.3 |
| `~/.fullops/package/<버전>/grok/`, `~/.fullops/package/<버전>/agy/` (새로) | `install`이 조립한 grok 패키지와 agy 패키지(앞 개정의 "통합 패키지의 사본"을 대신한다) | 6.4, 10.3 |
| `<GROK_HOME 또는 ~/.grok>/hooks/fullops.json` (새로, 호스트 쪽) | grok의 hook 등록 파일. `install`이 쓰고 `install --remove-hooks`가 지운다 | 6.4, 10.4 |
| `~/.fullops/install.json` | 필드는 그대로. `source`는 통합 패키지 폴더의 절대 경로 | 6.4 |
| `deps.toml` (새로, 패키지) | 10.7절의 형식. 파서는 `fullops-store::load_deps` | 10.7 |
| `scenarios/fixtures/host/agy/output/pass.pre_tool_use.json` | `{"decision":"allow"}` → `{"decision":"ask"}` | 7.2 |

### 0.4 규칙 ID (7.5절, 8절)

| ID | 바뀐 내용 | 절 |
|---|---|---|
| S-00, S-01 | `context_at_start`가 거짓인 호스트(grok)에서는 `Context` 대신 `Warn` 한 줄. S-01의 대상 `source`에 `fork` | 7.5 |
| S-02 | 세션 파일에 `host`를 쓴다 | 7.5 |
| S-04 | 조건의 비교 대상이 `install.json`의 `version`. 효과는 실행본 단계만 | 7.5, 10.3 |
| **S-06 (새로)** | SessionStart, 강도 `Context`. caveman 스킬 적용 안내 | 7.5 |
| P-01, P-03 | `Context`는 `context_at_prompt`가 참일 때만. P-03의 강도는 `Context`(뼈대의 `Warn`을 고친다) | 7.5 |
| P-02 | 분류 패턴 셋(앞 공백을 뗀 뒤 비교, `──── From:` 추가) | 7.5 |
| 프롬프트 이벤트가 없는 호스트 | 규칙 ID는 늘지 않는다. `decide`가 `prompt`의 지문이 바뀐 호출에서 P-00·P-01·P-02를 먼저 평가하고 `SetPromptFp`를 낸다 | 7.5 |
| T-00 | 통과하는 하위 명령에 `install`·`rules` 추가 | 7.5 |
| T-03, T-04 | 탈출구 문구. T-04의 "모름"에 `--spec-file` | 7.5 |
| T-08 | 조건을 병렬 방침으로 다시 정했다: (e) `off`, (f)·(g)는 `read`에서만. `on`·`ultra`에서는 걸리지 않는다. `escapes`가 경우 셋 | 7.5 |
| **T-13 (새로)** | PreToolUse, 강도 `Effect`. 대상 전부. `SubAgent`·`Workflow` 도구 호출을 `turns.jsonl`에 센다 | 7.5 |
| X-01 | 대상에서 하위 worker(`sub`)를 뺀다. `RULES`에는 `BlockOnce`로 넣고 dev 직접 작업은 `Finding::Hit.always`로 올린다(뼈대의 처리와 같다). 하위 작업 남음(`State::sub_open`)이 참이면 걸리지 않는다(6절) | 6.6, 7.5 |
| X-03 | 탈출구에 하위 worker의 길을 더했다. 하위 작업 남음이 참이면 Orca 조회 없이 걸리지 않는다(6절) | 6.6, 7.5 |
| X-05 | (b)의 조건이 "깨움 보장"(`orca_wakes`). 탈출구는 `ack_by`로 두 갈래. `background_wake`로 백그라운드·포그라운드를 가른다 | 7.5 |
| X-06 | 통합을 확인한 효과가 그 completion의 메시지에 `acked_at`도 쓴다 | 7.5 |
| F-01 | 본문이 `error`(종류)와 `error_details`의 조합. agy는 오류로 끝난 Stop에서 돈다 | 7.5 |
| **`route`의 R-01~R-05 → RT-01~RT-05** | `done`의 `R-`과 겹치지 않게 접두어를 바꿨다. `messages`의 구역 이름도 바꾼다 | 8.3 |
| D-06 | 예외: 구현 dispatch의 `terminal`이 `null`이면 터미널 비교를 하지 않는다 | 8.5 |
| D-08 | 조건의 "레벨"이 병렬 방침으로: `off` 거절, `read`는 Review·Research만, `on`·`ultra` 통과 | 8.5 |
| **D-12 (새로)** | `dispatch`의 패킷 검사. 패킷 조건이 참일 때만 | 8.5 |
| **D-13 (새로)** | 하위 배정의 조건 (a)~(d) | 8.5 |
| R-01 | 하위 worker를 거절한다 | 8.7 |
| R-08, R-09 | 묶인 dispatch가 없을 때의 둘째 길(후보에서 하위 배정을 뺀다. 6절). 좌석이 `Role`이면 본문을 낸다 | 8.7 |
| **R-10 (새로, `done`)** | `done`의 패킷 검사 | 8.7 |
| **R-11 (새로, `done`)** | 대상 dispatch(묶인 것, 없으면 R-08의 둘째 길)의 하위 작업 남음이 참이면 거절 | 8.7 |
| M-10, M-13 | 처리 표시의 범위, 출력 문구 | 8.8 |
| 리뷰 `check` 10번 (새로) | 패킷 검사 | 8.9 |
| **J-01~J-04, J-10~J-17 (새로)** | 탐색 명령의 거절과 `check_packet`의 항목 | 8.14 |

hook 규칙 표(`RULES`)는 34개가 된다: 뼈대의 32개에 S-06과 T-13을 더한다. `fullops rules --json`의 ID·이벤트·강도를 7.5절의 표와 대조하는 시험을 맞춘다. D·H·RT·R·M·J는 CLI의 거절 ID다.

### 0.5 CLI 표면 (7.1절, 8절, 10절)

| 명령 | 바뀐 내용 | 절 |
|---|---|---|
| `fullops hook` | `fullops hook <claude\|codex\|grok\|agy> [--event <이름>] [--plugin-root <경로>] [--in-place]`. 이벤트는 stdin의 `hook_event_name`, 없으면 `--event`. 통과의 글은 `Host::pass`(agy는 `decision`이 든 객체). 감시 스레드·해석 실패·panic 처리가 모두 그 글을 쓴다. 호스트 인자가 `claude`·`codex`인데 환경 변수 `GROK_HOOK_EVENT`가 있으면 `{}`만 낸다. 모르는 호스트 인자는 `{}`. 프로세스의 작업 디렉터리를 쓰지 않는다 | 7.1 |
| `fullops`(인자 없음) | stdout 빈 채로 종료 코드 0 | 7.1, 8.1 |
| 공통 규약 | 결과는 stdout, 거절·오류·알림은 stderr. `--json`의 네 형태(성공 `ok:true`, `refused`, `human`, `error{kind,message}`). 뼈대의 `{"ok":false,"error":"<글>"}`를 `error` 객체로 고친다. 거절이 여럿이면 `Refusal.exit`의 최댓값, 정책 오류는 2(뼈대 그대로). 활성 표식 예외와 정책 오류 예외 목록(뼈대 그대로) | 8.1 |
| `fullops dispatch` | `--task` **삭제**. `--brief "<글>"` 추가(하위 배정에서 필수, lead가 주면 사용법 오류). `DispatchRequest`에 `brief`와 호출자의 하위 여부가 들어간다 | 8.5, 8.15 |
| `fullops wait` | `--ack`의 값이 **메시지 ID**. `--no-wait` 추가. 반환 JSON에 `orca_wakes`, `actionable[].ack_by`·`from`·`subject` | 8.6 |
| `fullops done` | 인자는 그대로. R-01·R-08·R-09·R-10·R-11 | 8.7 |
| `fullops task recover` | 패킷 재생성과 `패킷 갱신` 커밋 | 8.4 |
| `fullops sync` | 표면은 그대로. 파일 `cmd/sync.rs`, 소유 묶음 **E**(뼈대는 C로 뒀다) | 8.8, 18.2 |
| `fullops status` | `--fetch`. 사람용 출력의 첫머리(역할, 인박스, 병렬 문구, lead 항목). `metrics`를 계산해 `status.json`에 넣는다 | 8.13 |
| `fullops find`, `context`, `packet build\|check` (새로) | 8.14절의 인자 | 8.14 |
| `fullops install` | `[--hosts …\|all] [--only runtime\|plugin\|deps]... [--dry-run] [--json]`, `--check`, `--prune`, `--rollback`, `--remove-hooks`. grok·agy는 패키지를 조립해 로컬 경로로 설치하고 grok은 hook 등록 파일을 쓴다 | 8.1, 10.3 |
| `fullops doctor` | 보는 항목에 호스트, 의존, Codex hook 신뢰, grok 폴더 신뢰, grok hook 등록, agy 패키지, 병렬 방침 | 10.6 |
| `fullops init` | `--subagent-level` → `--parallel off\|read\|on\|ultra`(기본 `on`). 관리 블록 대상에 `GEMINI.md` | 10.6 |
| `fullops migrate` | `subagent_level`을 `parallel`로 옮긴다. 관리 블록 대상에 `GEMINI.md` | 8.15, 13.2 |
| `cmd/mod.rs`의 명령 표 | `find`, `context`, `packet`을 더한다 | 8.2 |

### 0.6 워크스페이스, 파일 자리, 시험 도구 (10.1절, 12.1절, 18절)

| 대상 | 바뀐 내용 |
|---|---|
| `fullops-core` | 파일 `explore.rs`, `deps.rs` 추가. `messages.rs`를 `messages/` 폴더로 나눈다: `mod.rs`(공통 조립 함수, `MsgId`, `parallel_brief`. A)와 묶음별 빈 파일 `hook.rs`(B)·`flow.rs`(C)·`merge.rs`(E)·`lint.rs`(D)·`explore.rs`(J)·`install.rs`(F). `flow.rs`(core)는 뼈대 그대로 두고 `record_message`·`orca_effort`를 맞춘다. 쓸 수 있는 크레이트에 `serde_json`, `sha2` 추가 |
| `fullops-store` | 파일 `search.rs` 추가 |
| `fullops-ext` | 파일 `hosts.rs` 추가. `git.rs`와 `orca.rs`는 완성본이 되고 A의 파일로 남는다 |
| `fullops` | `hook/host_grok.rs`·`host_agy.rs`, `cmd/{find,context,packet}.rs`, `assets.rs`와 `assets/repo/`의 네 파일 추가. `assets/`에 `deps.toml` 내장 사본(F) |
| `plugin/` | `deps.toml`, `hosts/grok/`, `hosts/agy/` 추가(F). 배포 폴더는 통합 패키지와 디렉터리판 둘. `plugin/skills/fullops-motion/`은 K가 끝냈다 |
| `fullops-testkit` | `fake-host`(`claude`·`codex`·`grok`·`agy`·`npx` 이름), `fake-curl`, `ScriptJudge`, 임시 홈 빌더 추가. `fake-orca`는 소비형 `check`와 `--peek`의 차이, `--retry-request`를 흉내 낸다. `Setup.subagent_level` → `Setup.parallel`(기본 `on`). `Setup.host`가 `claude`·`codex`·`grok`·`agy`: 구동기가 `scenarios/fixtures/host/<호스트>/`의 입력 모양과 등록 인자(agy는 `--event`)로 hook을 부르고, 호스트마다 다른 출력 글을 `verdict` 어휘로 옮긴다. `[[step]]`의 `pending` 키(그 단계를 건너뛰고 "보류"로 적는다). 시나리오 파일의 자리는 `scenarios/<ID 소문자>.toml` |
| `docs/formats.md` | 이 절의 형식 변경을 옮겨 적고 † 표시와 10절 "열린 문제"를 0건으로 만든다 |
| `crates/fullops/src/main.rs` | `#![allow(dead_code)]`는 묶음이 합쳐진 뒤 지운다(지금은 둔다) |

### 0.7 바뀌지 않은 것

`Verdict`, `Intent`의 변형 목록, `Known`, `Refusal`, `Seat`, `Stage`·`Landing`, `TaskKey`·`RoleId`·`Sha`·`SessionId`의 문법, lint의 `result.json`·`pass.json`, 리뷰 폴더의 파일 형식, 종료 코드의 값(0~4), 잠금과 원자 교체의 방법, `discover`, `stage_of`·`ready`의 규칙(하위 배정을 건너뛰는 것만 더한다).

### 0.8 뼈대가 임시로 정했고 설계가 받아들인 것 (고칠 것 없음)

`messages::MsgId`. `WorkOutcome`. 파일 형식 타입을 `fullops_core::model`에 둔 것. `Rule.origin`의 값. `applies`로 못 나타내는 조건을 `check`가 보는 것. `Host` trait과 `DecodeError`를 `fullops/src/hook/mod.rs`에 둔 것. `seq`의 뜻. `state.json`의 `null` 허용 필드. hold의 필드 `{reason, owner, resume, at}`. 결정의 `dropped`·`repository`·`drop_reason`. 직접 작업 completion의 `id`(`direct:<sha 12자>`). Orca ID의 문법. `path_text`·`same_path`. `waiters` 파일 이름의 글자 바꾸기. `fullops.toml`의 필수 키와 오류 분류. 인박스 문법 오류의 기준. `anchor`의 `null`, 빈 세션 ID의 처리. CLI의 stderr·stdout 자리와 종료 코드 규칙, 활성 표식·정책 오류의 예외 목록, `--repo`·`--json`의 자리. `status.json` 교체 실패를 무시하는 것. `status.json`의 `version`·`mode`·`pending` 뜻. `read_plan(repo, template)`. store에 더한 함수들. ext의 타입 정의. `OrcaMessage`의 `subject`·`outcome`. 추적 줄의 어휘. `stage_of`의 시각 비교와 `key` 없는 completion. 시나리오의 키별 줄 형식, 동사 목록, `verdict` 어휘. `proc::run`의 기본 구현.


## 1. 절 번호 순 변경 목록

| 절 | 바뀌기 전 | 바뀐 뒤 | 이유 |
|---|---|---|---|
| 머리말 | 2026-11-13 작성 | 개정 일자와 근거(계약 문서, 더 읽은 원본 스크립트)를 더했다 | 개정 |
| 1 요지 2·3번 | 대기에 관한 말이 없다. hook 실행 방식에 관한 말이 없다. "도구 호출당 프로세스는 최대 1개" | 조율자를 깨우는 일은 Orca에 맡긴다. Claude Code hook은 셸 없이 직접 실행한다. 세는 것은 `fullops` 프로세스이고 호스트가 끼우는 셸은 세지 않는다 | Z1, M1 |
| 1 요지 7번 | 빼는 것에 탐색, caveman 주입, grok·agy가 있다 | 셋을 뺐다. `lint_tune`, OCR delegate 스킬, `session_cache`를 적었다 | 레포 주인의 결정 |
| 1 요지 8~10번 | 없음 | 네 호스트, 의존 목록 `deps.toml`, Jev 탐색 | 레포 주인의 결정 |
| 2.1 G4 | 실행 중 Python·Node가 필요 없다 | hook과 흐름 명령은 쓰지 않는다. `install`의 스킬 단계가 부르는 `npx`만 예외 | 의존 스킬 설치 |
| 2.3 | - | 호스트 설정(폴더 신뢰, hook 신뢰, PATH)을 대신 바꾸지 않는다 | 원본의 원칙 유지 |
| 3 (패킷 행) | v1에는 패킷이 없다 | 패킷 조건이 참일 때만 D-12·R-10·리뷰 check 10번. 옮겨 온 `requires_packet`만 예외 | 탐색을 v1에 |
| 4 | - | 호스트, 세션 핸들, 통합 패키지, 의존 목록, 탐색 결과, 탐색 패킷, 패킷 조건 | 새 개념 |
| 5 | - | `HostId`, `Route.requires_packet`, `PacketCategory`, `PacketCheck`, `Packet`, `PacketOutcomes`, `Deps`. 불변식 3번(ack 대신 소비형 check 전에 저장), 6번 보강, 7번(탐색은 추천) | Z1, 탐색, 설치 |
| 6.1 | - | 탐색 결과·패킷·처리 기록·history 행 | 탐색 |
| 6.1.4 | `requires_packet` 없음 | 필드와 옮기는 규칙 | 유지할 결정 |
| 6.1.5 | 없음 | 탐색 결과와 패킷의 형식, 지시서 지문 | 탐색 |
| 6.2.1 | `runs[]`에 `coordinator` 없음. `dispatches[]`에 `request` 없음. `delivery`는 항상 값 | 세 가지를 고쳤다. 예약 기록의 뜻. `acked_at`의 뜻 | Z1, Z3, X-05 |
| 6.3 | - | 탐색 색인. 세션 파일의 `host`·`terminal` 정의 | 탐색, 네 호스트 |
| 6.4 | - | `deps.toml`, `deps-state.json`, `skill-backups/`. Windows에서 `$HOME`을 쓰는 문자열을 두지 않는다 | 설치, M1 |
| 6.5 | - | 탐색 색인의 잠금 | 탐색 |
| 6.7 | - | 사람 턴이 부풀 수 있는 경우 | M5 |
| 7.1 | `fullops hook <claude\|codex>` | `<claude\|codex\|grok>`. 모르는 호스트·인자 없는 실행은 통과. 플러그인 루트 추론 | 네 호스트, G2 |
| 7.2 | 두 호스트 표. `stop_reason`·`error` 주석. snake_case만 | 네 호스트 표, `host_for`, grok의 camelCase, hook 없는 호스트의 한계 표, 출력을 누가 읽는가 표 | Z4, Z5, 네 호스트 |
| 7.3 2번 | `ORCA_TERMINAL_HANDLE` | 세션 핸들. 없거나 worker에 터미널이 없으면 건너뛴다 | M3 |
| 7.4 | `apply_patch` 문법(미정-M2) | 봉투·`Environment ID`·`End of File`·`@@ <문맥>`. 경로 맞추기. `--spec-file`. grok `search_replace` | M2, 네 호스트 |
| 7.5 | 0.4절 참고 | 0.4절 참고 | - |
| 7.6 | `stop_hook_active`를 Codex가 주는지 모른다 | 둘 다 준다. 그래도 턴 기준. `Warn`은 사용자만 본다. Claude Code의 여덟 번 상한 | M2 |
| 7.7 | 분량 한도 없음 | 9,000자와 줄이는 순서. lead의 대기 안내. pstack 우선 문장 | M2, 의존 설치 |
| 7.8 | - | 모르는 호스트 인자, hook 프로세스를 띄우지 못한 경우, 알림이 누구에게 가는가, 탐색 파일 파손 | G2 |
| 8.1 | - | 인자 없는 실행, hook 없는 호스트의 호출자 판정, CLI의 플러그인 루트 규칙 | 네 호스트 |
| 8.2 | - | `find`·`context`·`packet` 행. `install`·`doctor` 행을 갈랐다 | 탐색, 설치 |
| 8.3 | 9단계 | `requires_packet`을 정하는 9단계를 넣고 쓰기는 10단계 | 유지할 결정 |
| 8.4 | `task recover`가 패킷을 다시 만들지 않는다 | 다시 만든다(`rebuild_packet`). WIP 경로를 seed에 더하고 패킷 갱신 커밋을 만든다 | 탐색 |
| 8.5 | `--task`, `--spec "<글>"`, `--worktree <경로>`, 인자를 한 줄에 다 붙임, `pending:<run>:<started_at>` | `--task` 삭제. `--spec-file -`, `--worktree path:<경로>`, 새 터미널과 있는 터미널의 인자 조합, 예약 기록과 `--retry-request`, 결과 처리 표, `run-create --objective`, D-12, grok 신뢰 알림, 준비 커밋에 탐색 파일 | Z2, Z3, 탐색 |
| 8.6 | "ack하지 않고 반환 → `--ack <delivery>`" | 미리 보기 → 기록 → 소비형 check. `--ack <메시지 ID>`는 `state.json`만 쓴다. `--no-wait`. `run-use`로 다시 묶기와 깨움 판정(Orca가 깨우는 세션에서는 기다리지 않는다). 종류별 처리에 그 밖의 종류. 반환 JSON(`ack_by`, `next`) | Z1 |
| 8.7 | R-08·R-09가 묶인 dispatch를 요구한다 | R-10. 과제 키를 정하는 순서. R-08의 둘째 길과 R-09의 본문 출력(hook 없는 호스트의 worker) | 탐색, 네 호스트 |
| 8.8 | M-13 "다음: `fullops wait … --ack <delivery>`" | M-10이 처리 표시를 한다(같은 과제의 `worker_done` 전부). `hold`와 `settle`도 한다. M-13 문구 | Z1 |
| 8.9 | check 9번까지 | 10번(패킷). cleanup의 터미널 없는 worker | 탐색, M3 |
| 8.11 | `lint.json`이 없을 때의 기본값이 적혀 있지 않다 | 내장 템플릿의 `lint.json`. 제외 목록은 `lint_excludes` 하나가 만든다 | 탐색 색인과 함께 쓴다 |
| 8.14 | 없음 | 탐색 명령 전체 | 탐색 |
| 9.1 | - | 탐색이 쓰는 `ls-tree`, `grep` | 탐색 |
| 9.2 | 최소 1.4.246. 한 가지 형태. ack. `stage == "settled"` | 최소 1.4.256. 세 형태. 명령 표. 응답 해석 표. 터미널 없는 worker. `orca.exe` | M3 |
| 9.3 | `route`만 부른다 | `route`·`find`·`context`. 호출 수 상한, 검증 규칙, 오프라인 동작 | 탐색 |
| 9.5 | 없음 | 호스트 CLI와 스킬 설치 도구의 명령 표 | 설치 |
| 10.1 | - | 0.6절 참고 | - |
| 10.2 | - | 0.2절 참고 | - |
| 10.3 | Claude Code와 Codex가 고정 위치를 실행한다. `install [--prune] [--rollback]` | 호스트별 실행본 표. `install`의 세 단계와 네 호스트, `--check`, 오프라인, 설치 기록, 스킬 백업 | M1, 설치 |
| 10.4 | 패키지 셋, POSIX 셸 문자열 셋 | 통합 패키지와 디렉터리판. Claude Code는 exec form과 `onFailure: "continue"`. Codex는 `command`와 `commandWindows`. grok의 세 갈래. agy | M1, 네 호스트 |
| 10.5 | `dist/claude-marketplace`·`claude-directory`·`codex` | `dist/native`·`dist/directory`. 매니페스트와 MCP 파일을 `deps.toml`에서 만든다. 호스트 CLI의 `plugin validate` | 네 호스트 |
| 10.6 | `init`이 AGENTS·CLAUDE에 블록. `doctor` 한 문장 | `GEMINI.md` 추가. `doctor` 항목 표 | 네 호스트, 설치 |
| 10.7 | 없음 | `deps.toml`의 형식과 확정 목록 | 설치 |
| 11.1 | caveman 주입 버림 | S-06. 패킷 검사 행, grok 입력 행 | 레포 주인의 결정 |
| 11.2 | `caveman_start` 버림. `grok_trust` 버림. 탐색 스크립트 나중. `deps` 대체(설치하지 않는다) | S-06. `--check`는 v1, `--add`는 버림. 탐색은 v1. `deps`는 v1. 통합 설치기·`adapters/` 행 | 레포 주인의 결정 |
| 11.3 | GEMINI.md 제외 | 포함. 설치 대상 호스트 행, pstack 주의 행 | 네 호스트 |
| 11.4 | "v1에 패킷이 없다". grok camelCase는 잔재 | 패킷은 선택. 실험 기능 행. 신뢰 자동 변경 금지 행. grok camelCase는 살아 있는 계약 | 탐색, 네 호스트 |
| 12.1 | - | `fake-host`, `fake-curl`, 임시 홈 | 설치, 탐색 |
| 12.2 | I2~I5 | I2~I5를 고치고 H5, I6·I7, M2, O1~O3, P1~P13, Q1~Q9를 더했다 | 전부 |
| 12.3 | DF-12(caveman 없음) | DF-12를 고치고 DF-14~DF-21을 더했다 | 전부 |
| 12.4 | 실환경 점검 한 문단 | 13개 항목의 표 | 미확인 항목 |
| 12.5 | - | 탐색과 설치는 시나리오로 본다 | 검증 방식 |
| 13.2 | 옛 `requires_packet`은 사라진다. 블록은 AGENTS·CLAUDE | 옮겨진다. 탐색 기록의 처리. `GEMINI.md`. 호스트별 `fullops install` | 유지할 결정 |
| 13.4 5번 | 새 플러그인을 설치 | `fullops install`. grok·agy로 역할을 돌리는 레포는 0b 뒤에 전환 | 네 호스트 |
| 14 | - | 패널의 Node worker, 런처의 쓰임, 스킬 설치의 `npx`, 호스트 CLI | M4, 설치 |
| 15 | C3·C4의 미정 문구 | 계약 확인 결과로 고쳤다 | M1, M2 |
| 16 | M1~M9 | M1~M5를 "남은 것"으로 줄였다. M10(grok), M11(agy), M12(호스트 CLI 출력)를 더했다. 확정돼 내린 것을 적었다 | 계약 확인 |
| 17 | H2·H4·H6·H7이 기본안 | H2·H4·H6·H7 확정. H6에서 grok·agy·caveman을, H7에서 탐색을 뺐다. H3에 마켓플레이스 출처. H10(의존 목록, 확정), H11(grok·agy의 hook 범위, 열림) | 레포 주인의 결정 |
| 18.1 | - | 뼈대 산출에 새 타입·시그니처·가짜 실행 파일. 묶음 0은 끝남 | 전부 |
| 18.2 | 0, A~H | 0b(grok·agy 계약 확인)와 J(탐색)를 더했다. B·C·D·E·F·G·H의 범위와 끝났다는 확인을 고쳤다. `sync`는 C에서 E로 옮겼다. 묶음 사이에 부르는 함수는 10.2절 끝의 표가 전부다 | 전부 |
| 18.3 | 탐색이 나중 묶음 | 뺐다. 0b의 순서를 넣었다 | 레포 주인의 결정 |

## 2. "결정 필요"를 정한 것

`docs/contracts/design-delta.md`가 "결정 필요"로 남긴 것과 이 개정이 정한 답이다.

| 결정 필요 | 정한 것 | 이유 |
|---|---|---|
| Claude Code hook 문자열: shell form 유지(가)와 exec form(나) | exec form. `onFailure: "continue"`. Claude Code는 고정 위치가 아니라 플러그인 폴더의 실행본을 직접 실행한다 | Git Bash가 없는 Windows에서도 돌고 도구 호출을 막지 않는다. 셸 프로세스가 하나 줄어든다. 고정 위치의 이유는 Claude Code가 이전 버전 폴더를 14일 남겨서 없어졌다 |
| Codex 등록 파일 이름과 문자열 | `hooks/codex.json`을 두고 `.codex-plugin/plugin.json`에 `hooks`를 적는다. `command`(sh로 감싼 폴백)와 `commandWindows` | 통합 패키지에서 Claude Code의 `hooks/hooks.json`과 함께 둔다 |
| S-01을 `fork`에도 낼지 | 낸다 | 갈라진 새 세션이다 |
| Orca 최소 버전 | 1.4.256 | 계약을 확인한 버전이다 |
| `dispatch --task` | 뺀다 | Orca가 Task와 spec을 함께 받지 않는다 |
| 후속 지시를 `send --to dispatch:<id>`로 바꿀지 | T-03의 탈출구를 그 명령으로 안내한다. fullops가 감싸지 않는다. `dispatch --terminal`은 2분 창 안의 재사용용으로 남긴다 | Orca가 이미 하는 일을 겹치지 않는다 |
| `wait`와 Orca 자체 알림 가운데 무엇을 쓸지 | Orca가 깨울 수 있는 세션(Run의 `coordinator`)은 Orca에 맡기고 X-05 (b)를 통과시킨다. 그 밖의 세션만 `fullops wait`를 건다 | 폴링을 겹치면 스크립트의 소비형 check가 조율자의 메일을 가져간다 |
| ack가 없는 Orca에서 `wait --ack` | 값은 메시지 ID, 쓰는 곳은 `state.json`뿐. Orca의 읽음 표시는 미리 보기를 기록한 뒤의 소비형 check가 한다 | Orca에 없는 명령에 기대지 않는다. 기록이 먼저다 |
| `HookInput.stop_reason`을 지울지 | 남긴다 | grok이 세션 종료 Stop에 `reason: "shutdown"`을 준다(1.24.0의 사실) |
| "no longer bound" 오류 | 문구와 코드에 기대지 않는다. 실패하면 `run-use` 뒤 한 번 다시 | 코드 값이 미확인이다 |
| Orca 패널 | `trusted-worker` 플러그인. Node worker가 `status.json`을 읽는다 | 패널만으로는 파일을 읽지 못한다 |

## 3. 남은 미정과 사람 결정

- 미정(16절): M1~M5의 남은 것은 실환경 점검(12.4절의 표)으로 간다. M6·M7·M9는 그대로다. M8에 claude.ai·Cowork 제약을 더했다. M10(grok 계약), M11(agy 계약), M12(호스트 CLI의 출력과 스킬 설치 도구의 인자)는 새 항목이다. M10·M11은 묶음 0b가 맡는다.
- 사람 결정(17절): 확정은 H2, H4, H6, H7, H10이다. 열린 것은 H1, H3, H5, H8, H9, H11이다. H11(grok·agy에서 hook이 어디까지 도는가)은 0b의 결과를 보고 확정한다.

## 4. 점검 지적 반영 (2026-11-14)

개정본을 다른 모델이 점검해 낸 지적 19건이다. 문서와 근거(1.24.0의 `jev_packet.py`·`work.py`·`deps.py`·`caveman_start.py`·`lint.py`·`bin/fullops-squad.cjs`·`docs/development.md`, `docs/contracts/orca.md`)로 확인했다. 19건 모두 맞는 지적이어서 고쳤다. 두 건은 지적의 일부 서술이 사실과 달라 그 부분을 적었다.

| # | 지적 | 판정 | 고친 것 | 절 |
|---|---|---|---|---|
| 1 | 지시서가 패킷 항목이 될 때의 1.24.0 특례(필수, 지시서 지문)가 빠져 `--write-inbox` 뒤 배정이 J-12로 거절된다 | 맞다. 다만 "context를 돌린 과제는 모두"는 아니다: 개정본에서 지시서가 항목이 되는 것은 지시서가 `HEAD`에 커밋돼 지도에 든 경우(`task recover`의 재생성 등)였다. 어느 쪽이든 거절이 생긴다 | 지시서 파일을 항상 필수 항목으로 둔다. `source_sha256`은 지시서만 지시서 지문이고 만들 때와 J-12가 같은 함수(`explore::source_sha256`)를 쓴다. `check_packet`의 마지막 인자를 글을 돌려주는 `read_text`로 바꿨다. P2·P3에 `--write-inbox` 뒤 배정 통과를 더했다 | 6.1.5, 8.14, 10.2, 12.2 |
| 2 | 리뷰·조사 보고와 실패 보고의 메시지를 `merge`·`hold`가 처리 표시하지 못하는데 문구는 `merge`를 안내한다 | 맞다 | 메시지마다 `ack_by`(`merge`·`ack`)를 계산해 X-05 (a)의 문구와 `wait`의 `next`를 두 갈래로 낸다. M-10이 같은 과제의 `worker_done` 메시지 전부를 표시한다. 통합 확인(`landed_at` 기록)도 표시한다(날것 `git merge`로 통합한 보고가 같은 구멍에 빠지는 것을 함께 막았다). I4에 리뷰 보고·실패 보고·날것 병합을 더했다 | 6.2.1, 7.5, 8.6, 8.8, 12.2 |
| 3 | 패킷의 `input_sha256`에 원문과 producer의 해시가 없고, 입력이 다를 때의 동작이 없다 | 맞다 | 패킷 `options`에 `sources_sha256`·`producer_sha256`, find·context `options`에 `provider`·`cache_bypass`. "재사용과 덮어쓰기" 표와 J-04(입력이 다르고 `--force`가 없으면 거절). P13 | 6.1.5, 8.14, 12.2 |
| 4 | 지시서 지문 (4)단계가 두 갈래로 읽힌다 | 맞다 | "줄 앞 공백 0개 이상 + `-`·`*` + 공백 1개 이상 뒤의 `[x]`·`[X]`만 `[ ]`로 바꾸고 앞부분은 그대로"로 적었다 | 6.1.5 |
| 5 | 규칙이 호스트를 알 길이 없다 | 맞다 | `Facts::host() -> HostId`. Q8에 `hook claude`로 들어온 grok 입력을 더했다 | 7.7, 10.2, 12.2 |
| 6 | `wait`가 Run을 다시 묶은 뒤에도 기다려 Orca의 깨움과 겹친다 | 맞다 | 2단계에 깨움 판정을 넣었다: `orca_wakes`가 참이면 `--no-wait`와 같게 동작한다. X-05·`wait`·`dispatch`가 `orca_wakes` 하나를 쓴다. X-05 (b)의 문구, I5, DF-21 | 7.5, 8.6, 10.2, 12.2, 12.3, 16 |
| 7 | `install_plan`이 호스트 하나의 계획만 돌려줘 호스트를 가로지르는 스킬 명령을 만들 수 없다 | 맞다 | `install_plan(deps, hosts, registered, package_dir) -> Plan`. 스킬 묶음의 대상 호스트 규칙과 실패 처리 네 가지. Q2 | 10.2, 10.3, 12.2 |
| 8 | agy에 넘길 폴더와 CLI의 플러그인 루트가 정해지지 않았다 | 맞다 | `plugin` 단계가 통합 패키지를 `~/.fullops/package/<버전>/`에 복사해 그 경로를 넘긴다. CLI의 플러그인 루트 규칙(실행 파일 위치 → `install.json`의 `source` → 없음). 미정-M11에 "복사하는가 참조하는가"를 더했다 | 6.4, 8.1, 9.5, 10.3, 16 |
| 9 | hook이 없는 호스트의 worker는 `done`이 본문을 내지 않아 완료 경로가 끊긴다 | 맞다 | R-09를 "좌석이 `Role`이면 출력"으로, R-08에 "이 워크트리의 그 과제 dispatch가 하나면 그것에 `reported`"를 더했다. 인자는 늘리지 않았다. H5 | 7.2, 8.7, 12.2 |
| 10 | `task recover`의 패킷 재생성이 J의 코드에 기대는데 선언이 없고, `sync`의 파일과 소유가 흐리다 | 맞다 | `rebuild_packet`(J 구현, C 호출, 뼈대 임시 구현). `sync`는 E가 맡고 파일은 `cmd/sync.rs`다. 묶음 사이에 부르는 함수를 10.2절 끝의 표 하나로 모았다 | 8.4, 10.1, 10.2, 18.2 |
| 11 | J가 기대는 산출물 인덱스 파서, lint 제외 읽기, 바이트에서 패킷을 푸는 함수의 시그니처가 없다 | 맞다 | `parse_deliverable_index`, `lint_excludes`(D), `parse_packet`·`parse_packet_outcomes`(A). `lint.json`이 없을 때의 기본값을 "내장 템플릿의 `lint.json`"으로 적었다(1.24.0과 같다) | 8.9, 8.11, 8.14, 10.2, 18.2 |
| 12 | 이 문서 0.1·0.3에 새 타입과 새 자리가 빠졌다 | 맞다 | 0.1에 `Plan`·`DepsState`·`DepsError`·`TreeEntry`·`IndexEntry`·`Scope`·`Signal`·`Decision`·`PacketInput` 등, 0.2에 `Facts::host`, 0.3에 `history/`와 `.gitignore` 줄 | 이 문서 0절 |
| 13 | find·context·packet 파일에 뜻이 없는 필드가 있다 | 맞다 | `map_sha256`, `found_probability`, `unreadable_files`·`remaining_candidates`의 원소, `candidates[].id`, `options.sources`, `worktree_dirty`, `optional_context_paths`, `unknown[]`의 필드와 `reason` 값에 한 줄씩 | 6.1.5 |
| 14 | `task recover`가 WIP 경로를 seed에 더하지 않고, 다시 만든 파일을 누가 커밋하는지 없다 | 맞다 | WIP 커밋이 바꾼 경로를 seed에 더한다. `recover`가 `fullops: 패킷 갱신 <KEY> (<R>)` 커밋을 만든다. P12 | 8.4, 12.2 |
| 15 | `grok plugin marketplace update`에 이름이 없다 | 맞다(통합 설치기 174행) | `grok plugin marketplace update <이름>` | 9.5 |
| 16 | `Deps` 타입과 `deps.toml` 파서의 자리가 둘이다 | 맞다 | `Deps`는 `model`(A), `deps.rs`는 `HOST_COMMANDS`·`install_plan`(F), 파서는 `fullops-store::load_deps`(A) | 10.1, 18.1 |
| 17 | S-06과 9.5절이 스킬 폴더를 다르게 정의한다 | 맞다 | 목록의 정의는 `skill_roots` 하나다. S-06은 `~/.agents/skills`를 먼저 보고 그다음 `skill_roots(host)`를 본다. `claude`에서 `~/.agents/skills`를 보는 이유(본문을 가져오는 것이고 1.24.0이 그렇게 찾았다)를 적었다 | 7.5, 10.2 |
| 18 | "도구 호출당 프로세스 최대 1개"가 Codex의 `cmd.exe`와 어긋난다 | 맞다 | 세는 것은 `fullops` 프로세스이고 호스트가 끼우는 셸은 세지 않는다고 요지와 bench에 적었다 | 1, 12.4 |
| 19 | `doctor`의 Codex hook 신뢰 조건을 계산할 수 없다 | 맞다 | "`deps-state.json`에 `hosts.codex`가 있는데 `host`가 `codex`인 세션 파일이 하나도 없다" | 10.6 |

이 반영으로 뼈대가 더 바꿔야 하는 것은 0절에 합쳐 적었다. 추린 목록: `Facts::host`, `check_packet`의 마지막 인자(`read_text`), `install_plan`의 시그니처와 `Plan`·`Step`, 새 함수 `source_sha256`·`packet_source_paths`·`parse_packet`·`parse_packet_outcomes`·`orca_wakes`·`ack_by`·`rebuild_packet`·`lint_excludes`·`parse_deliverable_index`, 10.2절이 필드까지 적은 새 타입, 탐색 결과 `options`의 새 필드, `wait` 반환의 `ack_by`, 규칙 ID J-04, R-08·R-09·M-10·X-05·X-06의 문구와 효과, `cmd/sync.rs`, `~/.fullops/package/`.

## 5. 마무리 개정 (2026-11-14)

입력은 넷이다. (가) grok·agy 계약 확인 결과 `docs/contracts/hosts-delta.md`(근거 `grok.md`·`agy.md`와 fixture). (나) 뼈대 담당이 보고한 설계 빈틈 48건. (다) 모션그래픽 스킬 `fullops-motion`의 포함(이미 `plugin/skills/fullops-motion/`에 있다). (라) 병렬 진행: 레포 주인의 새 요구(2026-11-14) "rust 버전에서는 서브에이전트를 적극 활용해서 병렬작업으로 개발을 빠르게 진행하는 것도 포인트다". 뼈대가 따라 바꿀 것은 0절에 합쳐 적었다.

### 5.1 절 번호 순 변경 목록

| 절 | 바뀌기 전 | 바뀐 뒤 | 입력 |
|---|---|---|---|
| 머리말 | 계약 근거가 Claude Code·Codex·Orca | Grok Build 1.0.70, agy 1.4.8, 울트라코드의 공식 문서를 더했다 | 가, 라 |
| 1 요지 1·3·8번 | 통과는 `{}`. StopFailure는 Claude Code만. agy는 스킬과 안내 문서만 | 통과의 글은 호스트와 이벤트가 정한다. StopFailure는 Claude Code와 grok. 네 호스트 모두 hook이 돈다 | 가 |
| 1 요지 7번, 11·12번 | 빼는 것에 모션 도구 | 모션을 뺐다. 11번 병렬 진행, 12번 모션그래픽 스킬 | 다, 라 |
| 2.1 G3·G4 | 쓰기 예외 하나. 배포본 `.mjs` 0개 | agy의 세션 파일 쓰기 예외. `render.mjs` 하나는 예외 | 가, 다 |
| 2.3 | 호스트 설정을 바꾸지 않는다 | grok의 fullops 전용 hook 등록 파일 하나를 쓰고 보여 준다 | 가 |
| 3 | 리뷰·테스트 최소·표적 | 그 결정은 리뷰와 테스트에 관한 것이고 병렬 서브에이전트는 별개로 권장한다고 적었다 | 라 |
| 4 | - | 통합 패키지의 뜻, 병렬 방침, 호스트 서브에이전트, 울트라코드, 하위 배정 | 가, 라 |
| 5 | `SubagentLevel`. `Actor` 셋. `Caps` 없음 | `Parallel`, `Actor.sub`, `Caps`·`caps_of`·`host_of_agent`, Orca ID 문법, 이름과 자리(`WorkOutcome`, `MsgId`, 형식 타입은 `model`), 불변식 8번 | 나, 라 |
| 6.1.1 | `subagent_level`. 필수 키 미정 | `parallel`. 필수 키와 검증 규칙, 오류 분류, 기준 시점 설정을 못 읽을 때(`baseline`) | 나, 라 |
| 6.1.2 | 담당 칸 대소문자 그대로 | 소문자로 읽는다. 열 우선순위, 코드 펜스, `\|`, 괄호, 토큰 끝 글자 | 나 |
| 6.1.3 | "값은 줄 끝까지" | 1.24.0의 `deliverables.py`와 같은 읽기·쓰기 문법(`FmValue`). 인박스 문법 오류의 기준. `parallel` 필드 | 나, 라 |
| 6.2.1 | - | `sub_of`·`brief`, `seq`, `null` 허용 필드, hold의 형태, 결정의 값, 직접 작업 `id`, 경로 표기, archive의 `sub`와 `landed_at` | 나, 라 |
| 6.2.2 | `human`과 `machine:*` | `subagent`·`workflow` | 라 |
| 6.2.3 | 쓸 때마다 전부 다시 만든다 | 두 갈래: state에서 나오는 것은 매번, 유도값은 넘겨받은 것만. 잠금 안에서 자라는 파일을 읽지 않는다 | 나 |
| 6.3 | `host` 셋 | `agy`, `prompt_fp`, 모르는 필드 보존, `anchor`의 `null`, 추적 줄의 어휘 | 가, 나 |
| 6.4 | 통합 패키지의 사본 | 조립한 grok·agy 패키지. grok의 hook 등록 파일 | 가 |
| 6.6, 6.7, 6.8 | - | `stage_of`의 하위 배정·시각 비교, `ready`는 lead 문맥, 병렬 사용 지표, 하위 dispatch의 뒷정리 | 나, 라 |
| 7.1 | `hook <claude\|codex\|grok>`. 통과는 `{}` | `hook <claude\|codex\|grok\|agy> [--event]`. 통과의 글. `GROK_HOOK_EVENT` 겹침 막기. 작업 디렉터리를 쓰지 않는다 | 가 |
| 7.2 | 어댑터 셋, `host_for(arg, raw)`, agy는 hook 없음 | 어댑터 넷, `host_for(arg)`, `Host::pass`, `ToolCall` 필드, 입력 대응·도구 대응·능력·출력 표, 닿지 않는 곳과 대신하는 것 | 가, 나 |
| 7.3 | dispatch가 묶이면 좌석은 4~6 | 3~6(리뷰어가 `Reviewer`). `sub` | 나, 라 |
| 7.4 | 호스트별 도구 이름으로 계산 | `ToolCall`만 본다. `truncated`. 규칙 후보에 `Workflow`와 agy의 입력 변화 | 가 |
| 7.5 | `Rule.escape`, `Finding::Hit { fp, args }` | `escapes`·`case`·`always`. S·P 규칙의 능력 조건. 프롬프트 이벤트가 없는 호스트. T-08 재정의, T-13. X-01의 하위 worker 제외. StopFailure의 대상 | 가, 나, 라 |
| 7.6, 7.7, 7.8 | - | `user_warn`이 없는 호스트의 처리, agy의 턴, 병렬 문구와 하위 worker 문장, 통과의 글, CLI 예외 목록 | 가, 나, 라 |
| 8.1 | 종료 코드와 거절 출력 | 출력 자리, `--json`의 네 형태, 여러 거절의 종료 코드, 예외 목록, 옵션의 자리 | 나 |
| 8.3 | R-01~R-05 | RT-01~RT-05 | 나 |
| 8.5 | D-08은 레벨. spec에 `Subagent level` | `--brief`, D-08은 병렬 방침, D-13, 하위 배정의 단계, `orca_effort`, spec의 `Parallel`과 병렬 문장·끝 문장 | 라 |
| 8.6, 8.7 | - | 하위 배정은 completion을 만들지 않는다. R-01의 하위 worker, R-11 | 라 |
| 8.13 | `status`는 요약 | 첫머리에 역할·인박스·병렬 문구·lead 항목. `metrics` 계산. `--fetch` | 가, 나, 라 |
| 8.15 | 없음 | 병렬 진행: 방침, 수단과 기준, 안내가 닿는 자리와 문구, 울트라코드, 하위 배정, 게이트와의 관계, 한계 | 라 |
| 9.2, 9.5 | - | 쓰지 않는 Orca 인자. 호스트 CLI에 `--help`를 붙이지 않는다. grok·agy의 설치 명령, hook 등록 행 | 가, 라 |
| 10.1 | - | `model/` 하위 모듈, `flow.rs`, `messages/` 폴더, `host_agy.rs`, `cli.rs`, `assets.rs`와 네 자산 파일, `plugin/hosts/` | 가, 나 |
| 10.2 | - | `update_state(…, derived, …)`, `read_plan(repo, template)`, `Git`의 `name_status`·`diff_u0`와 그 밖의 시그니처, `parallel_brief`·`orca_effort`·`baseline`·`resolve_actor`, `StepKind`의 `Assemble`·`HookFile`, 묶음 사이 함수 표에 `GitCli`·`OrcaCli`·`proc::run`·`assets`·`JevCurl`·`metrics` | 가, 나, 라 |
| 10.3 | grok은 미정. agy에 통합 패키지의 사본을 넘긴다 | 호스트별 실행본 표에 grok·agy. 조립하는 패키지. grok의 hook 등록 파일과 `--remove-hooks`. `--check` (5) | 가 |
| 10.4 | 통합 패키지를 네 호스트가 읽는다. grok은 세 갈래 가정, agy는 hook 없음 | 통합 패키지는 Claude Code·Codex용이고 `hosts/`에 grok·agy의 재료를 싣는다. grok과 agy의 등록 행. 스킬 폴더를 통째로 복사. `plugin/skills` 자산의 lint·리뷰 취급 | 가, 다 |
| 10.5, 10.6, 10.7 | - | 네 매니페스트, hook 등록 파일 셋, MCP 파일 셋, G4 검사, 조립 패키지의 `validate`. `init --parallel`, 관리 블록의 내용, `doctor`의 세 행. `[self]`의 주석 | 가, 다, 라 |
| 11.1~11.4 | grok 입력(camelCase) 행. 모션 버림. 하위 위임 레벨 | grok·agy의 기준선과 새 처리. `render.mjs`와 `fullops-motion`은 v1. 병렬 방침으로의 컨셉 변경. 과제명 파일 병렬 배정을 되살리지 않았다는 확인 | 가, 다, 라 |
| 12.1 | TOML이 아닌 예시 | 유효한 TOML, 형식(`host`·`parallel`·`pending`), 파일 자리와 주인 | 나 |
| 12.2 | D7, O2·O3, Q1·Q2·Q8·Q9 | 고쳤다. O4·O5(agy), Q10, R1~R8(병렬)을 더했다 | 가, 다, 라 |
| 12.3 | DF-21까지 | DF-22(권장으로), DF-23(하위 배정과 R-11), DF-24(grok·agy) | 가, 라 |
| 12.4, 12.5 | 실환경 점검 13개 | 15개(12 grok, 13 agy, 14 울트라코드와 하위 배정). bench의 자라는 파일. front matter 사례 | 가, 나, 라 |
| 13.2, 13.4 | 누락 값은 `off` | `subagent_level`을 `parallel`로 옮긴다. archive의 `landed_at`. `rules/delegation.md`. grok·agy 레포의 전환 조건 | 나, 라 |
| 14 | 모션 렌더는 싣지 않는다 | `render.mjs` 행, 호스트 서브에이전트 행 | 다, 라 |
| 16 | M10·M11은 계약 전체 | 남은 것만으로 좁혔다. M13(울트라코드와 하위 배정). 0b로 확정돼 내린 것 | 가, 라 |
| 17 | H11 열림 | H11 확정. H12(병렬의 기본값과 이행), H13(grok의 hook 등록 파일). H3·H5·H6을 맞췄다 | 가, 다, 라 |
| 18 | A 뒤에 B~J, F는 스킬과 grok·agy 부분을 미룬다 | 동결 뒤 여덟 묶음이 동시에 시작한다. 파일 주인, 시나리오 주인, `pending`, A가 완성본으로 두는 것, 병렬 진행의 구현 몫 | 나, 라 |

### 5.2 `hosts-delta.md`의 N1~N8과 D1~D10

| # | 내용 | 정한 것 | 절 |
|---|---|---|---|
| N1 | agy의 입력에 이벤트 이름이 없다 | `--event` 인자. `Host::decode(raw, event_arg)` | 7.1, 7.2 |
| N2 | 통과 출력이 늘 `{}`는 아니다 | `Host::pass(event_arg)`. 인자만으로 정해져 감시 스레드와 panic 처리도 쓴다 | 7.1 |
| N3 | grok은 세션 시작·프롬프트 때 모델에게 글을 못 준다. agy에는 프롬프트 이벤트가 없다 | `Caps`의 `context_at_start`·`context_at_prompt`·`prompt_event`·`user_warn`. 규칙은 닿지 않는 안내를 만들지 않는다 | 5, 7.2, 7.5 |
| N4 | Stop의 종료 이유 | `stop_reason`을 유지한다. agy의 `NO_TOOL_CALL`은 `end_turn`으로 옮긴다 | 7.2, 7.5 |
| N5 | agy는 `workspacePaths`를 주고 작업 디렉터리가 플러그인 폴더다 | `cwd = workspacePaths[0]`. hook은 프로세스의 작업 디렉터리를 쓰지 않는다 | 7.1, 7.2 |
| N6 | agy는 `lastUserInput`을 모든 이벤트에 준다 | `HookInput.prompt`에 넣고, 지문이 바뀐 호출에서 P-00·P-01·P-02를 평가한다 | 7.5 |
| N7 | 도구 이름과 인자 이름이 다르다 | 어댑터가 `ToolCall`로 옮긴다. 7.4·7.5절은 호스트를 모른다 | 7.2, 7.4 |
| N8 | 패키지와 등록 파일이 둘 더 생긴다 | 배포 폴더는 둘 그대로이고 grok·agy의 패키지는 설치할 때 조립한다 | 10.3, 10.4 |
| D1 | grok의 hook 등록 방식 | G-나(전역 파일, exec form, 고정 위치 실행본의 절대 경로). 이유: PowerShell에서 확실히 도는 것이 exec form뿐이다 | 10.4 |
| D2 | grok의 세션 시작 안내 | `AGENTS.md` 관리 블록과 스킬이 `fullops status`를 먼저 실행하게 하고, SessionStart는 사용자에게 한 줄만 낸다 | 7.2, 7.5, 8.13 |
| D3 | agy의 PreToolUse 통과 출력 | `ask`. 권하는 쪽(`allow`)과 다르게 정했다. 이유: 확인 전에는 권한을 넓히지 않는 값을 쓰고, Orca의 agy hook이 이미 같은 값을 낸다. 확인되면 상수 하나를 바꾼다 | 7.2, 16 M11 |
| D4 | agy의 턴과 preamble 묶기 | `PreInvocation`을 등록하지 않는다(권하는 쪽과 같다). 턴은 `executionNum`이 아니라 `lastUserInput`의 지문으로 센다(권하는 쪽과 다르다). 이유: 막아 이어 갈 때 `executionNum`이 늘면 한 번 막기가 계속 막기가 된다 | 7.5, 7.6 |
| D5 | agy의 F-01 | 어댑터가 `ERROR`로 끝난 Stop을 StopFailure로 옮긴다 | 7.2, 7.5 |
| D6 | grok의 `background_wake` | 거짓으로 시작한다 | 7.2 |
| D7 | grok 폴더 신뢰 | 읽기 전용 검사와 안내만 한다 | 8.5, 10.6 |
| D8 | `--event`를 모든 호스트에 줄 것인가 | agy의 등록에만 준다. 다른 호스트는 stdin을 믿는다 | 7.1 |
| D9 | agy의 배포 경로 | 조립한 로컬 폴더로 설치한다 | 10.3 |
| D10 | 패키지 수 | 권하는 쪽(다섯)과 다르게 정했다: 배포 폴더는 통합 패키지와 디렉터리판 둘, grok·agy 패키지는 설치할 때 조립한다. 이유: 저장소의 바이너리를 한 벌로 둔다 | 10.3, 10.4 |

### 5.3 뼈대 담당이 보고한 빈틈 48건

"받음"은 뼈대가 임시로 정한 것을 설계가 그대로 적었다는 뜻이고(뼈대는 고칠 것이 없다), "고침"은 0절대로 뼈대가 바꿔야 한다는 뜻이다.

| # | 빈틈 | 정한 것 | 뼈대 | 절 |
|---|---|---|---|---|
| 1 | core가 JSON 값 타입을 못 쓴다 | core가 `serde_json`을 쓴다. 보존 자리는 `serde_json::Value`의 맵 | 고침(`Json` enum을 지운다) | 5, 10.1 |
| 2 | State·Session 타입의 자리 | 파일 형식 타입은 모두 `fullops-core::model` | 받음 | 5, 18.1 |
| 3 | `MessageId` 이름 충돌 | 규칙의 문구 키는 `messages::MsgId` | 받음 | 5, 7.5 |
| 4 | `Outcome` 이름 충돌 | 상태 값의 타입은 `WorkOutcome` | 받음 | 5 |
| 5 | `route`와 `done`의 R-xx 충돌 | `route`의 것을 RT-01~RT-05로 | 고침 | 8.3 |
| 6 | `Strength`에 `Warn`이 없다 | 필요 없다(P-03이 `Context`가 됐다) | 고침(`Warn`을 지운다) | 7.5 |
| 7 | X-01의 강도가 둘 | 표에는 `BlockOnce`, `Finding::Hit.always`로 올린다 | 고침(필드 추가) | 7.5 |
| 8 | `Rule.origin`의 값 | 11.1절의 1.24.0 기능 이름 | 받음 | 7.5 |
| 9 | `applies`가 못 나타내는 대상 | `check`가 본다 | 받음 | 7.5 |
| 10 | 경우별 탈출구 | `Rule.escapes`와 `Finding::Hit.case` | 고침 | 7.5 |
| 11 | `ToolCall`의 필드 | 설계가 정했다(뼈대의 구조체에 `truncated`와 `EditOp`) | 고침(작다) | 7.2 |
| 12 | `Host`에 호스트 종류 메서드 | `id() -> HostId`. `pass`와 `decode`의 `event_arg`도 더한다 | 고침 | 7.2 |
| 13 | `update_state`로 `landing`을 채울 수 없다 | `derived` 인자. 넘겨받은 유도값만 바꾼다 | 고침 | 6.2.3, 10.2 |
| 14 | `status.json`의 비용 | 잠금 안에서 자라는 파일을 읽지 않는다. `metrics`는 `status`와 `merge`가 계산해 넘긴다. `archive_keys`는 줄 훑기 | 고침 | 6.2.3 |
| 15 | `status.json`의 뜻 | `version`은 실행본 버전, `mode`는 이전 값 또는 `null`, `tasks`는 state에 흔적이 있는 키, `pending`은 보류 포함 | 받음(`tasks`에서 하위 배정을 빼는 것만 더한다) | 6.2.3 |
| 16 | `status.json` 쓰기 실패 | 무시하고 성공 | 받음 | 6.2.3 |
| 17 | `ready`와 체크아웃 | lead 문맥이 lead 체크아웃에서 계산해 `derived.ready`로 넘긴다 | 고침(13번과 함께) | 6.2.3, 6.6 |
| 18 | `read_plan`의 템플릿 | `read_plan(repo, template)`. 상수는 `assets::PLANS_MD` | 받음(`assets.rs`를 더한다) | 10.1, 10.2 |
| 19 | `seq` | 쓸 때마다 1 올린다 | 받음 | 6.2.1 |
| 20 | `state.json`의 `null` 허용 | 목록을 적었다 | 받음 | 6.2.1 |
| 21 | hold의 필드 | `{reason, owner, resume, at}` | 받음 | 6.2.1 |
| 22 | 결정의 값 | `dropped`, `repository`, `drop_reason` | 받음 | 6.2.1 |
| 23 | 직접 작업 completion의 `id` | `direct:<sha 12자>` | 받음 | 6.2.1 |
| 24 | Orca ID의 문법 | 비어 있지 않고 200바이트 이하, 공백·제어 문자 없음 | 받음 | 5 |
| 25 | 경로 표기 | `path_text`·`same_path` | 받음 | 6.2.1 |
| 26 | waiters 파일 이름 | `[A-Za-z0-9_.-]` 밖 글자를 `_`로 | 받음 | 6.2 |
| 27 | `fullops.toml`의 필수 키 | 목록을 적었다(`subagent_level`은 `parallel`이 됐다) | 받음(이름만) | 6.1.1 |
| 28 | `fullops.toml`의 검증 | `tech_plan_role`은 등록 역할이고 design 역할과 달라야 한다. 빈 `remote`는 오류. 오류 분류 둘 | 고침(검증 둘 추가) | 6.1.1 |
| 29 | 기준 시점 설정을 못 읽을 때 | 내장 기본값. CLI는 한 줄 알린다. `config::baseline` | 고침(함수 추가) | 6.1.1 |
| 30 | 담당 칸의 대문자 | 소문자로 바꿔 읽는다 | 고침 | 6.1.2 |
| 31 | `PLANS.md` 세부 | 열 우선순위·`\|`·괄호는 받음. 토큰 끝의 `)`·`）`도 떼고 코드 펜스 안은 읽지 않는다 | 고침(둘) | 6.1.2 |
| 32 | front matter와 1.24.0 문서 | 1.24.0의 문법으로 읽고 쓴다 | 고침 | 6.1.3 |
| 33 | 인박스 문법 오류의 기준 | 인박스 전용 필드 열 개 | 받음 | 6.1.3 |
| 34 | 세션 파일 | 모르는 필드를 보존한다. `anchor`는 `null` 허용, 빈 세션 ID는 해석 실패 | 고침(보존만) | 6.3 |
| 35 | dispatch로 배정받은 리뷰어의 좌석 | `Reviewer`가 된다(좌석은 3~6번) | 고침 | 7.3 |
| 36 | CLI 출력 | 결과는 stdout, 문구는 stderr. `--json`의 네 형태. 종료 코드 규칙 | 고침(`error`를 객체로, `human` 형태) | 8.1 |
| 37 | CLI 예외 목록 | 활성 표식 예외에 `rules`, 정책 오류 예외와 T-00에 `install`·`rules` | 받음 | 7.5, 7.8, 8.1 |
| 38 | `--repo`·`--json`의 자리 | 명령 이름 뒤 어디에나. 값이 겹치면 `--옵션=값` | 받음 | 8.1 |
| 39 | 10.1절에 없는 파일 | `flow.rs`, `cli.rs`, `model/` 하위 모듈을 적었다. `cmd/sync.rs`는 E의 것이다 | 고침(`sync.rs`의 주인 표기만) | 10.1, 18.2 |
| 40 | store에 없는 함수 | 뼈대가 더한 함수를 10.2절에 적었다 | 받음 | 10.2 |
| 41 | ext의 타입 | 뼈대의 정의가 정본이다. 여섯 git 메서드의 시그니처를 적었다 | 받음 | 10.2 |
| 42 | `OrcaMessage`의 `subject`·`outcome` | 있다 | 받음 | 10.2 |
| 43 | 추적 줄의 어휘 | `actor`와 `verdict`의 값을 적었다(`+sub`를 더한다) | 받음(접미사만) | 6.3 |
| 44 | archive와 migrate | 13.2절 4단계가 `landed_at`에 전환 시각을 쓴다 | 받음(F가 구현한다) | 6.2.1, 13.2 |
| 45 | `stage_of` 세부 | 엄격한 초과, `key` 없는 completion은 세지 않는다. 하위 배정은 건너뛴다 | 받음(하위 배정만 더한다) | 6.6 |
| 46 | 시나리오 형식 | 키마다 줄, 동사 목록, `verdict` 어휘. `host`·`parallel`·`pending`을 더했다 | 고침(키 셋) | 12.1 |
| 47 | 의도 수준 동사의 전제 | 흐름 동사가 앞 단계를 스스로 한다. 동결 뒤 `driver.rs`는 A만 고친다 | 받음 | 12.1 |
| 48 | Git Bash의 명령 길이 약 8KB | 설계와 무관하다. 구현 묶음은 긴 파일을 셸 heredoc이 아니라 파일 쓰기 도구로 쓴다 | - | - |

보고의 `needs`에 대한 답: `GitCli`와 `OrcaCli`는 뼈대가 메서드 전부를 완성본으로 넣는다(모든 묶음이 첫날부터 기댄다). 내장 템플릿의 자리는 `crates/fullops/assets/repo/`이고 상수 모듈은 `fullops::assets`다. `proc::run`은 뼈대의 기본 구현을 두고 D가 프로세스 트리 종료를 더한다. `abandoned.json`·`review/rule.json`의 전체 키(E)와 `lint/lint.json`의 전체 키·인덱스 표 문법(D)은 그 묶음이 정해 A 담당에게 `docs/formats.md` 반영을 요청한다.

### 5.4 모션그래픽 스킬 담당이 보고한 것

| 보고 | 정한 것 | 절 |
|---|---|---|
| 템플릿이 17개가 아니라 18개(#17 포함) | 받는다. 이슈에 모아 둔 템플릿을 싣는다는 요청에 맞다 | 11.3 |
| 1절 7번, 11.2·11.3·14절의 "모션 버림" | 모션을 뺐다. `render.mjs`와 스킬을 v1로 적었다 | 1, 11.2, 11.3, 14, 17 H6 |
| G4의 `.mjs` 0개 | `skills/fullops-motion/render.mjs` 하나는 예외다. `xtask dist`가 그 밖의 것을 잡는다 | 2.1, 10.5 |
| 패키지 배치가 `SKILL.md`만 적는다 | 스킬 폴더를 하위 폴더와 파일째 복사한다 | 10.4, 10.5 |
| 묶음 F의 스킬 수와 "스크립트 경로가 없다" | F는 8개를 쓰고 9개를 배포한다. 확인은 "Python 스크립트 경로가 없다"로 좁혔다 | 18.2 |
| `plugin/` 스킬 자산의 lint·리뷰 | 이 레포가 fullops를 적용할 때 `render.mjs`·`starter/**`·`templates/**`를 lint와 리뷰의 `exclude`에 넣는다 | 10.4 |
| 지시서의 템플릿 번호 | 본문에 `템플릿: #<번호> <이름>` 한 줄. front matter 필드를 더하지 않는다 | 11.3 |
| 활성 레포가 아니어도 진행한다, 도구가 없으면 승인받아 준비한다 | 받는다(스킬 문서의 규약이다) | 11.3, 14 |
| 웹·Unity 조작 테스트 | 그대로 버린다 | 1, 17 H6 |

### 5.5 병렬 진행에서 정한 것

| 물음 | 정한 것 | 이유 | 절 |
|---|---|---|---|
| 방침 키와 `subagent_level` | `parallel` 하나로 합쳤다. `off`·`read`·`on`(기본)·`ultra`. `migrate`는 `off` → `off`, 나머지와 누락 → `on` | 키 하나가 제한과 권장을 함께 나타낸다. `lite`는 1.24.0의 기본값이었다 | 6.1.1, 8.15, 13.2 |
| 호스트별 수단 | `Caps.subagent_tool`(`Agent`·`spawn_agent`·`spawn_subagent`·`invoke_subagent`)과 `Caps.workflow`(Claude Code만). 호스트와 무관한 수단은 Orca 하위 배정과 lead의 역할 단위 배정 | 규칙과 문구가 호스트 이름이 아니라 능력을 본다 | 5, 7.2, 8.15 |
| 수단을 고르는 기준 | 한 세션 안의 탐색·겹치지 않는 파일 묶음은 호스트 서브에이전트, 다른 모델이 필요한 부분 작업은 하위 배정, 서로 다른 워크트리가 필요한 독립 과제는 lead의 역할 단위 배정 | 워크트리가 하나면 게이트가 그대로 걸린다 | 8.15 |
| 안내가 닿는 자리 | S-01, `status`의 첫머리, dispatch의 spec, 스킬 문서. 문구는 `parallel_brief` 하나 | Orca의 preamble이 "task가 요구하지 않으면 하위 에이전트를 쓰지 말라"고 하므로 worker에게는 spec의 문장이 통로다. grok은 SessionStart의 글이 닿지 않는다 | 7.7, 8.5, 8.13, 8.15 |
| 울트라코드 | spec에 키워드를 넣지 않는다. Claude Code worker는 `parallel = "ultra"`일 때 Orca의 `--effort ultracode`로 띄운다. lead는 사용자가 직접 켠다. 없으면 `Agent`로 내려간다 | 공식 문서: 키워드는 사람이 직접 친 프롬프트에서만 opt-in이다. 실행 인자 `claude --effort ultracode`는 문서에 있는 길이다. Orca가 그 값을 넘기는지는 미정-M13 | 8.5, 8.15, 16 |
| T-08과 D-08 | `off`와 `read`에서만 막는다. `on`·`ultra`에서는 `Purpose:`도 요구하지 않는다 | 권장 방침에서 형식으로 막으면 늦어진다(DF-22) | 7.5, 8.5 |
| 하위 worker가 남은 채의 완료 보고 | 1.24.0에는 강제하는 규칙이 없었다(스킬 규약). `done`의 R-11을 새로 뒀다(DF-23) | 보고 하나가 변경 전부를 담는다 | 8.7, 12.3 |
| 하위 worker의 보고와 통합 | 보고는 부모의 Run으로 간다. 같은 워크트리라 병합이 없고 부모가 커밋·lint·`done`을 한다. completion을 만들지 않는다 | lead의 통합 대기에 섞이지 않는다 | 8.6, 8.15 |
| 인박스 "역할당 하나" | 하위 배정은 부모의 키·역할·워크트리로만 된다(D-13). 인박스와 과제명 파일을 만들지 않는다 | 0.9.12에서 폐기한 과제명 파일 병렬 배정을 되살리지 않는다 | 8.5, 11.4 |
| 유지할 결정과의 관계 | "리뷰·테스트는 최소·표적, LLM 호출을 늘리지 않음"은 리뷰와 테스트에 관한 것으로 읽는다. 병렬 서브에이전트는 별개로 권장하고 리뷰·테스트를 서브에이전트로 늘리지 않는다 | 레포 주인의 요구 | 3, 8.15 |
| 사용량 | `on`·`ultra`의 문구에 "사용량이 빨리 준다. 끄려면 `parallel`을 `off`로"가 들어 있다 | 켜고 끄는 것을 세션이 안다 | 8.15 |
| 시나리오와 지표 | R1~R8. 지표는 서브에이전트 호출 수, 워크플로 실행 수, 하위 배정 수(T-13이 `turns.jsonl`에 `kind`만 쓴다) | 본문 없이 병렬 사용을 센다 | 6.7, 12.2 |

### 5.6 남은 미정과 사람 결정

- 미정(16절): M10(grok)과 M11(agy)은 "세션에서 봐야 하는 것"만 남았다. M13이 새로 생겼다(Orca가 `--effort ultracode`를 넘기는가, 하위 배정을 Orca가 받는가). 셋 다 실환경 점검(12.4절 12~14번)에서 사람이 한 번 보고, 뼈대 동결을 막지 않는다.
- 사람 결정(17절): H11을 확정으로 바꿨다(네 호스트 모두 hook을 등록한다. agy에서 hook 실패가 도구를 막는 것으로 확인될 때만 agy의 hook을 뺀다). H12(병렬의 기본값과 `lite` → `on`의 이행, `ultra`의 방식)와 H13(grok의 전역 hook 폴더에 fullops 전용 등록 파일을 쓰는 것)을 더했다. H5에 DF-23을 더했다.
- 설계가 계약 문서의 권고와 다르게 정한 것은 셋이다(5.2절의 D3, D4의 턴, D10). 이유는 그 표에 있다.

## 6. 동결 전 점검 반영 (2026-11-14)

동결 직전의 설계와 뼈대를 다른 모델이 함께 점검해 낸 지적 16건이다. 문서와 코드, 근거(`docs/contracts/`, `scenarios/fixtures/`, 1.24.0의 `flow_gate.py`·`policy.py`·`setup.py`, Claude Code 공식 문서의 workflows 쪽)로 확인했다. 15건을 고쳤고 1건(14번)은 지적의 전제가 사실과 달라 작은 보강만 했다. 설계와 뼈대(`crates/`, `docs/formats.md`)를 같은 값으로 함께 고쳤다.

### 6.1 지적 16건

| # | 지적 | 판정 | 고친 것 | 절 |
|---|---|---|---|---|
| 1 | `docs/formats.md`에 †가 21곳 남았고 뼈대의 두 번째 빈틈이 설계에 없다 | 맞다. †는 머리말의 설명 한 줄을 빼면 20곳이다 | 20곳을 6.2절의 표대로 설계에 적고 †와 머리말의 설명을 지웠다 | 6.2절 |
| 2 | 하위 worker를 띄운 부모 worker가 턴을 끝낼 때 X-01·X-03이 먼저 걸리고, X-03의 탈출구(`done`)를 R-11이 거절한다. R4는 X-05를 기대한다 | 맞다 | "하위 작업 남음"(`State::sub_open`)을 6.6절에 정의했다. 참이면 X-01·X-03을 건너뛰고 X-05만 그 Run을 본다. 부모의 Stop 결과 표를 7.5절에 넣고 R4의 기대값을 그 표와 맞췄다. R-11이 같은 함수를 쓴다. 뼈대에 완성본과 시험을 넣었다 | 6.6, 7.5, 8.7, 8.15, 12.2 R4, 12.3 DF-23 |
| 3 | Windows의 Claude Code는 PowerShell 도구가 주 셸인데, 런처와 `.exe`가 한 폴더에 있는 `bin/`에서 맨 이름 `fullops`가 실행되는지 다루지 않았다 | 일부만 맞다. PowerShell 7.7과 5.1에서 PATH로 찾는 맨 이름은 `fullops.exe`로 풀린다(임시 폴더에서 직접 확인. 호스트는 쓰지 않았다). 실행되지 않는 것은 확장자 없는 경로를 준 경우뿐이다. Claude Code 세션에서의 확인은 남았다 | 10.3절에 확인한 사실과 규칙 셋(맨 이름, Windows의 경로에는 `.exe`, 그 밖의 호스트는 전체 경로)을 적었다. 실환경 점검 1번과 M1에 더했다. 안 풀릴 때 고칠 곳을 `spec_command` 하나로 좁혔다 | 8.5, 10.3, 12.4, 16 M1 |
| 4 | Workflow 승인 창이 Manual·accept edits에서 실행마다 뜬다. `ultra`로 띄운 worker가 멈춘다 | 맞다(공식 문서를 다시 읽어 확인했다) | 8.15절의 사실 목록·정한 것·한계에 적었다. fullops는 권한 모드를 바꾸지 않고 허용 규칙도 넣지 않는다. `dispatch`의 출력과 `doctor`가 한 줄 알린다. M13과 실환경 점검 14번, H12에 더했다 | 8.15, 12.4, 16 M13, 17 H12 |
| 5 | 1.24.0에서 `subagent_level`이 없으면 실제 동작은 `off`인데 `migrate`가 `on`으로 옮긴다 | 맞다(`policy.py`의 누락 기본값이 `off`다) | 누락 → `on`을 유지하고 13.2절의 원칙에서 하나뿐인 예외로 적었다. 보고서가 "꺼져 있었다"를 낸다. H12에 열린 결정으로 올렸다 | 8.15, 13.2, 12.2 R8, 17 H12 |
| 6 | `pending[]`에 completion ID가 없어 `landing`을 키와 SHA로 잇는다 | 맞다 | `pending[]`에 `id`를 넣고 `id`로 잇는다. 예시, `StatusPending`, `build_status`, 시험을 고쳤다 | 6.2.3 |
| 7 | `baseline(toml_text)`가 core에 있다고 적혀 있는데 core는 `toml`을 쓰지 않는다 | 맞다 | 두 함수(store의 글을 받는 함수, core의 고르는 함수)로 적었다. 뼈대는 그대로다 | 6.1.1, 10.2 |
| 8 | `load_deps(plugin_root)`로는 내장 사본을 읽을 수 없다 | 맞다 | `load_deps(plugin_root, builtin)`. 내장 사본의 상수는 `assets::DEPS_TOML`(F가 `plugin/deps.toml`을 만들 때 `include_str!`로 더한다) | 10.1, 10.2, 10.7 |
| 9 | J의 Jev 시나리오가 C의 `JevCurl`을 기다려야 한다 | 맞다. `JevCurl`은 전송 스키마(미정-M6)를 C가 정해야 해서 A의 완성본으로 올릴 수 없다 | `fake-curl`을 타는 단계는 `pending = "C"`. `ScriptJudge`는 프로세스 안의 시험에만 쓰고 단위 시험 종류에 하나를 더했다 | 10.2, 12.5, 18.2 |
| 10 | Orca가 `result` 없이 `error`로 거절한 배정의 처리가 없다 | 맞다 | 8.5절 결과 표에 줄을 더했다: 예약 기록을 지우고 종료 코드 4. I7에 경우를 더했다 | 8.5, 8.15, 10.2, 12.2 I7 |
| 11 | `Plan`·`Decision`의 이름이 겹치고 `TreeEntry`의 자리가 설계와 다르다 | 맞다 | 10.2절에 경로(`deps::Plan`, `explore::Decision`, `model::TreeEntry`)를 적고 18.1절의 "모두 `model`"에 예외를 적었다 | 10.2, 18.1 |
| 12 | spec의 `fullops`를 절대 경로로 바꾸는 주체와 방법이 없다. 하위 배정 문장에 `<KEY>`가 그대로 든다 | 맞다 | `BriefAt::Spec { command, key, sub }`로 `parallel_brief`가 문장 전체를 만든다. 실행 명령은 `spec_command`. spec에서 실행 명령으로 적는 자리 셋을 8.5절에 적었다. `SUB_DISPATCH_HINT` 상수를 지웠다 | 8.5, 8.15, 10.2 |
| 13 | 세션에 묶이지 않은 경로에서 R-08의 후보가 둘이 되고 R-11·R-01이 빠진다 | 맞다 | R-08 둘째 길의 후보에서 하위 배정을 뺀다. R-11은 대상 dispatch(묶인 것, 없으면 둘째 길) 기준으로 본다. 묶이지 않은 하위 worker 세션의 `done`은 R-11이 거절한다 | 8.7 |
| 14 | 수정 동사 검사를 뺀 것이 DF 목록에 없다 | 사실과 다르다. DF-6이 "lite 위임의 수정 동사 휴리스틱이 없다"를 이미 적고 있다 | 고치지 않았다. T-08 (g)에 DF-6을 가리키는 한 문장을 넣고 D7에 수정 동사가 든 경우를 더해 비교에서 의도한 차이로 잡히게 했다 | 7.5, 12.2 D7 |
| 15 | A의 완성본이 다른 묶음 소유 파일 안에 있다 | 맞다 | 파일을 옮기지 않고 규칙을 적었다: 그 함수·상수·타입과 10.2절의 모든 시그니처는 파일의 주인도 A 담당의 동의 없이 바꾸지 않는다 | 10.2 끝, 18.2 규칙 1 |
| 16 | agy의 통과 값 `ask`에서 사람 없는 worker가 멈출 수 있다 | 맞다(알려 둘 위험이다) | 7.2절에 위험을 적고 실환경 점검 13번과 M11에 더했다 | 7.2, 12.4, 16 M11 |

### 6.2 뼈대가 `docs/formats.md`에 †로 남긴 것 20곳

"받음"은 뼈대가 정한 것을 설계가 그대로 적었다는 뜻이고(뼈대는 고칠 것이 없다), "고침"은 설계와 뼈대를 함께 바꿨다는 뜻이다.

| # | †가 있던 곳 | 정한 것 | 뼈대 | 절 |
|---|---|---|---|---|
| 1 | `baseline`의 자리(두 줄)와 글이 없을 때의 `test_level` | store와 core의 두 함수. 글이 없으면 `test_level = "lite"` | 받음 | 6.1.1, 10.2 |
| 2 | find·context 결과의 `model` | Jev를 부르지 못했으면 `null` | 받음 | 6.1.5 |
| 3 | 패킷 `items[].source_basis` | 읽지 못한 항목은 `null` | 받음 | 6.1.5 |
| 4 | 패킷 `items[].evidence[]`의 `symbol`·`line`·`seed` | 값이 있는 것만 쓴다 | 받음 | 6.1.5 |
| 5 | `pending[].landing`을 잇는 키 | `pending[].id`(completion ID) | 고침 | 6.2.3 |
| 6 | `metrics`의 처음 값 | 모든 수가 0이고 `turns_per_task`가 `null`. `at == 0`이 "아직 계산하지 않았다" | 받음 | 6.2.3 |
| 7 | `DepsError::Parse`의 범위 | 읽기 오류, TOML 오류, 필수 필드 누락, 모르는 호스트 ID | 받음 | 10.2 |
| 8 | `load_deps`의 시그니처 | `load_deps(plugin_root, builtin)`. 내장 사본을 썼으면 경로가 비어 있다 | 받음 | 10.2, 10.7 |
| 9 | agy에서 `--event`가 없거나 값이 빠진 통과 | `{}` | 받음 | 7.1 |
| 10 | `fullops rules --json`의 형태 | `{"ok":true,"rules":[{"id","event","strength","escapes":[{"case","escape"}],"origin"}]}` | 받음 | 8.13 |
| 11 | `--json`일 때의 stderr | 거절·오류의 사람용 문구를 stderr에 함께 낸다 | 받음 | 8.1 |
| 12 | `DispatchRequest.sub` | 호출자가 dispatch가 묶인 세션이다. 부르는 쪽이 채운다 | 받음 | 10.2 |
| 13 | `CompletionNote`의 필드 | `key`, `role`, `sha`, `sha_source`, `outcome` | 받음 | 10.2 |
| 14 | `TreeEntry`의 자리 | `fullops_core::model`(store도 받는다) | 받음 | 10.2, 18.1 |
| 15 | spec의 하위 배정 문장을 누가 붙이는가 | `parallel_brief`가 `BriefAt::Spec`의 값으로 붙인다. `dispatch`는 떼지 않는다 | 고침 | 8.5, 8.15 |
| 16 | `messages::reason`이 문구를 못 찾았을 때 | `args`를 공백으로 이어 낸다 | 받음 | 10.2 |
| 17 | `diff_names`·`numstat`의 이름 변경과 `commit_only`의 새 파일 | `--no-renames`로 삭제와 추가로 낸다. `commit_only`는 바뀐 경로만 `add -A` 뒤 `commit --only` | 받음 | 9.1, 10.2 |
| 18 | `OrcaCli`의 필드 | `{ program, cwd, timeout }`와 `OrcaCli::new(cwd, timeout)` | 받음 | 10.2 |
| 19 | `worker_start`의 `Err` | 시작되지 않은 것이 확실한 실패(실행 파일 없음, Orca가 `error`로 거절). `dispatch`는 예약 기록을 지운다 | 받음(설계에 처리 줄을 더했다) | 8.5, 10.2 |

†가 없던 두 번째 빈틈 셋도 함께 적었다. X-01·X-03은 `Finding.case`를 쓰지 않는다: 탈출구 문장 하나가 두 쪽을 함께 적는다(7.5절. 받음). `HOST_COMMANDS`의 타입은 F가 정하고 뼈대는 두지 않는다(10.2절). 내장 자산: `deps.toml`의 내장 사본은 파일을 따로 두지 않고 `assets::DEPS_TOML`이 `plugin/deps.toml`을 읽는다(10.1절).

### 6.3 뼈대가 바뀐 곳

| 파일 | 바뀐 것 |
|---|---|
| `fullops-core/src/model/state.rs` | `StatusPending.id`. `State::sub_open`(완성본) |
| `fullops-core/src/model.rs` | `BriefAt<'a>`의 `Spec { command, key, sub }` |
| `fullops-core/src/messages/mod.rs` | `parallel_brief`가 하위 배정 문장을 `Spec`의 값으로 만든다. `spec_command`(완성본). `SUB_DISPATCH_HINT` 삭제 |
| `fullops-core/src/facts.rs` | `sub_open`의 시험 |
| `fullops-store/src/state.rs` | `pending[].id`를 쓰고 `landing`을 `id`로 잇는다 |
| `fullops-store/src/files.rs`, `lib.rs` | `fixed_bin`(완성본) |
| `fullops-store/tests/store.rs` | SHA가 없는 건의 `landing`이 이어지는지 본다 |
| `docs/formats.md` | † 20곳과 머리말의 설명을 지웠다. 4.4절(`pending[].id`), 8절(`BriefAt`, `spec_command`, `sub_open`, `fixed_bin`) |

### 6.4 남은 미정과 사람 결정

- 미정(16절): M1에 "Windows의 Claude Code에서 맨 이름 `fullops`가 풀리는가"를, M11에 "`ask` 통과에서 사람 없는 worker가 멈추지 않는가"를, M13에 "Orca가 띄운 Claude Code worker의 권한 모드와 Workflow 승인 창"을 더했다. 셋 다 실환경 점검(12.4절 1·13·14번)에서 사람이 한 번 보고 뼈대 동결을 막지 않는다. 함수 하나로 좁힌 것이 넷이 됐다(`spec_command`가 늘었다).
- 사람 결정(17절): H12에 둘을 더했다. (1) 키가 없어 1.24.0에서 꺼진 채 돌던 레포도 `migrate`가 `on`으로 옮긴다. (2) `ultra`는 worker가 auto 모드나 bypass로 뜨는 PC에서만 사람 없이 이어진다. 둘 다 열림이다.
