# 설계 개정 기록 (2026-11-14)

- 대상: `docs/design.md`.
- 반영한 것: (1) 계약 확인 결과 `docs/contracts/design-delta.md`(Claude Code 2.1.323, Codex 0.163.0, Orca 1.4.256). (2) 레포 주인의 결정(2026-11-13~14): 이름과 버전 확정, 이슈 자동 모드 미룸 확정, Jev 탐색을 v1에, 의존 플러그인·스킬 설치를 v1에, 네 호스트(Claude Code·Codex·grok·agy) 모두에 설치, 그 밖의 버리는 기능 확정.
- 읽는 순서: 뼈대(묶음 A) 담당은 0절만 보고 고치면 된다. 1절은 절 번호 순서의 전체 목록이다. 2절은 계약 확인이 "결정 필요"로 남긴 것을 어떻게 정했는지다. 4절은 개정본을 다른 모델이 점검해 낸 지적 19건을 어떻게 처리했는지다(그 결과는 0~1절에 이미 합쳐져 있다).
- 표기: "절"은 `docs/design.md`의 절 번호다.

## 0. 뼈대가 따라 바꿔야 하는 것

개정 전 설계로 만든 타입·trait·상태 형식·규칙 표·CLI 표면 가운데 바뀐 것만 적는다. 여기에 없는 것은 그대로다.

### 0.1 타입 (5절, 7.2절, 7.4절)

