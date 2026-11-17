# 설계 개정 기록 (2026-11-14)

- 대상: `docs/design.md`.
- 반영한 것: (1) 계약 확인 결과 `docs/contracts/design-delta.md`(Claude Code 2.1.323, Codex 0.163.0, Orca 1.4.256). (2) 레포 주인의 결정(2026-11-13~14): 이름과 버전 확정, 이슈 자동 모드 미룸 확정, Jev 탐색을 v1에, 의존 플러그인·스킬 설치를 v1에, 네 호스트(Claude Code·Codex·grok·agy) 모두에 설치, 그 밖의 버리는 기능 확정. (3) 마무리 개정(같은 날): grok·agy 계약 확인 결과 `docs/contracts/hosts-delta.md`, 뼈대 담당이 보고한 설계 빈틈 48건, 모션그래픽 스킬 `fullops-motion`의 포함, 병렬 진행 방침(레포 주인의 새 요구). (4) 동결 전 점검(같은 날): 다른 모델이 동결 직전의 설계와 뼈대를 함께 점검해 낸 지적 16건과, 뼈대 담당이 `docs/formats.md`에 †로 남긴 두 번째 빈틈 20건. 설계와 뼈대를 함께 고쳤다(6절).
- 읽는 순서: 뼈대(묶음 A) 담당은 0절과 `docs/design.md`만 보고 고치면 된다. 0절은 앞 개정과 마무리 개정을 합친 완전한 목록이다. 1절은 앞 개정의 절 번호 순 목록, 2절은 계약 확인(`design-delta.md`)이 "결정 필요"로 남긴 것을 어떻게 정했는지, 4절은 개정본을 다른 모델이 점검해 낸 지적 19건의 처리다. 5절이 마무리 개정이고 6절이 동결 전 점검 반영이다. 6절이 바꾼 타입·시그니처·형식은 0절의 표에도 넣었다(0절은 여전히 완전한 목록이다). 1~4절의 서술 가운데 마무리 개정이 뒤집은 것(agy에 hook이 없다, `host_for(arg, raw)`, `subagent_level`, `status.json`은 그대로다, `~/.fullops/package/<버전>`이 통합 패키지의 사본이다)은 0절과 5절이 정본이다.
- 표기: "절"은 `docs/design.md`의 절 번호다.
- 7절은 여덟 구현 묶음이 설계의 빈 곳을 정한 것을, 한 브랜치에 합친 뒤의 코드와 대조해 옮겨 적은 기록이다(문서 담당). 0~6절은 뼈대 동결 전의 개정이고 7절은 그 뒤의 것이다.

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

## 7. 구현 묶음이 정한 것 (2026-11-14)

여덟 묶음(B hook, C 흐름 명령, D lint·문서, E 리뷰·통합, F 설치·이행·배포·스킬, G 비교·성능, H 패널, J 탐색)이 서로 다른 워크트리에서 구현하면서 설계의 빈 곳을 정했고, 그 보고(설계 빈틈과 다른 묶음에 필요한 것)를 한 브랜치에 합친 뒤의 코드와 대조해 설계와 형식 문서에 옮겼다. 건마다 합쳐진 코드에서 구현을 확인했다. "절" 칸에 절 번호만 있으면 `docs/design.md`의 절이고, 그 절 끝의 "구현 묶음이 정한 것" 목록이나 본문의 해당 문장에 적었다. `formats`는 `docs/formats.md`다. 구현이 설계의 의도와 어긋나 보이는 건은 문서를 고치지 않고 7.4절에 코드 문제로 모았다(절 칸에 "코드 문제 n").

### 7.1 건별 표