| 타입 | 바뀐 내용 | 절 |
|---|---|---|
| `HostId` (새로) | `enum HostId { Claude, Codex, Grok, Agy }`. 문자열은 `claude`·`codex`·`grok`·`agy` | 5 |
| `Route` | 필드 `requires_packet: bool` 추가. serde 기본값 `false` | 5, 6.1.4 |
| `PacketCategory` (새로) | `DirectEdit`, `ImpactCheck`, `DocumentRead`, `DocumentUpdate`. 문자열은 `direct_edit`·`impact_check`·`document_read`·`document_update` | 5, 6.1.5 |
| `PacketCheck` (새로) | `Delivery`, `Completion` | 5, 8.14 |
| `Packet`, `PacketOutcomes` (새로) | 6.1.5절의 `packet.json`·`packet-outcomes.json` 필드 그대로 | 5, 6.1.5 |
| `Deps` (새로) | 10.7절의 `deps.toml` 필드 그대로 | 5, 10.7 |
| `HookInput.source` | 값에 `fork` 추가(주석만 바뀐다) | 7.2 |
| `HookInput.stop_reason` | 유지한다(계약 확인은 지우라고 했다). grok만 `reason`을 준다 | 7.2 |
| `HookInput.error` | 뜻이 바뀐다: 오류 글의 앞 300자가 아니라 오류 **종류 이름**(`rate_limit` 등) | 7.2 |
| `HookInput.error_details` (새로) | `Option<String>`. `error_details`, 없으면 `last_assistant_message`의 앞 300자 | 7.2 |
| `HookInput.agent_id` (새로) | `Option<String>`. 읽기만 하고 판정에 쓰지 않는다 | 7.2 |
| `Intent::OrcaTaskCreate { spec }` | 타입은 그대로. `--spec-file`이면 `Known::Unknown` | 7.4 |
| `ToolKind` | 그대로. grok의 `run_terminal_command`→`Shell`, `search_replace`→`Edit`(또는 `Write`), `spawn_subagent`→`SubAgent` 대응이 생겼다 | 7.2 |
| `StartRequest` | `{ run, spec, worktree, target: StartTarget, ready_timeout, request }`. `task` 필드 없음 | 10.2 |
| `StartTarget` (새로) | `NewTerminal { agent, model: Option, effort: Option }` 또는 `Existing { terminal }` | 10.2 |
| `StartOutcome` (새로) | `Started(Started)`, `Failed(StartFailure)`, `Unknown(String)` | 10.2 |
| `Started` | `{ dispatch, task, run, terminal: Option<String>, state, run_created }`. `terminal`이 `Option`이 됐다 | 10.2 |
| `StartFailure` (새로) | `{ dispatch: Option, failed_stage, last_error, next, residual }` | 10.2 |
| `CheckMode` (새로) | `Peek`, `Consume`, `PeekWait { types, timeout }`, `AckLegacy(String)` | 10.2 |
| `CheckResult` (새로) | `{ messages, delivery_id: Option<String>, timed_out }` | 10.2 |
| `OrcaMessage` | `{ id, kind, from, subject, body, task_id, dispatch_id, outcome }`. `run_id`와 전달 ID 필드 없음 | 10.2 |
| `WorkerView` | `{ dispatch, state, stage: Option, outcome: Option, liveness(첫 낱말), terminal: Option }`와 `settled()` | 10.2 |
| `Question`, `QuestionKind`, `Answer`, `Judged`, `Usage`, `JudgeError` (새로) | 10.2절 그대로 | 10.2 |
| `Plan` (새로) | `{ per_host: BTreeMap<HostId, Result<Vec<Step>, Refusal>>, skills: Vec<Step> }`. 설치 계획. 호스트를 가로지른다 | 10.2, 10.3 |
| `Step`, `InstallStage`, `StepKind` (새로) | `Step { stage: InstallStage, kind: StepKind, argv, what, hosts: Vec<HostId> }`. `InstallStage`는 `Plugin`·`Deps`. `StepKind`는 `MarketAdd`·`MarketUpdate`·`PluginInstall`·`SelfInstall`·`SkillInstall` | 10.2 |
| `DepsState`, `HostInstall` (새로) | `DepsState { v, manifest_sha256, hosts: BTreeMap<HostId, HostInstall> }`, `HostInstall { at, self_version, complete, failed: Vec<String> }`. `deps-state.json`의 형식 | 10.2, 10.3 |
| `DepsError` (새로) | `Parse(String)`, `Schema(u32)`, `Invalid(String)` | 10.2 |
| `TreeEntry` (새로) | `{ mode: String, kind: String, oid: String, path: String }`. `Git::ls_tree`의 반환 | 10.2 |
| `DeliverableRow` (새로) | `{ id, title, summary, status, sources: Vec<String> }`. `parse_deliverable_index`의 반환 | 10.2 |
| `AckBy` (새로) | `Merge`, `Ack`. 메시지의 처리 표시를 누가 하는가 | 8.6, 10.2 |
| `Scope`, `IndexEntry` (새로) | `Scope`는 `Code`·`Documents`. `IndexEntry { path, blob, summary, headings, definitions, searchable, read_error }`는 6.3절 색인 `files[]`의 한 항목 | 6.3, 10.2 |
| `Signal`, `Decision` (새로) | `Signal { relevant, evidence, contradicts, injection: f64 }`. `Decision`은 `Keep`·`SuggestOmit`·`Conflict`·`Caution` | 8.14, 10.2 |
| `FindResult`, `ContextResult` (새로) | 6.1.5절의 `find.json`·`context.json` 필드 그대로 | 6.1.5, 10.2 |
| `PacketInput`, `PacketDecision`, `Source`, `SourceBasis` (새로) | `build_packet`의 입력. 필드는 10.2절 그대로. `Source`는 `Text { text, basis }`·`Unreadable(이유)` | 10.2 |
| `MessageNote` (새로) | `record_message`의 반환 | 10.2 |
| `SkillText` (새로) | `{ path: PathBuf, body: String }` | 10.2 |

### 0.2 trait과 함수 시그니처 (7.2절, 10.2절)