| 묶음 | # | 무엇이 비어 있었나 | 어떻게 정했나 | 절 |
|---|---|---|---|---|
| B | 1 | `Rule.check`의 시그니처로는 호출 문맥(레포 루트, `ORCA_CLI_COMMAND`의 이름, S-04의 조건, PATH의 git·orca 유무, 규칙이 내는 효과의 값)을 넘길 수 없다 | `rules::Scene { root, orca_names, self_install, missing_tools }`과 `rules::decide_in(scene, input, facts)`. 문맥은 `rules/call.rs`의 thread-local. 동결된 `decide`는 그대로 있고 `decide_in`을 부른다 | 7.1, 10.2, formats 7 |
| B | 2 | `Facts::session()`이 '세션 파일이 없다'와 '읽지 못했다'를 가르지 못한다 | `Unknown("no_session")`(`rules::NO_SESSION`)이 '없다'의 관례다 | 7.1, formats 7 |
| B | 3 | SessionStart를 받지 못한 세션에 UserPromptSubmit·Stop이 오는 경우(설계는 agy만 적었다) | 세션 파일을 쓰는 효과가 있으면 어느 호스트든 S-02와 같은 내용으로 먼저 만든다 | 7.1 |
| B | 4 | X-04의 '`handovers/logs/<KEY>.`로 시작하는 파일'을 볼 방법(`Facts`에 폴더 훑기가 없다) | `<KEY>.<역할>.r0~r9.md`의 존재로 본다. 차수가 10을 넘으면 놓친다 | 7.5(한계로 적음), 코드 문제 7 |
| B | 5 | S-04의 효과를 실행하는 방법(10.2절 표에 묶음 사이 함수가 없다) | 플러그인 루트의 실행본을 `install --only runtime`으로 띄운다(3초, 실패는 추적에만). 프로세스 안의 진입점은 F의 `cmd::runtime_stage` | 7.5, 10.2, formats 8 |
| B | 6 | T-11이 부르는 함수와 Run을 주지 않은 `check`의 처리 | C의 `record_message`를 그대로 부른다. Run이 없으면 lead의 열린 Run이 하나일 때만 그 Run으로 기록한다 | 7.5 |
| B | 7 | 12.2절 J1의 둘째 경우(로컬 기본 브랜치에만 있음)의 첫 차단 | 설계대로 구현하면 첫 차단이 X-01이고 X-06은 '함께 남은 것'에 붙는다. lead의 X-01에서 병합으로 들어온 커밋을 뺄지는 정하지 않았다 | 7.5(열린 문제로 적음) |
| B | 8 | Stop의 git 호출 상한 6이 worker와 dev 직접 작업 경로에 빠듯하다 | 시작점이 바닥과 같으면 조상 확인을 생략하고, '기록 경로만 바뀌었는가'를 lint 통과와 리뷰 증거가 함께 쓴다. 넘으면 X-02는 작업 트리의 설정으로, 그 밖은 `Unknown` | 7.5 |
| B | 9 | T-00의 대상 '전부'와 7.8절의 '모르는 도구는 통과' | 정책 오류에서도 모르는 도구는 통과한다 | 7.5 |
| B | 10 | 후보가 없는 호출이 읽는 파일(7.4절) | 설계 역할이 등록된 레포의 편집 호출에서만 `state.json`을 더 읽는다(좌석 판정) | 7.4 |
| B | 11 | 세션당 한 번 알림의 '그 밖의 이벤트' | lead의 Stop으로 좁혔다. 7.7절의 그 문장도 lead에게만 낸다 | 7.7, 7.8 |
| B | 12 | `changed()`의 코드 파일 판정이 쓰는 설정 | 작업 트리의 `[code]`와 `lint.json`의 `exclude`. 기준 시점 적용은 `lint`가 한다 | 6.6 |
| B | 13 | `review_evidence`의 '유효한' | 형식과 `head`만 본다(해시 검증은 `review check`). 최근 기록 셋 | 7.5, formats 3.5 |
| B | 14 | T-10의 '차수 접미사를 뗀 것', T-05 (b) | 접미사부터 끝까지 뗀다. 바뀐 뒤 글을 모르는 편집에서는 T-05 (b)를 걸지 않는다 | 7.5 |
| B | 15 | `blocks` 기록 실패로 경고로 낮출 때 '함께 남은 것'의 계속 막기 규칙 | 그 호출은 통과한다(여는 쪽). 다음 Stop에서 다시 평가한다 | 7.6 |
| B | 16 | 이벤트를 알기 전의 감시 스레드 기한 | 가장 긴 12초 | 7.1 |
| B | 17 | coor 모드에서 dispatch가 묶인 기본 브랜치 세션의 역할 안내 | '역할 규칙이 적용되지 않는다' 한 줄 | 7.7 |
| B | 18 | '상태 파일을 직접 주입하지 않는다'(12.1절)의 범위 | 인박스와 `PLANS.md`는 `file`로 준비할 수 있다. `routes/`·`state.json`·리뷰 기록은 넣지 않는다(N1의 파손만 예외) | 12.1 |
| B | 19 | 동사로 적을 수 없는 확인(날 입력, 파일과 fake-orca 기록의 단언) | `crates/fullops/tests/hook.rs`가 실제 바이너리로 본다 | 12.1, 10.1 |
| B | 20 | `docs/formats.md` 7절에 없는 공개 함수 | `read_trace`, `derive::{RECORD_PATHS, is_record_path, is_change_target, landing_of}`, `patch::repo_relative`, `shell::command_name` | formats 7 |
| C | 1 | 미정-M6: Jev 요청·응답의 전송 스키마 | TypeSafe System One HTTP API 형식(요청 `{model, state, questions}`, 응답 `{model, answers, usage}`). 선택지 설명은 `criteria`의 `null`, 역할 설명은 `state.roles` | 9.3, 16 |
| C | 2 | 응답에 `usage`의 토큰 수와 비용이 없을 수 있다 | 없거나 `null`이면 0, 있는데 정수가 아니면 실패. 비용은 없음 | 9.3 |
| C | 3 | `record_message`에 시각 인자가 없다 | core의 `flow.rs`가 시스템 시계를 직접 읽는다 | 코드 문제 1 |
| C | 4 | `record_message`에 조상 판정 인자가 없다 | SHA가 여럿이면 역할 브랜치 tip과 같은 것, 없으면 브랜치 tip | 8.6(지금 동작으로 적음), 코드 문제 2 |
| C | 5 | `check_dispatch`가 git 상태를 보지 못한다 | D-05의 워크트리 없음과 D-06의 snapshot 검사는 `cmd/dispatch.rs`가 이어서 하고 같은 거절 목록에 넣는다. H-04는 `logs/<KEY>.<R>.r0.md`의 존재 | 8.5 |
| C | 6 | `changed()`·`pass_covers`의 공용 시그니처가 뼈대에 없다 | `cmd/done.rs`에 따로 구현했다 | 10.2(남은 것으로 적음), 코드 문제 3 |
| C | 7 | CLI용 `Facts` 구현이 없다 | `cmd/dispatch.rs`의 `CliFacts`. git이 필요한 사실은 `Unknown` | 8.5 |
| C | 8 | `dispatch` 3·4단계와 `wait`의 'Run이 여럿' 거절에 규칙 ID가 없다 | `FullOps[publish]`, `FullOps[sync]`, `FullOps[wait]`(종료 코드 2) | 8.1, 8.5, 8.6, formats 7 |
| C | 9 | 리뷰 배정 spec의 끝 문장(리뷰어는 R-01에 걸린다) | 8.9절의 끝맺음을 적는다. 인박스가 없는 배정은 `Handover:` 조각을 뺀다 | 8.5 |
| C | 10 | D-03 예외의 범위와 D-10 | 글자 그대로: 같은 키의 통합 대기는 목적과 무관하게 막지 않는다. D-10은 Review에도 적용한다 | 8.5 |
| C | 11 | 원격이 앞서 있을 때 3단계의 push가 먼저 실패해 D-11에 닿지 못한다 | 로컬이 원격의 조상이면 push를 건너뛰어 D-11이 알린다 | 8.5 |
| C | 12 | D-07이 여는 결정의 선택지와 출력 형태 | `같은 모델로 다시 배정`·`중단`. D-07뿐이면 사람 손 형태(3), 다른 거절과 함께면 거절 형태 | 8.5 |
| C | 13 | `done --failed`가 타는 검사 | R-01만 검사하고 R-02~R-07을 건너뛴다. `reported`를 쓰고 `[실패]` 본문을 낸다. 인박스는 남는다 | 8.7 |
| C | 14 | `done`을 다시 실행할 때의 lint 증거 | `head` 쪽 조건만 본다. `--no-checks` 사유는 본문에 다시 나오지 않는다 | 8.7 |
| C | 15 | R-05의 폴더와 설계 역할 본문의 '만든 인박스 경로들' | `qa-reports/<KEY>-test/report.md`. `from`이 자기 역할인 인박스 | 8.7 |
| C | 16 | `wait --max-minutes 0`과 Orca 실행 파일이 없을 때 | 한 번의 기다림은 최소 1초이고 기다린 뒤에 만료를 본다. 실행 파일이 없으면 바로 종료 코드 4 | 8.6, 16(M3에 더함) |
| C | 17 | `status`가 통합을 확인할 때의 `drift`와 지표의 '사건' 범위 | `drift`는 세지 않는다. '사건'에 하위 배정과 예약 기록을 넣지 않는다 | 6.7, 코드 문제 6 |
| C | 18 | `task recover`의 호출자 검사와 WIP 커밋 | 그 역할의 브랜치를 체크아웃한 워크트리인지만 본다. `commit_only(["."])` | 8.4 |
| C | 19 | `plan add`의 중복, `plan check`의 종료 코드, `route --role`의 미등록 역할, `dispatch --dry-run`의 출력 | 사용법 오류(1), 0, 사용법 오류(1), 대상 워크트리·tuple·spec | 8.3, 8.5, 8.13 |
| C | 20 | 원본 체크아웃의 정의와 7단계의 '기준 test_level' | Git 공용 디렉터리가 있는 주 체크아웃. 작업 트리의 `fullops.toml` 값 | 8.5 |
| C | 21 | 12.2절 D3의 예시가 그대로는 성립하지 않는다(H-03이 먼저 걸린다) | 다른 키를 다른 역할에 배정해 D-03을 본다 | 12.2 D3 |
| C | 22 | `metrics`의 `at` | 0으로 돌려주고 부르는 쪽(`status`, M-13)이 시각을 넣는다 | 6.7 |
| C | 23 | 이름만 준 `curl`이 Windows에서 PATH보다 System32에서 먼저 찾아진다(시험 중 실제 curl이 가짜 키로 나간 사고) | PATH에서 처음 찾은 `curl`의 전체 경로를 실행한다 | 9.3 |
| C | 24 | `JevCurl`이 `--env-file`과 `--repo`를 받을 방법 | `JevCurl::at(repo_root, env_file)` → `JevCurlAt`. `provider()` | 9.3, formats 8 |
| C | 25 | 보내기 전 본문 검사가 `jev.rs`(`unsafe_text`)와 `explore`(`sendable_text`)에 따로 있다 | 그대로 둘 다 있다 | 코드 문제 10 |
| D | 1 | 8.11절 8번의 Job Object(안전한 API가 없다) | `taskkill /T /F /PID`. 먼저 끝난 부모의 손자는 찾지 못하고 5초만 더 기다린다 | 8.11, formats 8, 17 H17 |
| D | 2 | POSIX의 프로세스 그룹을 lint 명령에만 줄 수 없다(시그니처 동결) | `run`으로 띄우는 모든 프로세스에 `process_group(0)`. 신호는 `kill -KILL -- -<pgid>` 명령 | 8.11, formats 8 |
| D | 3 | `proc::run`의 기다리는 방식 | 출력 EOF를 먼저 기다린다. 자손이 파이프를 쥐면 제한 시간에 `TimedOut`. `TimedOut`에 부분 출력이 남는다 | 10.2, formats 8 |
| D | 4 | `--from` 기본값인 '`changed()`의 시작점'을 부를 함수가 없다 | 6.6절 1~3번을 `cmd/lint.rs`의 `default_start`에 구현. 인박스도 anchor도 없으면 바닥, 바닥도 없으면 `--from` 요구(1) | 8.11, 코드 문제 3 |
| D | 5 | lint의 거절 ID, `--json` 형태, 잠금 실패 | `FullOps[LINT]`. `{ok: <passed>, result, …}`. 잠금을 못 얻으면 종료 코드 1 | 8.1, 8.11, formats 8 |
| D | 6 | `config_sha256`의 정의 | 기준 시점 `lint.json` blob 바이트의 sha256. 없으면 내장 템플릿 글의 sha256. `fullops.toml`은 넣지 않는다 | 8.11, formats 5.6 |
| D | 7 | 컴파일하지 못한 정규식 규칙의 자리 | `commands[]`의 `{name: <코드>, kind: "rule", status: "unrunnable", reason}` | 8.11, formats 5.6 |
| D | 8 | 명령의 `paths`를 맞추는 대상 | `exclude`와 무관하게 바뀐 경로 전부(이름 변경은 앞뒤). 설계대로면 게이트가 약해져 1.24.0을 따랐다 | 8.11 |
| D | 9 | 코드 파일의 기준(1.24.0은 고정 확장자 목록) | 기준 시점 `[code].extensions`. DOC-001은 주석 문법을 아는 내장 확장자에만. 그 밖의 줄 수는 C 계열 주석 규칙 | 8.11 |
| D | 10 | Python의 코드 줄 수(tokenize가 없다) | 줄 머리의 삼중 따옴표를 docstring으로 보는 줄 훑기 | 8.11 |
| D | 11 | DOC-003의 제외 범위가 1.24.0과 다르다 | 설계대로. 인덱스·`project.md`·리뷰 `report.md`·QA 보고서는 바뀌면 front matter가 있어야 한다 | 8.11 |
| D | 12 | 내장 인덱스 템플릿에 front matter가 없어 상태 칸을 고친 커밋이 DOC-003에 걸린다 | 정하지 않았다 | 코드 문제 11 |
| D | 13 | 미정-M7: 산출물 인덱스 표의 열 문법 | 다섯 칸, 첫 칸 `D`+숫자 두 자리, 원천은 백틱 안의 첫 공백 앞까지, `/`로 끝나면 폴더, 같은 ID는 뒤 행. `summary`는 단계 칸 | 8.12, 10.2, 16, formats 3.6 |
| D | 14 | `doc stamp`의 빈 곳 여섯(`--id`, 인덱스 상태 칸, 없는 파일, `--path` 기준, owner·summary 누락, 본문) | `--id` 없음(인덱스에서 찾는다), 인덱스는 맞추지 않음, 제목 줄과 함께 만듦, `.fullops-squad` 기준, 종료 코드 1, 본문을 바꾸지 않음 | 8.12 |
| D | 15 | `doc check`의 종료 코드와 `--json` | 문제 있음 2, 인덱스 없음·없는 ID 1. `{ok, checked, unwritten, errors[], warnings[]}` | 8.12, formats 8 |
| D | 16 | `updated`의 날짜(std에 로컬 시간대가 없다) | UTC 날짜를 쓴다(한국에서는 09시 전까지 어제 날짜) | 코드 문제 8 |
| D | 17 | front matter `{…}` 값이 문자열 객체가 아닐 때 | 문법 오류, `doc check`는 'front matter 없음' | 8.12 |
| D | 18 | `expect`에 '출력에 없어야 한다'가 없다 | 비검출은 요약 줄을 `escape`로 단언한다(rs 전용) | 12.1 |
| D | 19 | Windows에서 확장자 없이 적은 `.cmd` 도구 | PATH와 PATHEXT로 찾는 `resolve_program` | 8.11 |
| D | 20 | `lint/lint.json`의 전체 키와 읽는 규칙 | 키 표, 내장 템플릿 위에 덮기(`size`만 한 단계 더), glob의 `*`가 `/`도 넘음, 어긋나면 종료 코드 1 | formats 3.6 |
| D | 21 | `result.json`의 보충(skipped의 `exit_code`, 빈 `log`, SIZE-002의 `path`) | `null`·0, 빈 글, 빈 글 | 8.11, formats 5.6 |
| D | 22 | lint 중 Ctrl+C·SIGTERM 처리(1.24.0에는 있었다) | 넣지 않았다 | 8.11(한계로 적음) |
| D | 23 | py와 rs의 종료 코드 차이(lint 미통과와 `doc check` 문제: 1과 2) | 비교에서 둘 다 '통과 아님'으로 옮긴다. 2분 넘는 LINT-002 검출 사례는 `--ignored`로만 돈다 | 12.1 |
| E | 1 | `review`·`resume`·`completion set-sha`·`decision`의 거절에 규칙 ID가 없다 | `RV-00`~`RV-10`, `HD-01`·`HD-02`, `DC-00`~`DC-03`. `sync`는 `M-11`, `hold`의 대상 없음은 `M-02` | 8.1, 8.8, 8.9, 8.10, formats 7 |
| E | 2 | M-03의 '깨끗하다'와 M-08의 '미커밋 리뷰 파일을 넣는다'가 부딪친다 | 기록 경로 밖에 변경이 없으면 깨끗한 것이다. M-07이 띄우는 `lint`는 같은 예외가 없다 | 8.8, 코드 문제 9 |
| E | 3 | 리뷰의 base(completion에는 base가 없다) | `merge-base(refs/heads/<base>, head)`. dev 직접 작업은 통합 기준 ref, anchor 순 | 8.8, 8.9, formats 3.5 |
| E | 4 | L4의 기대('M-04 거절')와 8.8절 표가 다르다 | 표를 따른다: 결정을 새로 열고 종료 코드 3(`review.auto`면 2) | 8.8, 12.2 L4 |
| E | 5 | M-04 표에 없는 두 경우(결론 없는 리뷰 기록, 만료된 열린 결정) | 거절(탈출구 `review check`). `dropped`로 닫고 새 결정을 연다 | 8.8 |
| E | 6 | check 9번의 `config_sha256` 비교(D의 계산식이 공개 함수가 아니다) | 구현하지 않았다. lint 결과의 `merge_base`가 리뷰 base의 조상인지로 대신한다 | 코드 문제 4 |
| E | 7 | `merge` 다시 실행의 판정 규칙 | 조상이면 M-04~M-06, 병합 기록 커밋이 있으면 M-07·M-08을 건너뛴다. 병합 전 tip은 병합 커밋의 첫 부모 | 8.8 |
| E | 8 | `Git` trait에 없는 호출 | `fullops_ext::run`으로 git을 직접 부른다(넷) | 10.2 |
| E | 9 | lead가 coordinator 브랜치에 있는 배치의 리뷰 파일 | 병합 기록 커밋에 들어가지 않는다 | 코드 문제 5 |
| E | 10 | 오프라인에서의 `settle`의 fetch와 M-09의 push | fetch는 건너뛴다. push는 오프라인에서도 한다 | 8.8, formats 8 |
| E | 11 | `rule.json`의 `include`, `policy_sha256`, `accepted.json`의 해시, 건너뛴 대상의 뼈대, `abandoned.json` | `include`는 쓰지 않는다. LF로 맞춘 글의 sha256. 파일 바이트의 sha256. `review_status: "skipped"`를 채운다. `{v, key, head, reason, at}` | 8.9, formats 3.5·3.6 |
| E | 12 | 통합 명령의 호출자 검사와 `sync`의 종료 코드 | 검사는 M-01뿐이다. 반영 못 한 역할이 있으면 2, 충돌해 되돌렸으면 3 | 8.8 |
| E | 13 | `gc`의 '정산된 dispatch'의 범위 | 완료 보고가 있는 과제는 archive로. 그 밖에는 정산된 조사·리뷰 dispatch와 부모가 없어진 하위 dispatch만 지운다. 끝난 구현 dispatch는 남긴다 | 6.8 |
| E | 14 | `drift`를 세는 범위 | 역할 브랜치 tip이 통째로 들어간 경우만(first-parent, 병합 커밋·기록 경로 제외). `merge`로 통합한 건은 `null` | 8.8 |
| E | 15 | `--sha`와 `set-sha`의 짧은 값 | 40자가 아니면 `rev-parse`로 푼다 | 8.8 |
| E | 16 | 시나리오 어휘의 한계(리뷰 폴더의 head 12자, 결정 만료, 특정 단계에서 죽이기) | `crates/fullops/tests/review_merge.rs`가 본다. 죽이는 대신 그 단계까지만 끝난 상태를 만든다 | 12.1, 10.1 |
| E | 17 | M-07에서 `lint`의 종료 코드 1과 제한 시간 | 1도 실패로 보고 되돌린다. 3600초 | 8.8 |
| E | 18 | check 9번의 등록 명령 누락·중복을 맞추는 기준 | 기준 시점 `lint.json`의 `commands[].name` | 8.9 |
| F | 1 | `install_plan`(순수 함수)이 경로를 모른다 | 자리표시자 `PACKAGE_ROOT`·`GROK_HOOK_FILE`·`FIXED_BIN`과 `Plan::bind` | 10.2, formats 8 |
| F | 2 | 설치·이행 명령의 거절에 규칙 ID가 없다 | `FullOps[install]`(출처 충돌), `FullOps[migrate]`(전제 위반) | 8.1, 10.3, 13.2, formats 7 |
| F | 3 | `doctor`의 종료 코드 | 알림이 있어도 0. 2를 내는 것은 `install --check` | 10.6, 12.2 Q6·Q10 |
| F | 4 | 되돌린 뒤 `fullops.toml`을 남기면 두 판의 hook이 함께 돈다 | `fullops.toml.rolled-back`으로 이름을 바꾼다 | 13.3, formats 2, 17 H18 |
| F | 5 | 1.24.0의 빈 인박스는 0바이트 파일이다 | `migrate`가 지우고 보고서에 내며 `--rollback`이 다시 만든다 | 13.2, 13.3 |
| F | 6 | 이슈 자동 모드의 잡 확인(Python판은 sqlite) | `fullops-issues/status.json`의 `enabled`와 `job.status`만 본다 | 13.2 |
| F | 7 | `doctor`·`install --check`의 '네트워크 없이 본다' | `FULLOPS_OFFLINE=1`이면 호스트 CLI를 부르지 않고 '모름'으로 둔다 | 8.1, 10.3, 10.6, formats 8 |
| F | 8 | fake-host 방식의 전제가 실제 CLI가 PATH에 있는 PC에서 깨진다 | install·doctor 단계는 PATH를 가짜 폴더 하나로 좁힌다. `Sandbox::command`가 실제 호스트 CLI·npx가 든 폴더를 PATH에서 뺀다 | 12.1, formats 9.1 |
| F | 9 | `plugin/`의 hook 등록 파일 | 정적 파일을 두지 않고 `xtask dist`가 `deps`의 표(`HOOK_TIMEOUTS`, `pre_tool_matcher`)에서 만든다 | 10.1, 10.5 |
| F | 10 | `xtask dist`의 인자 | `--root`, `--target`, `--prebuilt`, `--no-validate` | 10.5 |
| F | 11 | `init` 다시 실행(주석을 보존하는 TOML 편집 크레이트가 없다) | 최상위 키 줄만 바꾸고 역할 표를 끝에 덧붙인다. `[git]`은 바꾸지 않는다 | 10.6 |
| F | 12 | '심볼릭 링크 대상은 거부한다'의 범위 | `.fullops-squad`·`fullops.toml`이 링크면 전체 거절, 엔트리 문서나 개별 템플릿이 링크면 그 대상만 건너뛴다 | 10.6 |
| F | 13 | dev 모드 `primary_branch`의 출처 | 실행한 체크아웃의 브랜치(detached면 `mode`는 거절) | 10.6 |
| F | 14 | `init --local-only`의 역할 브랜치 | 만들지 않는다 | 10.6 |
| F | 15 | 1.24.0 coor 모드의 `primary_role`과 `제품 기획 역할` | `kind = "coordinator"`로 옮기고 `primary_role`은 적지 않는다. `kind = "design"` | 13.2 |
| F | 16 | 설치 기록의 `complete` | 건너뛴 단계가 있으면 `false`. `--only`는 이전 값을 잇는다. `deps.toml`이 바뀌면 다른 호스트의 값도 내린다 | 10.3, formats 6 |
| F | 17 | `install --check`가 목록을 읽지 못했을 때와 grok·agy의 id | 모름이면 빠졌다고 하지 않는다. 이름(`fullops`)만 맞아도 있는 것으로 본다 | 10.3 |
| F | 18 | 플러그인 루트 없이 고정 위치 실행본으로 실행한 실행본 단계 | 자기 자신을 복사하지 않는다 | 10.3 |
| F | 19 | `doctor`의 Git Bash 행과 agy 패키지 행 | Git Bash 행은 넣지 않았다(등록이 exec form이다). '활성 레포이고 설치 기록에 agy가 있다' | 10.6 |
| F | 20 | `upgrade`의 필수 적용 검사와 `--verify`의 종료 코드 | `applied_version`이 2.0.0보다 낮은 레포에서만. 통과 못 하면 2 | 10.6 |
| F | 21 | 버린 기능의 자산 | 넷을 싣지 않았다. 내장 `lint.json`의 `.fullops-squad/board/**` exclude는 남아 있다 | 11.3, 코드 문제 12 |
| F | 22 | 구동기에 `cargo xtask`와 동시 실행을 적을 방법이 없다 | Q9와 N2의 일부는 `crates/fullops/tests/bundle_f.rs`가 한다 | 12.1, 10.1 |
| F | 23 | `names = ["*"]` | `--skill *`를 글자 그대로 넘긴다(미정-M12) | 9.5, 16 |
| F | 24 | 패키지의 `INSTALL.md` 원본의 자리 | `plugin/INSTALL.md`. `xtask dist`가 패키지 루트로 복사한다 | 10.1 |
| F | 25 | S-04가 부를 진입점 | `cmd::runtime_stage(plugin_root)` | 10.2, formats 8 |
| F | 26 | `docs/formats.md`에 없는 것(`deps`의 표와 함수, `hosts`의 두 함수, 홈과 공용 디렉터리의 새 경로, 관리 블록 표식) | 적었다 | formats 2·6·8 |
| F | 27 | `LICENSE`와 매니페스트의 `license`·`repository` | `LICENSE`가 없다. `license`는 넣지 않았고 `repository`는 `tourbut/fullops` | 10.5, 17 H15 |
| F | 28 | `xtask dist`가 저장소 루트에 만드는 `dist/`와 카탈로그 둘 | 저장소에 넣지 않았다 | 10.5, 17 H16 |
| F | 29 | POSIX 런처의 실행 비트(Windows에서 만든 커밋) | `git update-index --chmod=+x`가 필요하다. `dist`는 POSIX에서만 실행 비트를 준다 | 10.5 |
| G | 1 | 12.4절의 '대표 입력 12개'(나열은 13개)와 '주요 명령'의 목록 | 13개. `rules --json`과 `status --json` | 12.4 |
| G | 2 | git·orca 호출 수와 파일 쓰기 수를 세는 법(대리 실행 파일이 없다) | `GIT_TRACE2_EVENT`, fake-orca의 `calls.jsonl`, 두 `fullops/` 폴더의 바뀐 파일 수. 세는 실행은 따로 한다 | 12.4 |
| G | 3 | 시나리오 형식에 DF를 적는 자리가 없다 | 12.2절 행의 DF, '더 나음', 1.24.0 칸의 `-`로 행 단위로 가린다 | 12.1 |
| G | 4 | 1.24.0의 caveman 경고(DF-12)가 모든 `session_starts`에서 회귀로 잡힌다 | py가 경고이고 rs가 이어지는 `session_starts`는 DF-12로 본다 | 12.1 |
| G | 5 | 흐름 동사의 판정 어휘가 py와 rs에서 다르다 | 흐름(이어짐·멈춤)만 견준다. hook 동사는 판정 종류를 견준다 | 12.1 |
| G | 6 | '병합된 과제 수'와 'lead가 받은 차단 수'를 세는 법 | 통합 기준 ref의 `<KEY> 구현` 커밋의 키 수. lead 단계 가운데 차단·거부인 수 | 12.4 |
| G | 7 | 1.24.0의 분류는 설계 역할이 있어야 돌고 오프라인이면 폴백이다 | py 구동기가 fake-curl과 가짜 키로 그 한 호출만 돌린다. 없으면 `architecture`를 등록한다 | 12.1 |
| G | 8 | 1.24.0의 lead Stop이 `worker-list`를 묻는다 | py 구동기가 `worker-list` 규칙 하나를 유지한다 | 12.1 |
| G | 9 | 1.24.0에는 준비 커밋·동기화·병합 명령이 없다 | `lead_dispatches`·`lead_merges`가 스킬 절차를 셸 도구 호출로 수행한다 | 12.1 |
| G | 10 | py의 PostToolUse 알림(DF-2) | 판정에 넣지 않는다. bench의 `pre_*`는 두 hook의 합이다 | 12.1, 12.4 |
| G | 11 | 1.24.0의 `stop_hook_active`(DF-11) | 막힌 세션에 다음 사용자 입력 전까지 참으로 싣는다 | 12.1 |
| G | 12 | 1.24.0에 대응이 없는 단계 | '기준선 없음'으로 센다 | 12.1 |
| G | 13 | `report.md`의 표 | 합격 점검 표와 '끝까지 간 정도' 표를 앞에 더했다 | 12.1 |
| G | 14 | rs 구동기의 흐름 동사가 `env`를 넘기지 않는다 | 세션 핸들이 있는 lead는 `runs_fullops`로 적는다 | 12.1, formats 9.4 |
| G | 15 | 시나리오 파일 이름과 ID | 이름의 첫 `-` 앞이 12.2절의 ID다 | 12.1 |
| G | 16 | 비활성 레포의 목표 15ms가 프로세스 바닥값과 거의 같다 | 합친 뒤 다시 재고 정한다 | 12.4, 16(M9) |
| G | 17 | testkit의 `bin_path`가 `target/<profile>/deps/`를 가정한다 | `xtask`가 자신을 `deps/`에 복사해 다시 실행한다(`rerun_in_deps`). 2026-11-15: rustc 1.100의 새 빌드 배치(`<프로필>/build/<패키지>/<해시>/out/`)에서 깨져, `bin_path`가 위쪽의 `debug`·`release` 폴더를 찾게 고치고 `rerun_in_deps`를 지웠다 | 문서에 적지 않음(구현 세부) |
| H | 1 | 설계는 패널을 TypeScript라고 적었다 | 빌드 단계와 npm 의존이 없는 JS(`worker.mjs`)와 HTML 한 파일 | 10.1, 14 |
| H | 2 | 계약 문서에 매니페스트의 파일 이름과 필드가 없다 | `orca-plugin.json`, `manifestVersion` 2, `contributes.panels`, `capabilities`, worker의 `activate` | contracts/orca 5 |
| H | 3 | 패널 프레임이 두 가지다(데스크톱의 격리 게스트, 브라우저 클라이언트의 srcdoc) | 인라인만 허용하는 쪽에 맞춰 CSS와 JS를 `index.html` 하나에 넣었다 | contracts/orca 5, 10.1 |
| H | 4 | `orca plugin check`는 components 패널만 그려 본다 | HTML 패널의 화면은 `plugin shot`이나 미리보기로 본다 | contracts/orca 5 |
| H | 5 | 미정-M4: worker가 fs로 읽을 수 있는가 | 코드로는 제한이 없다고 보고 fs로 읽게 만들었다. 실행으로는 확인하지 못했다. `status --json` 대안은 만들지 않았다 | 16(M4) |
| H | 6 | 패널이 어느 레포를 보여 주는가 | 워크스페이스에서 찾은 레포 가운데 활성 표식이나 `status.json`이 있는 것. 초점 워크스페이스의 레포를 먼저 | 10.1 |
| H | 7 | 갱신 방식 | 패널이 5초마다 worker에게 묻는다 | 10.1 |
| H | 8 | `status.json`의 `v`가 1보다 클 때 | 그리지 않고 '새 형식이다'를 보여 준다 | 10.1 |
| H | 9 | 매니페스트의 id와 version | `fullops-panel`, `2.0.0` | 17 H14 |
| H | 10 | Orca 밖의 미리보기 | `window.orca`가 없으면 `src/preview.js`의 표본으로 그린다 | 10.1 |
| H | 11 | `xtask dist`의 G4 검사와 패널의 `.js`·`.mjs` | 패널을 뺀다 | 10.5 |
| J | 1 | `PacketInput`에 인박스 템플릿 글이 없다 | `build_packet_from(input, template)`을 옆에 더했다 | 8.14, 10.2, formats 8 |
| J | 2 | `rebuild_packet`의 반환에 거절이 아닌 실패를 실을 자리가 없다 | 규칙 이름 `packet`, `exit`가 4 또는 1인 `Refusal`. `task recover`는 알리고 계속한다 | 8.4, 8.14, formats 7·8 |
| J | 3 | `JevCurl`이 `--env-file`을 받을 방법이 없었다 | J가 키 찾는 순서를 자기 쪽에 한 번 더 구현했고 `JevCurl::default()`를 부른다(C의 `JevCurl::at`을 쓰지 않는다) | 코드 문제 10 |
| J | 4 | 응답 검증과 요청 본문의 글 규칙 검사가 누구의 일인가 | `find`·`context`도 자기 쪽에서 검증하고 `error = "response"`로 폴백한다 | 8.14 |
| J | 5 | 시나리오로 적을 수 없는 확인(`input_sha256`, 파일 300개, 동시 실행) | `crates/fullops/tests/explore.rs`가 같은 레포에서 잇는다 | 12.1, 10.1 |
| J | 6 | `context --paths`의 상대 경로 기준 | 레포 루트. 거부된 경로의 `sources` 값은 `unavailable` | 8.14 |
| J | 7 | `context` 3번의 '34번째 줄 이후' | 35번째 줄부터 찾는다(1.24.0과 같다). 한 줄도 못 실으면 `too large` | 8.14 |
| J | 8 | 색인 항목의 `read_error` 값 | `not utf-8`, `unreadable`. 다음 실행에서 다시 읽는다 | 8.14, formats 5.5 |
| J | 9 | `packet build`의 사유와 근거 이름, '합쳐 64개'의 순서 | `seed definition` 등 여섯 이름. 정의 이름을 먼저 넣고 낱말을 정렬해 채운다 | 8.14 |
| J | 10 | 필수 문서 여섯 가운데 없는 파일 | 설계 그대로 `partial`이고 J-12에 걸린다 | 8.14(알려 둘 결과) |
| J | 11 | 65,536바이트를 넘는 파일 하나로 패킷이 `partial`이 된다 | 설계 그대로다 | 8.14(알려 둘 결과) |
| J | 12 | 지시서 파일도 문자 검색 대상인가 | 대상이다. 지난 패킷 블록을 뺀 글을 훑는다 | 8.14 |
| J | 13 | 패킷 블록의 '줄 <근거 줄들>'의 길이와 빈 '미확인' | 앞 12개와 `…`. 0건이면 줄을 쓰지 않는다. `## 완료 보고`가 없으면 끝에 붙인다 | 8.14 |
| J | 14 | J-04 문구의 '달라진 머리 필드' | `role`을 더했다 | 8.14 |
| J | 15 | `error` 값과 `usage.calls` | `unsafe`를 더했다. `context`도 같은 낱말. 실패한 호출도 센다 | 8.14, formats 3.7 |
| J | 16 | 읽히지 않는 옛 파일의 `history/` | 덮어쓰기 표대로 옮긴다(`keep_history`는 항상 참) | 8.14, formats 3.7 |
| J | 17 | `packet check`를 직접 실행할 때의 대상과 `--head` | 이 체크아웃. `--head`는 조상 검사의 기준 커밋만 바꾼다 | 8.14 |
| J | 18 | 색인의 `config_sha256`에 넣을 제외 목록의 글 | J가 `lint.json`의 `exclude`를 한 번 더 읽어 해시한다 | 문서에 적지 않음(형식은 formats 5.5 그대로) |
| J | 19 | 명시 경로의 '추적 중 변경 경로' | `git status`에서 미추적이 아닌 항목 | 8.14 |
| J | 20 | `history/`를 커밋하지 않게 하는 줄 | 내장 `.gitignore`에 `docs/evaluations/jev/history/`가 있다 | formats 3.7 |

건수는 163건이다(B 20, C 25, D 23, E 18, F 29, G 17, H 11, J 20). 그 가운데 문서에 옮긴 것이 154건이고, 코드 문제로만 넘긴 것이 7건(C3, C25, D12, D16, E6, E9, J3), 문서에 적지 않은 구현 세부가 2건(G17, J18)이다. 문서에 옮긴 것 가운데 B4, C4, C6, C17, D4, E2, F21의 7건은 지금 동작을 적으면서 코드 문제에도 올렸다.

### 7.2 새 규칙 ID와 `fullops rules`

| ID | 내는 명령 | 뜻 | 정본 절 | `fullops rules`의 표 |
|---|---|---|---|---|
| `RV-01`~`RV-10` | `review check` | 8.9절 check의 실패 항목 1~10번 | 8.9 | 없음 |
| `RV-00` | `review`의 그 밖의 거절 | 대상 없음, 폴더가 이미 있음, `waive`·`cleanup`·`supersede`의 전제 위반 | 8.9 | 없음 |
| `HD-01`, `HD-02` | `resume`, `completion set-sha` | 보류된 대상 없음. 모르는 메시지 ID·이미 통합된 completion·풀 수 없는 SHA | 8.8 | 없음 |
| `DC-00`~`DC-03` | `decision` | 없는 결정 ID, SHA 불일치, 만료, 허용 밖의 답한 사람 | 8.10 | 없음 |
| `M-11`(재사용) | `sync` | 반영하지 못한 역할 | 8.8 | 없음 |
| `M-02`(재사용) | `hold` | 대상 completion 없음 | 8.8 | 없음 |
| `FullOps[publish]`, `FullOps[sync]` | `dispatch` 3·4단계 | 기록 게시와 대상 동기화의 거절 | 8.5 | 없음 |
| `FullOps[wait]` | `wait` | 열린 Run이 여럿인데 `--run`이 없다 | 8.6 | 없음 |
| `FullOps[LINT]` | `lint` | 작업 트리가 더럽다 | 8.11 | 없음 |
| `FullOps[install]` | `install` | 마켓플레이스 출처 충돌 | 10.3 | 없음 |
| `FullOps[migrate]` | `migrate` | 전제 위반 | 13.2 | 없음 |
| `FullOps[packet]` | `task recover`의 알림 글 | `rebuild_packet`의 거절이 아닌 실패 | 8.14 | 없음 |