| 대상 | 바뀌기 전 | 바뀐 뒤 |
|---|---|---|
| `Host` | `decode`, `encode`, `caps`, `tool_kind` | `fn id(&self) -> HostId` 추가. 구현이 셋(`host_claude`, `host_codex`, `host_grok`) |
| `host_for` (새로) | - | `fn host_for(arg: &str, raw: &[u8]) -> Option<Box<dyn Host>>`. 인자가 `claude`여도 입력이 camelCase면 grok 어댑터 |
| `Facts` | - | `fn host(&self) -> HostId`(S-01의 pstack 문장과 S-06이 쓴다), `fn session_handle(&self) -> Option<String>`, `fn user_skill(&mut self, name: &str) -> Known<Option<SkillText>>` 추가 |
| `Orca::run_create` | `fn run_create(&self)` | `fn run_create(&self, objective: &str)` |
| `Orca::worker_start` | `-> Result<Started, ExtError>` | `-> Result<StartOutcome, ExtError>` |
| `Orca::check` | `fn check(&self, run: &RunId, peek: bool, wait: Option<Duration>) -> Result<Vec<OrcaMessage>, ExtError>` | `fn check(&self, run: Option<&RunId>, mode: CheckMode) -> Result<CheckResult, ExtError>` |
| `Orca::ack` | 있음 | **삭제** |
| `Judge::ask` | `-> Result<Vec<Answer>, JudgeError>` | `-> Result<Judged, JudgeError>` |
| `Git` | - | `ls_tree(rev)`, `blob_prefixes(oids, max)`, `grep_files(rev, literals)` 추가 |
| `record_completion` | `fn record_completion(state, msg, resolve, branch_tip) -> CompletionNote` | 이름과 시그니처 변경: `fn record_message(state, run: &RunId, msg, resolve, branch_tip) -> MessageNote`. 모든 종류의 메시지를 받는다 |
| `fullops-core::explore` (새로) | - | `instruction_sha256`, `source_sha256(text, is_instruction)`, `sendable_path`, `sendable_text`, `index_entry`, `batches`, `triage`, `packet_source_paths`, `build_packet`, `check_packet` |
| `check_packet` (새로) | - | 10.2절의 시그니처. 마지막 인자는 해시가 아니라 글을 돌려주는 `read_text: &mut dyn FnMut(&str) -> Known<Option<String>>`다(지문은 함수 안에서 `source_sha256`으로 구한다). 뼈대는 "항상 빈 목록"을 돌려주는 임시 구현을 둔다(C·E가 부르고 J가 채운다) |
| `rebuild_packet` (새로) | - | `fullops::cmd::packet`의 `fn rebuild_packet(repo, key, role, extra_seeds: &[String]) -> Result<Option<PathBuf>, Refusal>`. 뼈대는 "항상 `Ok(None)`" 임시 구현(C의 `task recover`가 부르고 J가 채운다) |
| `parse_packet`, `parse_packet_outcomes` (새로) | - | `fullops-core::model`. `fn parse_packet(bytes: &[u8]) -> Option<Packet>`(`v != 1`이면 `None`). 뼈대가 완성본을 둔다. `read_packet`과 리뷰 `check` 10번이 쓴다 |
| `orca_wakes`, `ack_by` (새로) | - | `fullops-core::model`. `fn orca_wakes(session_handle: Option<&str>, coordinator: Option<&str>) -> bool`, `fn ack_by(state: &State, message: &MessageId) -> AckBy`. 뼈대가 완성본을 둔다(X-05, `wait`, `dispatch`, `merge`가 쓴다) |
| `header_summary` (새로 드러냄) | - | `fn header_summary(text: &str, ext: &str) -> String`. 뼈대는 "항상 빈 글" 임시 구현(D가 채우고 J가 부른다) |
| `lint_excludes` (새로 드러냄) | - | `fullops-core::lint_rules`의 `fn lint_excludes(lint_json: Option<&str>, template_lint_json: &str) -> GlobSet`. 뼈대는 "항상 빈 `GlobSet`" 임시 구현(D가 채우고 B·C·E·J가 부른다) |
| `parse_deliverable_index` (새로 드러냄) | - | `fullops-core::lint_rules`의 `fn parse_deliverable_index(text: &str) -> Vec<DeliverableRow>`. 뼈대는 "항상 빈 목록" 임시 구현(D가 채우고 C·J가 부른다) |
| `fullops-core::deps` (새로) | - | `install_plan(deps, hosts: &[HostId], registered: &BTreeMap<HostId, Known<BTreeMap<String, String>>>, package_dir) -> Plan`과 데이터 `HOST_COMMANDS`. `Deps` 타입은 여기가 아니라 `model`에 있다. 뼈대는 빈 `Plan`을 돌려주는 임시 구현(F가 채운다) |
| `fullops-store` | - | `read_packet`, `read_packet_outcomes`, `write_explore_result`, `update_index`, `load_deps`(`deps.toml`의 TOML 해석과 필드 검증을 여기서 한다. 반환 오류는 `DepsError`), `read_deps_state`, `write_deps_state` 추가 |
| `fullops-ext::hosts` (새로) | - | `host_available`, `host_version`, `host_marketplaces`, `host_plugins`, `skill_roots`, `run_step`. `skill_roots`는 뼈대가 9.5절 표대로 완성본을 둔다(B의 S-06과 F가 함께 쓴다) |