정리: `fullops rules`가 내는 표는 `fullops_core::rules::RULES`, 곧 hook 규칙 34개(S·P·T·X·F)뿐이고 설계 7.5절의 표와 같다. CLI의 거절 ID는 처음부터 그 표에 없었다(RT, H, D, R, M, J 계열도 없다). CLI 거절 ID의 정본은 설계 8절(과 10·13절)의 명령별 절이고, 위의 새 ID도 그 절에 적었다. 그래서 "규칙 표에 없는 ID가 출력에 나온다"는 뜻의 어긋남은 없다. 다만 번호가 없는 단계 이름 일곱(`publish`·`sync`·`wait`·`LINT`·`install`·`migrate`·`packet`)은 `FullOps[<규칙>]` 자리에 규칙 ID가 아닌 글이 온다. 번호를 줄지는 정하지 않았다(`docs/formats.md` 7절에 목록이 있다).

### 7.3 미정과 사람 결정

- 미정(16절): M6(Jev의 전송 스키마)과 M7(산출물 인덱스 표의 열 문법)을 닫아 표에서 내렸다. M4에 묶음 H가 코드로 확인한 것과 남은 확인(실행)을, M9에 첫 측정의 바닥값을 적었다. M3(`--timeout-ms`의 최솟값과 0의 뜻)과 M12(`--skill *`, `agy plugin list --json`의 형태)에 남은 확인을 더했다.
- 사람 결정(17절): H14(패널 플러그인의 id·버전과 배포 자리), H15(`LICENSE`와 매니페스트의 `license`·`repository`), H16(`xtask dist`의 산출물과 카탈로그를 어느 브랜치에 두는가), H17(Windows의 프로세스 트리 종료를 Job Object로 할 것인가. unsafe 또는 `windows-sys`가 필요하다), H18(`migrate --rollback`이 `fullops.toml`을 `fullops.toml.rolled-back`으로 바꾸는 것)을 더했다. 모두 열림이다.
- 설계에 열린 문제로 적은 것 하나: 12.2절 J1의 둘째 경우(B7). lead의 X-01에서 병합으로 들어온 커밋을 뺄지 정해야 한다.

### 7.4 문서를 고치지 않고 코드 문제로 넘긴 것

구현이 설계의 의도와 어긋나 보여 설계 쪽을 구현에 맞추지 않은 것이다. 통합 담당이 고치거나, 고치지 않기로 하면 그때 설계를 바꾼다.

| # | 묶음 | 자리 | 문제 | 설계의 근거 |
|---|---|---|---|---|
| 1 | C | `fullops-core/src/flow.rs`의 `record_message` | core가 시스템 시계를 직접 읽는다(시그니처에 시각 인자가 없다) | core는 시계·파일·프로세스에 닿지 않는다 |
| 2 | C | `fullops-core/src/flow.rs` 563줄 | 본문의 SHA가 여럿일 때 조상 판정 없이 브랜치 tip과 같은 것을 고른다 | 8.6절: 나머지를 모두 조상으로 가지는 하나 |
| 3 | B·C·D | `hook/facts_real.rs`, `cmd/done.rs`, `cmd/lint.rs`의 `default_start` | `changed()`·`pass_covers`·시작점 계산이 세 곳에 따로 있다 | 6.6절: X-01과 `done`이 같은 함수를 쓴다 |
| 4 | E | `cmd/review.rs` 562줄 | check 9번의 `config_sha256` 비교가 없다(D가 정의한 값으로 계산할 수 있다) | 8.9절 check 9번 |
| 5 | E | `cmd/merge.rs` 309줄 | lead가 coordinator 브랜치에 있으면 리뷰 파일이 병합 기록 커밋에 들어가지 않는다 | 8.8절 M-08 |
| 6 | C | `cmd/status.rs` 71줄 | `status`가 `landed_at`을 쓸 때 `drift`를 세지 않는다(`settle`과 같은 일을 하지 않는다) | 8.8절 끝 문단 |
| 7 | B | `fullops-core/src/rules/stop.rs` 162줄 | X-04가 `logs/`의 r0~r9만 본다 | 7.5절 X-04 |
| 8 | D | `cmd/doc.rs` 315줄 | `doc stamp`의 `updated`가 UTC 날짜다 | 8.12절: `updated`를 오늘로 |
| 9 | D·E | `cmd/lint.rs` 303줄 | `lint`의 깨끗함 검사에 기록 경로 예외가 없어, 미커밋 리뷰 파일이 있는 기본 워크트리에서 M-07의 lint가 거절된다(병합 결과의 tree가 다를 때) | 8.8절 M-07·M-08 |
| 10 | C·J | `cmd/find.rs`의 `jev_curl`, `fullops-ext/src/jev.rs`의 `unsafe_text` | `find`·`context`가 `JevCurl::default()`를 불러 `--env-file`로 준 키가 실제 호출에 닿지 않는다. 키 찾는 순서와 본문 글 검사가 두 곳에 따로 있다 | 9.3절: 키를 찾는 순서는 `--env-file`이 먼저. 구현은 `fullops-ext::jev` 하나 |
| 11 | D·F | `crates/fullops/assets/repo/docs/deliverables/README.md` | 내장 인덱스 템플릿에 front matter가 없어, 인덱스의 상태 칸을 고친 커밋이 DOC-003에 걸린다 | 8.11절 DOC-003, 8.12절(인덱스 상태는 사람이 고친다) |
| 12 | F | `crates/fullops/assets/repo/lint/lint.json` | 버린 기능의 `.fullops-squad/board/**` exclude가 남아 있다 | 17절 H6 |

## 8. 검증에서 드러난 것과 고친 방식 (2026-11-14~15)

여덟 묶음을 합친 뒤 통합 담당 다섯(I1 hook, I2 설치·비교, I3 탐색, I4 hook 쪽 확인, I5 흐름·리뷰·lint)이 묶음마다 따로 구현한 곳을 하나로 모으고 `pending` 단계를 풀었다. 그 뒤 검증자 여섯이 실제 바이너리로 결함을 찾았고(R1~R6), 영역별 수정 담당이 고치고 있다. 이 절은 (1) 통합이 정한 것, (2) 7.4절의 코드 문제 12건이 어떻게 됐는가, (3) 안내 문구·스킬·문서 영역(F6)이 고친 것, (4) 결함마다 고친 방식(다섯 영역의 수정, 흐름 쪽 다섯 바퀴, 비교 반영과 그 반박 확인)을 적는다. 검증에서 드러난 설계의 구멍과 그 상태는 `docs/design.md` 16.1절에, 남은 사람 결정은 17절 H19~H28에 있다.

### 8.1 통합이 정한 것

| # | 무엇을 | 어떻게 정했나 | 절 |
|---|---|---|---|
| 1 | 변경 계산이 세 곳에 따로 있었다 | `fullops/src/hook/changes.rs` 하나로 모았다(`GitGate`, `start_candidate`, `start_point`, `merged_in`, `changed`, `pass_covers`). hook은 호출 상한을 준 창구로, `done`과 `lint`는 상한 없는 창구로 부른다 | 6.6, 10.2 |
| 2 | `changed()`의 코드 파일 판정 기준 | 시작점 커밋의 `[code]`와 `lint.json`의 `exclude`. hook이 상한 때문에 그 blob을 읽지 못하면 작업 트리의 설정. 대상 후보가 없으면 blob을 읽지 않는다(7.1번 표의 B 행 서술을 뒤집었다. 검증 보고 R1-10) | 6.6 |
| 3 | 시작점 후보가 없을 때 | hook은 판정하지 않고(Unknown, 통과) CLI(`done`, `lint`)는 바닥을 쓴다. 후보가 `HEAD`와 같으면 바닥을 계산하지 않는다 | 6.6 |
| 4 | 12.2절 J1의 둘째 경우(로컬 기본 브랜치에만 병합. 7.3절에 열린 문제로 적었던 것) | lead의 `changed()`가 통합 위치 `PushLeft`인 통합 대기 보고의 SHA가 바꾼 파일을 뺀다. 그 경우의 차단은 X-06이다. 파일 단위라 lead가 같은 파일을 병합 뒤에 고친 변경은 놓친다(검증 보고 R1-7로 다시 올라왔다) | 6.6, 7.5 |
| 5 | X-05 (a)와 X-06의 순서 | lead의 Run에서 통합 위치가 `Landed`인 통합 대기 보고의 메시지는 미처리로 세지 않는다. 미처리 메시지가 있는 통합 대기 보고는 첫 Stop이 X-05 (a), 다음 Stop이 X-06이다 | 7.5 |
| 6 | S-04의 효과 | 프로세스를 띄우지 않고 hook 프로세스 안에서 `cmd::runtime_stage`를 부른다. 프로세스 방식은 다른 빌드가 도는 PC에서 SessionStart가 0.9~1.9초 걸렸고, 플러그인 루트에 이 OS의 바이너리가 없으면 조용히 실패했다 | 7.5, 10.2 |
| 7 | `record_message`의 시그니처 | `(state, run, msg, at, resolve, branch_tip, is_ancestor)`. core가 시계를 읽지 않고, 본문의 SHA가 여럿이면 나머지를 모두 조상으로 가지는 하나를 고른다 | 8.6, 10.2, formats 8 |
| 8 | `config_sha256` 계산 | `fullops_core::lint_rules::config_sha256` 하나. `review check` 9번이 리뷰 base 시점의 `lint.json` blob으로 비교한다 | 8.9, 8.11, 10.2 |
| 9 | lint의 깨끗함 검사 | 기록 경로의 미커밋 변경은 세지 않는다(M-03과 같은 예외). 미커밋 리뷰 파일이 있는 기본 워크트리에서 M-07의 lint가 거절되던 것을 고쳤다 | 8.11 |
| 10 | `status`가 통합이 확인된 보고를 닫을 때 | `settle`과 같은 `mark_landed`를 써서 `landed_at`과 `drift`를 함께 쓴다 | 8.8, 10.2 |
| 11 | Jev 호출의 키 찾기와 제공자 결정 | `JevCurlAt` 한 곳. `find`·`context`도 `JevCurl::at`으로 만든 `Judge`를 쓴다. 보내기 전 글 규칙은 `explore::sendable_text` 하나다. env 파일의 줄 형식을 적었다 | 9.3, 10.2 |
| 12 | `GitCli::blob_prefixes` | 크기를 먼저 묻고(`cat-file --batch-check`) 크기 합 16MiB마다 `cat-file --batch` 하나로 읽는다. P11의 '요청 1건'은 읽은 blob 수로 센다 | 10.2 |
| 13 | `proc::run`: 직접 띄운 프로세스가 끝났는데 자손이 파이프를 쥐고 있을 때 | 200ms만 더 읽고 `Exited(코드)`를 돌려준다(묶음 D는 제한 시간에 `TimedOut`으로 정했었다) | 10.2, formats 8 |
| 14 | 시나리오의 단계 키 `hook_args`·`df`·`py`, 흐름 동사의 `env`, `FULLOPS_TESTKIT_BIN_DIR` | 12.1절 '통합이 정한 것'과 formats 9.3·9.4절에 적었다. `xtask compare`는 단계 단위로 가리고 py 구동기가 끝까지 돌지 못한 시나리오를 실패로 센다 | 12.1, formats 9 |
| 15 | 내장 `lint/lint.json`의 exclude | 버린 기능의 `.fullops-squad/board/**`를 지웠다 | 11.3 |
| 16 | `install`의 인자 조합과 계획 | `--prune`·`--rollback`·`--remove-hooks`는 `--dry-run`·`--check`와 함께 주면 사용법 오류다. `--dry-run` 계획에 사용자 홈에 쓰는 파일(`deps.toml`, `install.json`, `skill-backups/`, `deps-state.json`)도 나온다. 스킬 백업에 실패한 묶음은 설치하지 않는다 | 10.3 |
| 17 | `migrate --rollback --dry-run` | 실행하지 않고 사용법을 낸다(되돌리기에는 미리 보기가 없다) | 13.3 |

통합 담당이 확인만 하고 코드를 설계대로 둔 것(열린 채 남은 것)은 `docs/design.md` 16.1절의 '통합에서 넘어온 채 열려 있는 것'에 있다.

### 8.2 7.4절의 코드 문제 12건

| # | 문제 | 지금 |
|---|---|---|
| 1 | `record_message`가 시계를 직접 읽는다 | 고쳤다(8.1의 7) |
| 2 | 본문의 SHA가 여럿일 때 조상 판정이 없다 | 고쳤다(8.1의 7) |
| 3 | 변경 계산이 세 곳에 따로 있다 | 고쳤다(8.1의 1) |
| 4 | `review check` 9번의 `config_sha256` 비교가 없다 | 고쳤다(8.1의 8) |
| 5 | lead가 coordinator 브랜치에 있으면 리뷰 파일이 병합 기록 커밋에 들어가지 않는다 | 남았다(`cmd/merge.rs`의 shortcut 주석) |
| 6 | `status`가 `drift`를 세지 않는다 | 고쳤다(8.1의 10) |
| 7 | X-04가 `logs/`의 r0~r9만 본다 | 남았다(`rules/stop.rs`의 shortcut 주석) |
| 8 | `doc stamp`의 `updated`가 UTC 날짜다 | 남았다(`cmd/doc.rs`의 shortcut 주석) |
| 9 | `lint`의 깨끗함 검사에 기록 경로 예외가 없다 | 고쳤다(8.1의 9) |
| 10 | `find`·`context`의 `--env-file` 키가 실제 호출에 닿지 않는다 | 고쳤다(8.1의 11) |
| 11 | 내장 산출물 인덱스 템플릿에 front matter가 없다 | 남았다 |
| 12 | 내장 `lint.json`에 `board/**` exclude가 남았다 | 고쳤다(8.1의 15) |