### 0.3 상태와 파일 형식 (6절)

| 파일 | 바뀐 내용 | 절 |
|---|---|---|
| `routes/<KEY>.json` | `"requires_packet": false` 추가 | 6.1.4 |
| `docs/evaluations/jev/<KEY>-find.json`, `-documents-find.json`, `-context.json`, `-packet.json`, `-packet-outcomes.json` (새로) | 필드 단위 형식. 머리 필드 `v`·`key`·`role`·`round`·`head`·`instruction_sha256`·`options`·`input_sha256`·`created_at`. `find`·`context`의 `options`에 `provider`·`cache_bypass`, `packet`의 `options`에 `sources_sha256`·`producer_sha256`이 있다. 패킷 항목의 `source_sha256`은 지시서 파일만 지시서 지문이다 | 6.1.5 |
| `docs/evaluations/jev/history/` (새로) | `--force`와 낡은 형식 교체 때의 사본 자리. 커밋하지 않는다 | 6.1, 8.14 |
| 템플릿 `.fullops-squad/.gitignore` | `docs/evaluations/jev/history/` 줄 추가(`init`과 `migrate`가 넣는다) | 6.1, 13.2 |
| `state.json` `runs[]` | `"coordinator": "<세션 핸들>\|null"` 추가 | 6.2.1 |
| `state.json` `dispatches[]` | `"request": "req_<16진수 16자>"` 추가. `id`의 임시 값이 `"pending:<run>:<started_at>"`에서 `"pending:<request>"`로. `terminal`이 `null`일 수 있다 | 6.2.1 |
| `state.json` `messages[]` | `delivery`가 `null`일 수 있다(보통 `null`). 넣는 종류가 "worker_done·question·escalation"에서 "heartbeat·status가 아닌 모든 종류"로. 형식은 그대로이고 `acked_at`을 쓰는 곳이 넷으로 정해졌다(M-10, `hold`, `landed_at`을 쓰는 곳, `wait --ack`) | 6.2.1 |
| 세션 파일 | `host` 값에 `grok` 추가. `terminal`은 세션 핸들(`ORCA_AGENT_SESSION_ID` → `ORCA_TERMINAL_HANDLE`) | 6.3 |
| `<git-dir>/fullops/search/{code,documents}.json`, `search/lock` (새로) | 탐색 색인 | 6.3 |
| `~/.fullops/deps.toml`, `deps-state.json`, `skill-backups/` (새로) | 의존 목록 사본, 설치 기록, 스킬 백업 | 6.4, 10.3 |
| `~/.fullops/package/<버전>/` (새로) | 통합 패키지 폴더의 사본. agy에 넘기는 경로다 | 6.4, 10.3 |
| `~/.fullops/install.json` | 필드는 그대로. `source`의 뜻을 "통합 패키지 폴더의 절대 경로"로 못 박았다 | 6.4 |
| `deps.toml` (새로, 패키지) | 10.7절의 형식. 파서를 뼈대에 둔다 | 10.7 |

### 0.4 규칙 ID (7.5절, 8절)