### 8.3 안내 문구·스킬·문서 영역(F6)이 고친 것

| 보고 | 무엇을 | 어떻게 | 절 |
|---|---|---|---|
| R6-3 | S-01의 역할 안내가 1.24.0보다 약했다 | `messages::hook::role_guide(guide, sub, split)`에 1.24.0 `BRIEF`·`PRODUCT_BRIEF`의 항목을 `fullops` 명령으로 되살렸다(묻는 기준, `ask`, `worker_done` 한 번과 종료 차단, 설계 보고 형식, tester의 제품 코드 금지, coordinator의 질문 위임과 묶음 승인, dev 주 담당의 범위 확인과 리뷰). `routing = "split"`이면 설계 좌석은 제품 기획자의 글을, coordinator와 worker는 책임 분리의 글을 받는다. 테스트 레벨 문구를 `TEST_BRIEF` 수준으로 되돌렸다. 넣지 않은 것: 현황판 단계(버린 기능), 병합 SHA는 리뷰한 SHA(M-04가 강제), 로그 재독 간격(`wait`가 흡수) | 7.7 |
| R6-4 | worker 질문에 답하는 길이 스킬과 안내에 없었다 | 스킬 `fullops-orca`에 "질문: lead가 전달한다" 절을 넣었다: `orca orchestration reply --id <메시지 ID> --body "<답>"`(계약 `docs/contracts/orca.md` 6절의 꼴), 설계·범위 질문은 새 키의 설계 과제로 설계 역할에 넘김, 사용자 결정은 `decision add --blocks <KEY> --blocks <메시지 ID>`. coordinator의 역할 안내에도 `reply` 명령을 넣었다. `wait`의 다음 안내 문구는 흐름 담당의 파일이라 고치지 않았다 | 7.7, 11.3 |
| R6-5 | caveman 주입이 비활성 레포에서 사라졌다 | 레포 주인이 1.24.0의 동작을 유지하기로 정했다. hook 진입이 표식이 없는 레포의 SessionStart에서 `rules::start_skill`(S-06의 조건과 읽기)만 부르고 caveman 글만 낸다. 그 밖의 이벤트는 그대로 아무것도 읽지 않는다 | 7.1, 7.5, 7.8, 11.1, 12.3 DF-12 |
| R6-8 | lint 기록이 없는 보고의 알림이 까닭을 `--no-checks`로 단정했다 | "lint 통과 기록이 없는 보고다(코드 변경이 없거나 `--no-checks`). 병합 때 `fullops merge`가 확인한다"로 바꾸고 실패 보고에는 내지 않는다. 문서만 바꾼 보고와 `--no-checks` 보고를 가르지는 않았다(`reported`에 사유 필드가 필요하다) | 8.6 |
| R6-9, R1-10 | 설계와 형식 문서에 통합 전 상태가 남아 있었다 | 8.1의 표대로 고쳤다. `fullops env link`를 스킬 `fullops-orca`의 bootstrap 절에 넣었다 | 6.6, 8.6, 10.2, 18.4 |
| R6-2(스킬 몫) | 설계 완료 뒤 구현 인박스를 배정하는 절차가 스킬에 없었다 | 스킬 `fullops-orca`에 "설계 완료 뒤" 절을 넣었다(병합 → 들어온 지시서마다 `dispatch <KEY> --role <역할> --agent … --model … --effort …`, tester는 구현 병합 뒤). `merge`와 Stop의 안내는 흐름·hook 담당의 몫이다 | 11.3 |

스킬 문서가 말하는 `fullops` 명령과 옵션은 인자 해석기와 다시 대조했다(없는 명령·옵션 0건). `crates/fullops/tests/bundle_f.rs`의 `skill_docs_name_real_commands_and_skills`가 옵션 이름까지 본다.

### 8.4 결함별 고친 방식

심각도는 검증 보고(R1~R6)와 비교 반박 확인의 판정이다. 바퀴마다 뒤집힌 결정은 마지막 것만 적고 '뒤집힘'에 한 줄로 이유를 남겼다. 지금 코드가 정답이다.

| 결함 | 심각도 | 고친 방식 | 반영한 절 |
|---|---|---|---|
| R1-1 `lint --from HEAD`의 빈 통과 기록을 M-05·`review check`가 받음 | high | `pass_covers`가 `pass.base`가 요구 시작점과 다르면 설정 지문도 본다. M-05의 `pass.json` 길과 `done`의 다시 실행 길이 시작점과 함께 부른다. check 9번에 'lint의 `merge_base`가 리뷰 base이거나 그 조상' | 설계 6.6, 8.8 M-05, 8.9 9번 |
| R1-2 `.gitattributes`·diff 설정이 추가 줄 검사를 끔 | high | diff 계열을 한 함수로 모아 속성·외부 diff·textconv·접두어를 고정. 남는 통로에는 '옛 글과 새 글이 다르면 모든 줄이 추가 줄' | 설계 8.11 4번, 9.1, formats 8 |
| R1-3 `-diff` 속성이 리뷰 줄 수를 0으로 | high | numstat도 고정 인자, `FileStat.binary`, 줄 수를 세지 못한 코드 파일은 리뷰 필요 | 설계 8.9, 9.1 |
| R1-4 날것 `git merge` 뒤 `fullops merge`가 게이트를 건너뜀 | high | 병합 기록 커밋이 있을 때만 건너뛴다. 없으면 M-04·M-05를 다시 판정하고, 이미 원격에 있으면 `ungated`로 표시한다. `--continue`도 게이트를 본다 | 설계 8.8, formats 8 |
| R1-5 결정의 답에 사람 증거가 없음 | high | 새 거절 DC-04(결정을 연 뒤의 사람 턴), 결정과 `waiver.json`의 `human_turn`·`via`, `decided_by`는 실제 답한 사람 | 설계 6.2.1, 8.9 waive, 8.10, formats 3.5·4.1, 17 H22 |
| R1-6 구현자 세션을 모르면 같은 세션 리뷰가 통과 | high | snapshot이 구현자 세션을 정하지 못하면 RV-00, check 4번은 `state.json`의 snapshot 값과 견줌 | 설계 8.9, formats 4.1 |
| R1-7 lead의 X-01이 병합된 파일을 경로째 뺌 | medium | 내용이 병합 SHA와 같은 파일만 뺀다(`diff(sha, HEAD)`에 없는 경로) | 설계 6.6 4번 |
| R1-8 원격이 먼저 나아가면 `merge` 다시 실행이 멈춤 | medium | M-03이 통합 기준 ref를 합치고 lint, 충돌은 종료 코드 3, 실패는 `reset --keep`. 오프라인 push 실패 안내 | 설계 8.8 M-03·M-09, 17 H27 |
| R1-9 옛 글에 NUL이 있으면 lint 검사를 건너뜀 | medium | 옛 글을 읽지 못하면 새 파일처럼 검사 | 설계 8.11 4번 |
| R1-10 6.6절의 코드 파일 판정 기준 문장이 코드와 반대 | low | 문서만 고침(F6) | 설계 6.6 |
| R2-1 커밋된 `remote`·`base`가 git 옵션으로 읽힘(임의 명령 실행) | critical | 설정 검증(원격 이름의 꼴, `check-ref-format`), git·Orca·`deps.toml` 값의 `-` 시작 거절과 `--`, 등록된 원격만, 온전한 refspec | 설계 6.1.1, 9.1, 9.2, 10.7, formats 3.1·8, 17 H24 |
| R2-2 Windows 별칭 이름으로 비밀 파일이 Jev로 감 | high | 끝 점·공백을 떼고 `:` 조각 거절, 실제 이름으로 다시 검사 | 설계 8.14 |
| R2-3 글 규칙이 흔한 비밀 꼴을 놓침 | high | 이름·접미사·따옴표·토큰 꼴을 넓히고 경로 목록을 더함. 색인 `parser`를 `map-v2`로 | 설계 6.3, 8.14, formats 5.5, 17 H23 |
| R2-4 `route --request-file`이 레포 밖 파일을 읽음 | high | 레포 안이고 경로 규칙을 통과한 파일만, 레포 상대 경로로 기록 | 설계 6.1.4, 8.3, formats 3.4 |
| R2-5 필수 문서가 없으면 J-12가 배정을 계속 막음 | medium | 만들 때 읽은 항목만 '없음'으로 건다 | 설계 8.14 J-12, 12.2 P14 |
| R2-6 `.env`의 BOM이 키를 가림 | low | BOM을 뗀다 | 설계 9.3 |
| R2-7 PATH의 상대 항목에서 레포 안 실행 파일이 돎 | high | `proc::find_program`이 절대 경로 항목만 보고 `proc::run`이 전체 경로로 띄운다 | 설계 9.3, formats 8, 17 H24 |
| R2-8 `route`에 `--env-file`이 없음 | low | 더했다 | 설계 8.3, formats 8 |
| R3-1 키 없는 보고가 있으면 X-06이 풀리지 않음 | high | 문구에 메시지 ID, 탈출구에 `completion set-sha … --key`와 `hold --completion <ID>`, `hold`가 KEY 없이 받음 | 설계 7.5 X-06, 8.8, formats 8 |
| R3-2 대소문자·junction 경로가 T-05·T-06·T-10을 지남 | high | `Facts::real_path`로 실제 경로를 풀고 규칙 이름을 정본 표기로 | 설계 7.4, 10.2, formats 8 |
| R3-3 셸 분류가 감싼 명령을 못 풂 | high | 괄호·중괄호·백틱 구분자, 큰따옴표 안 치환, 제어 키워드·앞붙이 명령, 세 단계 중첩, `eval`·`iex`·`Start-Process`, `.ps1`. 남은 한계를 적었다 | 설계 7.4 2·3·4·5·7번, 17 H5 |
| R3-4 `commit -m<글>`을 `-a`로 읽음 | low | 값을 받는 옵션 앞의 `a`만 센다 | 설계 7.4 9번 |
| R3-5 `Purpose : research`를 못 읽음 | low | `Purpose\s*[:=]\s*`, T-04와 같은 함수 | 설계 7.5 T-08 |
| R3-6 agy의 stdin 기한이 등록 타임아웃보다 김 | low | `--event`가 있으면 읽기 전부터 그 이벤트의 기한 | 설계 7.1 |
| R4-1 병합 전 수정 배정이 막힘(탈출구가 서로를 가리킴) | high | 병합 전 `task reopen`(`unmerged_record`, 재오픈 준비 커밋, 거절 셋), lead의 reopen은 class와 무관하게 H-02 통과, `fix_round` 하나가 RT-02·M-04·M-07·review의 탈출구를 만든다. 뒤집힘: 반려된 설계 과제를 '설계 역할에 다시 보낸다'에서 '구현 역할의 수정 차수는 lead가 연다(1.24.0과 같다), 설계 역할 자신의 보고가 반려되면 `route --force --request`로 다시 맡긴다'로 바꿨다. 설계 역할은 코드를 고치지 않기 때문이다(T-07) | 설계 8.3, 8.4, 8.5, 8.8, 8.9 |
| R4-1 이어서: spec의 반려 정보 | - | 뒤집힘: spec에 `Rejected SHA`·`Review result`를 넣었다가 지웠다. 형식을 늘리지 않고 반려 사유를 요청 글 하나로 전한다 | 설계 8.5 |
| R4-1 이어서: `done` 뒤 실패한 차수 | - | 뒤집힘: reopen을 허용하는 안 대신 '그 차수의 완료 기록을 지시서로 다시 보내기'를 골랐다. 보통 실패와 같은 재시도이고 기록을 다시 쓰지 않는다 | 설계 8.5 D-02 |
| R4-2 미리 보기 뒤 `wait`가 병합한 보고를 되살림 | high | `Landed` SHA의 dispatch 없는 보고를 통합 대기로 되살리지 않는다 | 설계 8.6 |
| R4-3 같은 키를 다시 보고하면 `merge <KEY>`가 M-02 | medium | `--completion <가장 늦은 ID>` 안내, M-10이 이미 `Landed`인 앞선 보고도 닫는다 | 설계 8.6, 8.8 M-10 |
| R4-4 성공한 `dispatch`를 다시 실행하면 worker가 하나 더 뜸 | high | D-10 `same`(Build·Fix를 한 배정으로, `--reason limit`만 예외), `task recover`가 끊긴 배정을 `lost`로 | 설계 8.5, 17 H21 |
| R4-5 인박스 front matter 오류면 `done`이 검사를 건너뜀 | medium | R-04 `syntax`로 거절(`--failed`는 보낸다) | 설계 8.7 |
| R4-6·R4-7·R4-8 CRLF, BOM, 모르는 enum 값 | low | 줄 끝 보존, BOM 떼기, 타입 오류는 옮기지 않고 '새 형식'으로 거절 | 설계 6.5, formats 4.1, 17 H19 |
| R5-1 `migrate` 중간 실패가 두 판의 표식을 남김 | high | 쓰기 전에 모두 계산하고, `fullops.toml`을 `git mv` 바로 앞에 쓰고, 실패하면 기록대로 되돌리고, 끝나지 않은 전환을 알아본다 | 설계 13.2, formats 2 |
| R5-2 `install --rollback`을 S-04가 바로 다시 올림 | medium | `install.json`의 `pinned`, S-04 조건, `doctor` 알림 | 설계 6.4, 7.5 S-04, 10.3, formats 6 |
| R5-3 `migrate`가 심볼릭 링크 엔트리 문서를 일반 파일로 바꿈 | medium | 링크면 쓰지 않고 보고, 표식이 링크면 거절 | 설계 13.2 |
| R5-4 전환 뒤 기본 템플릿이 Python 명령을 안내 | medium | 1.24.0 기본 글 12개를 front matter·모드 블록을 뺀 본문으로 판별해 바꾸고, 고친 문서는 Python 명령 줄을 보고 | 설계 13.2, 17 H25 |
| R5-5 Python판 레포에서 `init`이 됨 | high | `FullOps[init]` | 설계 8.1, 10.6, formats 7 |
| R5-6 끝 표식 없는 블록에서 `init`이 글을 지움 | high | `block_span` 하나, 깨진 블록은 고치지 않음 | 설계 10.6 |
| R5-7·R5-9 되돌린 뒤 닫힌 결정이 남음, 결정 번호 겹침 | medium | 지난 사본과 견줘 닫힌 것을 뺌, 닫힌 결정까지의 최대 번호 + 1 | 설계 13.2 |
| R5-8 `--dry-run`에 `PLANS.md` 문법 오류가 없음 | low | 검사를 쓰기 전 점검으로 옮김 | 설계 13.2 |
| R5-10 되돌린 엔트리 문서의 바이트가 다름 | low | `migrate.json` v 2에 전환 전 글 전문과 쓴 글의 sha, 그대로면 바이트 복원 | 설계 13.3, formats 2, 17 H25 |
| R6-1 `route --request` 글이 저장되지 않음 | high | `routes/<KEY>.request.md`(비밀 무늬면 공용 디렉터리)에 저장해 `request_file`에 적고 준비 커밋에 넣음. 옛 기록은 D-02 `request` | 설계 6.1.4, 8.3, 8.5, formats 2·3.4 |
| R6-2 설계 완료 뒤 구현 지시서 배정 안내가 없음 | high | `flow::leftover`의 '배정되지 않은 지시서'를 X-07·`status`·`wait`·`merge`가 낸다(차수 인식). 스킬은 F6이 고쳤다. 뒤집힘: 안내를 '`task new` → `dispatch`'로 적었다가 `dispatch`만으로 바꿨다(설계 과제의 지시서는 H-02 때문에 설계 역할만 쓴다) | 설계 6.6, 7.5 X-07, 8.5, 17 H20 |
| R6-3·R6-4·R6-5·R6-8·R6-9 안내 문구·스킬·문서 | medium~low | 8.3절 표. R6-4의 `wait` 안내에 `reply` 절차를 붙이는 일은 남았다 | 8.3절, 설계 8.6 |
| R6-6 lead가 worker의 하위 Run을 가로챔 | medium | `wait --run`이 남의 Run을 `FullOps[wait]`로 거절, `status`는 lead 소유만, 정산된 전임 worker의 Run은 다시 배정된 worker가 이어받음 | 설계 8.6, 8.1 |
| R6-7 실패 보고를 처리하면 과제가 '보고됨'으로 남음 | high | `derive::last_failed`(차수 인식. `dispatches[].round`), `ready()` 4번과 '실패한 채 남은 과제' 안내 | 설계 6.2.1, 6.6, formats 4.1 |
| (흐름 바퀴) 설계 역할 재배정 뒤 `review check`가 RV-09 | medium | 설계 역할 좌석의 `lint --from` 기본은 인박스 base, 없으면 바닥(anchor를 쓰지 않음) | 설계 8.11 1번 |
| (흐름 바퀴) escalation이 끝난 dispatch를 실패처럼 보이게 함 | medium | 이미 `done`인 정산은 바꾸지 않고 `limit_escalated`만 | 설계 8.6, formats 4.1 |
| (흐름 바퀴) `Unknown`을 '없음'으로 내려 게이트를 건너뜀(`done`, `packet check`, `review`, `route`) | high | 읽지 못하면 거절(R-04 `syntax`, `FullOps[packet]`, 사용법 오류) | 설계 8.1, 8.3, 8.7, formats 7 |
| 비교: f1 `commit -am`이 미추적 캡처로 T-09 거부 | medium | `GitAdd { paths, update }`와 `CommitScope`로 git이 담을 항목만 본다 | 설계 7.4 9번, 7.5 T-09, 10.2, formats 8 |
| 비교 반박: 하위 폴더 cwd의 `add -A`·`$D` 등이 T-09를 지남(위 수정의 회귀) | high | 레포 전체 표식을 `.`에서 `:/`로 바꾸고 `stage`, `--pathspec-from-file`, 풀리지 않은 확장, 경로 없는 add를 레포 전체로 본다 | 설계 7.4 9번, 7.5 T-09 |
| 비교 반박: 날것 `orchestration dispatch`(`--inject` 없음)가 분류 안 된 Task를 배정 | medium | `OrcaWorkerStart`로 분류해 T-01(`--dry-run`만 통과). DF-28의 근거를 고침 | 설계 7.4 8번, 7.5 T-01, 11.1, 12.2 C5, 12.3 DF-1·DF-28, contracts/orca.md 2.9 |
| 비교 반박: py 구동기의 `fullops <sub>` 지름길이 1.24.0 판정을 가림, 판정이 같은데 붙은 `df` | low | 지름길을 지우고 a3·a4 6단계에 DF-26, p2 30·r4 14의 `df`를 지우고 r4 18을 DF-33으로 | 설계 16.1(비교), 12.2 A3·A4 |

## 9. 2.1.0: 패킷 기능을 뗐다 (2026-11-17)

레포 주인이 정했다. 이유는 둘이다. `<KEY>-packet.json`은 과제당 1~4MB이고 worker는 항목마다 `<KEY>-packet-outcomes.json`을 써야 해서 토큰만 쓴다. 그리고 새 판에는 패킷을 새로 켜는 길이 없었다(1.24.0의 `requires_packet`만 이어받았다). 설계 3절의 "탐색 패킷은 선택" 결정은 이것으로 거두었다.

### 9.1 뗀 것

| 무엇을 | 내용 | 절 |
|---|---|---|
| 명령 | `fullops packet build\|check`와 `cmd/packet.rs`, 명령 표의 줄 | 8.2, 8.14 |
| 검사 | `check_packet`(`PacketCheck::{Delivery, Completion}`)과 부르던 세 곳: `dispatch`의 D-12, `done`의 R-10, `review check`의 10번(`RV-10`). 번호는 다시 쓰지 않는다. `review check`의 항목은 1~9번이다 | 8.5, 8.7, 8.9 |
| 거절 ID | J-03과 J-10~J-17, 단계 이름 `FullOps[packet]`. J-01·J-02·J-04는 `find`·`context`의 것으로 남는다 | 8.1, 8.14 |
| 타입과 함수 | `Packet`·`PacketOutcomes`·`PacketCategory`·`PacketCheck`와 딸린 타입, `PacketInput`·`Source`·`SourceBasis`, `build_packet`·`build_packet_from`·`packet_block`·`with_block`·`packet_source_paths`·`source_sha256`, `parse_packet`·`parse_packet_outcomes`, `read_packet`·`read_packet_outcomes`, `rebuild_packet`, `Repo::packet_path`·`packet_outcomes_path` | 5, 10.2 |
| `Route.requires_packet` | 필드와, `route`가 1.24.0의 `docs/evaluations/jev/<KEY>-route.json`에서 그 값을 옮기던 코드, `migrate` 보고서의 "requires_packet이 켜진 옛 route 기록" 줄. 이미 커밋된 route 기록에 그 키가 남아 있어도 읽힌다(`Route`는 모르는 필드를 거절하지 않는다. 시험이 고정한다). 다음에 쓸 때 키가 사라진다 | 6.1.4, 8.3, 13.2 |
| `dispatch` | spec의 `; Packet: …` 필드, 준비 커밋에 `docs/evaluations/jev/` 파일을 넣던 것 | 8.5 |
| `task recover` | 패킷을 다시 만들고 `패킷 갱신` 커밋을 하던 단계. 출력과 JSON의 `packet` 필드 | 8.4 |
| 인박스의 패킷 블록 | 쓰는 코드(`--write-inbox`). `instruction_sha256`이 `<!-- fullops-packet:start -->`~`end` 블록을 빼고 계산하는 규칙은 남겼다: 2.0.x가 쓴 인박스가 남아 있을 때 지문과 과제 글이 달라지지 않게 한다 | 6.1.5 |
| 시나리오 | P1, P3~P6, P12, P14(파일도 지웠다). P2와 P13은 `find`·`context`만으로 고쳐 남겼다 | 12.2 |
| 스킬 | `fullops-work`의 패킷·처리 기록 안내 | 11.3 |

### 9.2 탐색 결과의 자리

`find`·`context`는 남는다. 결과 파일(`<KEY>-find.json`, `<KEY>-documents-find.json`, `<KEY>-context.json`)과 덮어쓰기 전 사본(`history/`)은 커밋되는 `.fullops-squad/docs/evaluations/jev/`가 아니라 Git 공용 디렉터리 `<git-common-dir>/fullops/explore/`에 쓰고 읽는다(`Repo::explore_dir`). 모든 워크트리가 같은 파일을 보고 커밋되지 않는다.