| ID | 바뀐 내용 | 절 |
|---|---|---|
| S-01 | 대상 `source`에 `fork` 추가 | 7.5 |
| S-02 | 세션 파일에 `host`를 쓴다 | 7.5 |
| S-04 | 조건: 비교 대상이 "자기 버전"에서 "`install.json`의 `version`"으로. 효과: 실행본 단계만 | 7.5, 10.3 |
| **S-06 (새로)** | SessionStart, 강도 Context. caveman 스킬 적용 안내 | 7.5 |
| P-02 | 분류 패턴 셋(앞 공백을 뗀 뒤 비교, `──── From:` 추가) | 7.5 |
| P-03 | 강도가 `Warn`에서 `Context`로 | 7.5 |
| T-00 | 통과하는 하위 명령에 `install` 추가 | 7.5 |
| T-03 | 탈출구 문구 변경(`orca orchestration send --to dispatch:<ID>`) | 7.5 |
| T-04 | "모름" 칸에 `--spec-file` 추가. 탈출구 문구 | 7.5 |
| X-05 | (b)의 조건이 "깨움 보장"으로(판정 함수 `orca_wakes`). 탈출구 문구 변경: (a)는 미처리 메시지를 `ack_by`로 나눠 `merge`와 `--ack <메시지 ID> --no-wait`를 따로 적고, (b)는 세션 핸들 유무에 따라 다르다 | 7.5 |
| X-06 | 통합을 확인한 효과가 그 completion의 메시지에 `acked_at`도 쓴다 | 7.5 |
| Stop 공통 전제 | 유지. 걸리는 것이 grok의 `reason: "shutdown"`뿐이라는 설명 | 7.5 |
| F-01 | 본문이 `error`(종류)와 `error_details`의 조합 | 7.5 |
| D-06 | 예외 칸: 구현 dispatch의 `terminal`이 `null`이면 터미널 비교를 하지 않는다 | 8.5 |
| **D-12 (새로)** | `dispatch`의 패킷 검사. 패킷 조건이 참일 때만 | 8.5 |
| R-08 | 묶인 dispatch가 없으면 이 워크트리의 그 과제 dispatch가 하나일 때 그것에 `reported`를 쓴다 | 8.7 |
| R-09 | 조건이 "dispatch가 묶여 있으면"에서 "좌석이 `Role`이면"으로. dispatch를 찾지 못해도 본문을 낸다 | 8.7 |
| **R-10 (새로, `done`)** | `done`의 패킷 검사 | 8.7 |
| M-10 | 그 completion의 메시지와 같은 과제 키의 dispatch에 딸린 `worker_done` 메시지 전부에 `acked_at`을 쓴다 | 8.8 |
| M-13 | 출력 문구 변경 | 8.8 |
| 리뷰 `check` 10번 (새로) | 패킷 검사 | 8.9 |
| **J-01, J-02, J-03, J-04, J-10~J-17 (새로)** | 탐색 명령의 거절과 `check_packet`의 항목. J-04는 입력이 달라졌는데 `--force`가 없을 때의 거절이다. J-12는 지시서 파일을 지시서 지문으로 비교한다. `messages.rs`에 J 구역 | 8.14 |

hook 규칙 표(`RULES`)에 들어가는 새 ID는 S-06 하나다. D·R·M·J는 CLI의 거절 ID다.

### 0.5 CLI 표면 (8절, 10절)

| 명령 | 바뀐 내용 | 절 |
|---|---|---|
| `fullops hook <host>` | 호스트 값이 `claude\|codex\|grok`. 그 밖의 값은 `{}`·종료 코드 0. `--plugin-root`가 없으면 실행 파일 경로에서 추론 | 7.1 |
| `fullops`(인자 없음) | stdout 빈 채로 종료 코드 0 | 7.1, 8.1 |
| `fullops dispatch` | `--task <Orca task id>` **삭제** | 8.5 |
| `fullops wait` | `--ack`의 값이 전달 ID에서 **메시지 ID**로. `--no-wait` 추가. Orca가 깨우는 세션(`orca_wakes`가 참)에서는 `--no-wait` 없이도 기다리지 않는다. 반환 JSON에 `orca_wakes`, `actionable[].ack_by`·`from`·`subject` 추가, `delivery`는 `null` 가능. `next`의 조립 규칙 | 8.6 |
| `fullops done` | 인자는 그대로. 본문 출력 조건과 `reported`를 쓰는 대상이 바뀌었다(R-08·R-09) | 8.7 |
| `fullops task recover` | 패킷을 다시 만들 때 WIP 커밋이 바꾼 경로를 seed에 더하고, 다시 만든 패킷과 인박스를 `fullops: 패킷 갱신 <KEY> (<R>)` 커밋에 넣는다 | 8.4 |
| `fullops sync` | 표면은 그대로. 파일 `cmd/sync.rs`, 소유 묶음 E | 8.8, 10.1 |
| `fullops find` (새로) | `<KEY> --role <R> [--scope code\|documents] [--limit N] [--max-calls N] [--force] [--env-file 경로]` | 8.14 |
| `fullops context` (새로) | `<KEY> --role <R> --paths 경로... [--required 경로]... [--force] [--env-file 경로]` | 8.14 |
| `fullops packet build` (새로) | `<KEY> --role <R> [--seed 경로]... [--required 경로]... [--update-doc Dnn]... [--decisions 파일] [--write-inbox] [--force]`. `find`·`context`·`packet build` 공통: 결과 파일이 있고 입력이 다르면 `--force` 없이는 J-04로 거절한다 | 8.14 |
| `fullops packet check` (새로) | `<KEY> --role <R> [--completion] [--head SHA]` | 8.14 |
| `fullops install` | `[--prune] [--rollback]`에서 `[--hosts …\|all] [--only runtime\|plugin\|deps]... [--dry-run] [--json]`, `--check`, `--prune`, `--rollback`으로. 예전의 `install`(실행본 복사)은 `--only runtime`. agy에는 `~/.fullops/package/<버전>`의 사본 경로를 넘긴다. CLI의 플러그인 루트 규칙(8.1절) | 8.1, 10.3 |
| `fullops doctor` | 보는 항목에 호스트, 의존, Codex hook 신뢰(`host == codex`인 세션 파일의 유무로 본다), grok 폴더 신뢰 추가 | 10.6 |
| `fullops init`, `migrate` | 관리 블록 대상에 `GEMINI.md` 추가 | 10.6, 13.2 |

### 0.6 워크스페이스와 시험 도구 (10.1절, 18.1절)

| 대상 | 바뀐 내용 |
|---|---|
| `fullops-core` | 파일 `explore.rs`, `deps.rs` 추가(`deps.rs`는 `HOST_COMMANDS`와 `install_plan`만 담는다. `Deps` 타입은 `model.rs`). 쓸 수 있는 크레이트에 `serde_json`, `sha2` 추가 |
| `fullops-store` | 파일 `search.rs` 추가 |
| `fullops-ext` | 파일 `hosts.rs` 추가 |
| `fullops` | `hook/host_grok.rs`, `cmd/{find,context,packet,sync}.rs` 추가. `assets/`에 `deps.toml` 내장 사본 |
| `plugin/` | `deps.toml` 추가. 패키지가 "마켓플레이스판·디렉터리판·Codex" 셋에서 "통합 패키지·디렉터리판" 둘로 |
| `fullops-testkit` | `fake-host`(`claude`·`codex`·`grok`·`agy`·`npx` 이름), `fake-curl`, 임시 홈 빌더 추가. `fake-orca`는 소비형 `check`와 `--peek`의 차이, `--retry-request`를 흉내 낸다 |

### 0.7 바뀌지 않은 것

`Config`와 `fullops.toml`, `PLANS.md` 문법, 인박스 front matter, `Verdict`, `Effect`(변형 목록 그대로), `Intent`의 변형 목록, `Rule` 구조체, `Known`, `Refusal`, `Actor`·`Seat`, `turns.jsonl`, `status.json`, `archive.jsonl`, lint의 `result.json`·`pass.json`, 리뷰 폴더의 파일 형식, 종료 코드 규약.

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