| 따라 바뀐 것 | 내용 | 절 |
|---|---|---|
| 준비 커밋 | 탐색 결과를 넣지 않는다. worker는 공용 디렉터리의 같은 파일을 본다 | 8.5 |
| 내장 `.gitignore` | `docs/evaluations/jev/history/` 줄을 뺐다. `assets::GITIGNORE_LINES`는 비었고 `migrate`가 `.gitignore`에 더하는 줄이 없다. `upgrade`의 필수 적용 `explore-history-ignore`도 뺐다 | 6.1, 10.6 |
| 출력의 경로 | 원본 체크아웃에서는 `.git/fullops/explore/<파일>`, 역할 워크트리에서는 절대 경로다(레포 밖이라 상대 경로로 줄이지 않는다) | 8.14 |
| 워크트리 사이의 재사용 | 결과가 워크트리마다 따로가 아니므로, lead 체크아웃에서 만든 결과가 있는 키를 역할 워크트리에서 다시 돌리면 `head`가 달라 J-04(`--force`)다 | 8.14 |
| 이미 커밋된 파일 | `.fullops-squad/docs/evaluations/jev/`의 옛 탐색 결과·패킷·처리 기록은 지우지 않고 읽지도 않는다. 레포의 `.gitignore`에 남은 `docs/evaluations/jev/history/` 줄은 해가 없다 | 6.1, 13.2 |

`.fullops-squad`라는 폴더 이름과 나머지 경로는 같은 판의 다음 변경(10절)에서 바꿨다.

## 10. 2.1.0: `.fullops` 자리와 문서 트리 (2026-11-17)

레포 주인이 정했다(명세 `docs/proposals/2.1.0-repo-folder-and-artifacts.md`). 근거는 jevpet 점검이다: `.fullops-squad/` 아래 커밋 파일 9,309개, `docs/evaluations` 약 850MB, 1.24.0의 기계 파일과 사람 문서가 한 곳에 섞여 있고, 과제 하나의 기록이 `handovers/logs/`, `routes/`, `reviews/`, `docs/evaluations/qa-reports/`, `docs/evaluations/jev/` 다섯 곳에 흩어져 있었다. 17절 H2의 "레포 폴더 `.fullops-squad/` 유지"를 뒤집었다. 유지한 이유(Python판과 같은 폴더로 공존)는 Python판 플러그인을 Claude Code·grok에서 지우면서 사라졌다.

### 10.1 바뀐 것

| 무엇을 | 내용 | 절 |
|---|---|---|
| 레포 폴더 | `.fullops-squad/` → `.fullops/`. 활성 표식은 `.fullops/fullops.toml` | 1, 4, 6.1, 17 H2 |
| 문서 트리 | `config/`(lint.json, review.json), `guide/`(FULLOPS.md, project.md, orca-agents.md, lint.md, rules/, roles/, agents/), `templates/`, `inbox/to_<역할>.md`, `tasks/<KEY>/{route.json, request.md, <역할>.r<n>.md, reviews/<head12>/, qa/}`, `docs/`(제품 산출물), `archive/`(1.24.0 기록), `local/` | 6.1 |
| 경로 모듈 | `fullops_core::layout`: `DIR`, `LEGACY_DIR`, 레포 상대 경로 함수, `is_record`, `REQUIRED_DOCS`, 옛 자리의 대응 `moved`·`rewrite`. `Repo::squad()`는 `Repo::dir()`이 됐고 경로 메서드는 `root.join(layout::…)`만 한다. src의 다른 곳은 `.fullops` 문자열을 만들지 않는다 | 6.1, 10.2, formats 2절 |
| 흐름 기록 | `derive.rs`·`hold.rs`·`review.rs`가 따로 갖던 목록(`handovers/`, `docs/evaluations/`, `routes/`, `reviews/`, `PLANS.md`)을 `layout::is_record`(`PLANS.md`, `inbox/`, `tasks/`) 하나로 바꿨다. 리뷰 대상의 '기록이라 열람하지 않는다'도 같은 판정이다 | 6.6, 8.9 |
| hook 규칙 | T-05·T-06은 `inbox/` 바로 아래 `.md`(템플릿 예외가 없어졌다), T-09는 `tasks/` 아래 커밋 후보 가운데 `tasks/<KEY>/qa/`의 미디어와 그 폴더의 `report.md`, T-10은 차수 이름의 새 과제 폴더(`tasks/<KEY>-FIX/qa/…`인데 `tasks/<KEY>/`가 있다), `terminal send`의 주입 판정은 글 안의 `inbox/to_` | 7.4, 7.5 |
| QA 보고서 | `docs/evaluations/qa-reports/<KEY>-test/report.md` → `tasks/<KEY>/qa/report.md`. `done`의 R-05, tester 안내, 스킬이 따른다 | 8.7, 11.3 |
| 옛 자리 레포 | 비활성. SessionStart의 안내, 명령의 `FullOps[layout]` 거절(`status`·`upgrade`는 안내와 종료 코드 0), `doctor`의 행. 판정은 `fullops_store::legacy_marker` | 7.1, 13.6 |
| `migrate --layout [--dry-run]` | 거절 조건, 표대로 옮기기, 추적 해제와 `.fullops/local/legacy/`, 글 고치기, exclude 보강, `.gitignore`, `applied_version`, 커밋 하나 | 13.6 |
| `migrate --from-python` | 옛 자리에서 전환한 뒤 전환을 커밋하고 같은 실행에서 자리 옮김을 한다(커밋 둘). 없는 템플릿은 옛 자리의 경로로 더한다 | 13.2 |
| `migrate --rollback` | 옛 자리에서만 한다. 새 자리에서는 거절하고 자리 옮김 커밋의 `git revert`를 안내한다 | 13.3 |
| `migrate --finalize` | 표식과 `git rm` 안내의 경로가 새 자리를 본다 | 13.5 |
| lint | DOC-002·DOC-003의 대상에서 `archive/`와 `tasks/*/reviews/`를 뺐다. 새 규칙 QA-001(ERROR), QA-002(WARNING) | 8.11 |
| `doctor` | `.fullops/` 아래의 '무시 목록에 있으면서 추적되는 파일' 알림 | 10.6 |
| `init` | 새 트리를 만든다. 레포 루트가 사용자 홈이면 거절한다 | 10.6 |
| 탐색 | 문서 색인에서 `archive/`를 뺐다. 필수 문서 여섯의 경로가 `guide/` 아래다 | 8.14 |
| 내장 템플릿 | `crates/fullops/assets/repo/`를 새 트리로 옮기고 글의 경로를 고쳤다. `lint.json`의 `exclude`는 `inbox/`·`tasks/`·`archive/`·`guide/roles/`·`docs/evaluations/`, `review.json`의 `exclude`는 `tasks/`·`archive/`·`*.bak`. `.gitignore`는 `.env*`·`!.env.example`·`local/`·`**/raw/` | 6.1, 11.3 |
| 스킬·관리 블록·패널 | `plugin/skills/**`와 `managed-block.md`의 경로, Orca 패널의 활성 표식 경로와 판 번호 2.1.0, npx 실행기의 표식 판정 | 10.4, 11.3 |

### 10.2 명세와 다르게 정한 것

| 무엇을 | 명세 | 구현 | 이유 |
|---|---|---|---|
| `legacy_marker`의 자리 | `layout::legacy_marker(root)` | `fullops_store::legacy_marker(root)`(와 `legacy_root(cwd)`) | `fullops-core`는 파일에 닿지 않는다(10.1절). 폴더 이름은 `layout`의 상수를 쓴다 |
| 옛 자리 레포의 `status`·`upgrade` | 거절에서 빼는 명령 | 안내를 내고 종료 코드 0 | 활성 표식 없이는 요약도 필수 적용 검사도 못 한다. 세션 시작의 `fullops status`와 npx의 `upgrade --repo`가 실패로 보이지 않게 한다 |
| 파일 옮기기 | 추적 파일을 `git mv` | 파일 이동과 `git update-index --index-info` 한 번 | 결과가 같다. 파일 9천 개에 `git mv`를 한 번씩 부르면 Windows에서 여러 분이 걸리고 인덱스를 그만큼 다시 쓴다 |
| 추적에서 빼는 범위 | jev, OCR 중간 파일, 무시 목록에 있으면서 추적되는 파일 | 그것과 새 `.gitignore` 템플릿에 걸리는 자리로 갈 파일(`raw/` 아래 등) | 옮긴 직후 `doctor`가 바로 알릴 파일을 만들지 않는다 |
| 추적되지 않는 파일 | `.env*`·`local/`을 옮긴다 | 그 둘은 같은 이름으로, 나머지(옛 무시 목록에 걸려 있던 파일)는 `local/legacy/`로 | 옛 폴더를 남기지 않는다. 새 자리의 무시 목록에 걸리지 않을 파일이 작업 트리를 더럽히지 않는다 |
| exclude 보강 | `config/review.json` | `config/lint.json`에도 같은 둘을 더한다 | 2.0.x의 템플릿은 `handovers/**`·`docs/evaluations/**`로 끝난 지시서와 QA 폴더를 코드 lint에서 뺐다. 그 자리가 `tasks/`로 갔다 |
| 글 고치기의 엔트리 문서 | 관리 블록 안의 경로 치환 | 관리 블록을 새 블록으로 바꾼다 | 블록은 fullops가 관리하는 글이고 새 블록이 새 경로를 담는다 |
| `--from-python`의 커밋 | 13.2절은 커밋하지 않았다 | 전환 커밋과 자리 옮김 커밋 둘 | 자리 옮김이 깨끗한 작업 트리에서 커밋 하나를 만드는 단계다. 커밋을 나눠야 자리 옮김만 revert한 뒤 기존 `--rollback`이 그대로 된다 |
| `handovers/` 아래 그 밖의 파일 | 표에 없다 | `archive/handovers/` | 인박스(`to_*.md`)와 틀이 아닌 것은 옛 기록이다 |
| T-10 | 언급 없다 | 차수 이름의 새 과제 폴더로 옮겨 남겼다 | 같은 과제의 QA를 차수마다 새 폴더로 흩는 것을 막는 규칙이다. 폴더의 자리만 바뀌었다 |

### 10.3 남은 것

- 사용자 레포의 안내 문서(`guide/**`)에 접두어 없이 적힌 상대 경로(`handovers/_TEMPLATE.md`, `lint/lint.json` 등)는 `migrate --layout`이 고치지 않는다. 사람이 고치거나 새 템플릿의 글을 가져온다.
- QA 보고서가 역할별 파일(`tasks/<KEY>/qa/<역할>.md`)인 과제에서 T-09는 `report.md`만 링크의 근거로 본다. 그런 폴더에 미디어를 더 커밋하면 `report.md`에 링크를 적어야 한다.
- `scenarios/drivers/py_driver.py`는 시나리오의 새 자리 경로를 1.24.0의 자리로 옮겨 읽는다(`old_place`). 기준선 사본이 없는 환경이라 `cargo xtask compare`로 다시 재지 않았다.
- `Cargo.toml`·`package.json`의 판 번호, 릴리스 노트, 카탈로그는 따로 올린다.
