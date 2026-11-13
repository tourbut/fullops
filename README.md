# Fullops

Orca 안에서 개발이 사람 없이 이어지게 하는 하네스의 **Rust 재구축판**입니다.

이 저장소는 Python 플러그인 [tourbut/Fullops-Squad-plugin](https://github.com/tourbut/Fullops-Squad-plugin)(현재 v1.24.0)을 다시 구현합니다. 이전 저장소 [tourbut/FullOps-Squad](https://github.com/tourbut/FullOps-Squad)나 기존 Python 플러그인과는 다른 레포입니다. 원격 작업 효율은 [orca-helm](https://github.com/tourbut/orca-helm)과의 결합 효과이지, Fullops 자체의 목적은 아닙니다.

## 재구축 방식

기존 Python 코드를 줄 단위로 옮기지 않습니다. 제품의 컨셉과 현재 버전(v1.24.0)의 기능을 기준으로 설계를 먼저 정하고, 그 설계대로 Rust로 다시 만듭니다. 설계는 [docs/design.md](docs/design.md)가 정본입니다.

- 완전히 Rust 기반입니다. hook과 CLI는 실행 중에 Python·Node에 기대지 않습니다. 다시 만드는 이유는 성능입니다(hook은 도구 호출마다 실행됩니다).
- 합격 기준은 결과 비교입니다. 다 만든 Rust 플러그인을 기존 플러그인과 같은 상황에 놓고, 같은 목적을 이루는지(또는 더 잘 이루는지)와 속도를 비교합니다. 출력 글자나 상태 파일 바이트가 같은지는 기준이 아닙니다.
- Python판과 상태 파일을 번갈아 쓰지 않습니다. Rust판은 자기 형식을 씁니다.
- 처음 컨셉에서 나중에 바뀐 것은 바뀐 뒤가 기준입니다(예: Jev의 모델 선택은 1.10.0에서 없앴습니다).
- 구현은 빠르게 하고 검증은 통합 시험에서 합니다. 단위 시험은 최소로 둡니다.

그동안 **jevpet은 기존 플러그인 1.24.0을 계속 사용**합니다. 합격 기준을 통과한 뒤에만 전환하며, 전환은 사람 승인을 받습니다.

## 기존 Python판의 실제 구조 (v1.24.0)

원본은 `plugins/fullops-squad/`입니다. 배포본은 `scripts/build.py`가 `dist/native/fullops-squad`와 `dist/directory/fullops-squad`로 만듭니다. 마켓플레이스 등록 경로는 `./dist/native/fullops-squad`입니다. coordinator는 모듈이 아니라 역할 이름입니다.

### hook 진입점

`plugins/fullops-squad/hooks/hooks.json`이 호스트 이벤트를 `hook_runtime.py`로 보냅니다. 런타임이 아래 스크립트를 실행합니다.

| 호스트 이벤트 | 호출 | 인자(mode) |
|---|---|---|
| SessionStart (`startup\|clear`) | `caveman_start.py` | (없음) |
| SessionStart | `done_gate.py` | `start` |
| SessionStart (`startup\|clear`) | `flow_gate.py` | `start` |
| UserPromptSubmit | `flow_gate.py` | `prompt` |
| PreToolUse | `flow_gate.py` | `tool` |
| PostToolUse / PostToolUseFailure | `flow_gate.py` | `post` |
| Stop | `done_gate.py` 이어서 `flow_gate.py` | `stop` |
| StopFailure | `flow_gate.py` | `failure` |
| SessionEnd | `issue_mode.py` | `session-end` |

공통 입출력: 표준입력은 hook 이벤트 JSON, 표준출력은 판정 JSON입니다. 계약은 `done_gate.py`의 `run_hook()`이 정리합니다. hook 오류는 작업을 막지 않습니다(fail-open). `.fullops-squad/fullops.json`이 없으면 빈 객체를 돌려줍니다.

`hook_runtime.py`는 네이티브(마켓플레이스)판에서 `~/.fullops-squad/runtime/fullops-squad`에 패키지 스냅샷을 두고 디스패치합니다. 디렉터리판은 `hooks/run.sh`로 플러그인 폴더에서 그대로 실행하며 런타임 복사가 필요 없습니다(`in_place`). Claude Code가 남기는 `.in_use` 표지는 패키지 내용이 아닙니다. Codex 패키지(`hooks/codex.json`)에는 `PostToolUseFailure`와 `StopFailure`를 넣지 않습니다.

### hook이 부르는 게이트·지원 모듈

- `flow_gate.py`: 역할 안내, 도구 차단, worker_done 정산, 백그라운드 대기(`waiting_in_background`), PLANS.md 준비된 대기 행(`board.py:ready`), 사람 턴 기록(`fullops-flow/human-turns.jsonl`), StopFailure escalation
- `done_gate.py`: 세션이 만든 코드 변경에 현재 HEAD의 `lint.py` 통과 기록이 있는지 Stop에서 확인
- `harness_gate.py`: 보고서 분량·미디어 링크·반복 예산 알림
- `integration.py`: 통합 기록, 원격 기본 브랜치 기준 판정, `settle`(병합된 hold 해제)
- 그밖에 hook 경로에서 쓰는 모듈: `policy.py`, `board.py`, `flow_state.py`, `review.py`, `decision.py`, `env_link.py`, `deps.py`, `output_trim.py`, `orca_wait.py`

### 그다음 이식할 스크립트

`work.py`, `review.py`, `jev_route.py`, `jev_packet.py`, `issue_mode.py` 본체, `lint.py`, `setup.py`, `update.py`, `board.py` 현황판, 기타 `plugins/fullops-squad/scripts/`의 나머지 스크립트. 시험은 `tests/`에 스크립트별로 있습니다. 과제 흐름 노드·게이트 표는 `docs/flow-graph.md`와 `tests/flow-graph.py`입니다.

## 구현 순서

[docs/design.md](docs/design.md) 18절의 구현 계획을 따릅니다. 공용 뼈대(타입·인터페이스·저장·시험 도구)를 먼저 깔고, hook, 흐름 명령, lint·문서, 리뷰·통합, 설치·배포, 비교·성능 묶음을 서로 다른 워크트리에서 동시에 진행합니다.

## 유지할 결정 (1.24.0 / HARNESS-AUTONOMY)

- 리뷰는 위험 기반. 테스트·리뷰는 최소·표적. LLM 호출을 늘리지 않음
- 되돌릴 수 있는 디자인·연출·문구·배치 선택과 버그 수정 여부는 자율 판단. 사용자에게 올리지 않음
- 운영 배포, 운영 부하 테스트, 기존 데이터 보충(backfill), 운영 데이터 정리는 항상 사람 승인
- 제품이 정한 사용자 선검토(예: 병합 전 ART 시안 검토)는 유지
- 이슈 모드가 꺼져 있으면 보류 잡이 무관한 worker를 Orca 조회에 묶지 않음
- 탐색 패킷은 선택. 패킷이 있는 과제만 검사
- 모델 사용 한도로 worker가 끝나면 같은 모델을 기다리고, 과제당 재배정은 최대 2회. 다른 모델·호스트로 자동 전환하지 않음
- 지표는 과제당 사람 턴 수, 사람 없이 이어진 과제 수. 본문은 저장하지 않음(`fullops-flow/human-turns.jsonl`)

근거: [v1.24.0 릴리스 노트](https://github.com/tourbut/Fullops-Squad-plugin/blob/v1.24.0/docs/releases/1.24.0.md), [HARNESS-AUTONOMY 검증 보고](https://github.com/tourbut/Fullops-Squad-plugin/blob/v1.24.0/.fullops-squad/docs/evaluations/qa-reports/HARNESS-AUTONOMY-test/report.md).

## 배포

Claude Code 플러그인 배포본에는 Windows·macOS 바이너리를 넣어야 합니다. 기존 구조(`scripts/build.py`, `adapters/`, `dist/native`, `dist/directory`)를 참고하되, hook 명령은 `python3` 대신 OS·CPU에 맞는 바이너리를 고릅니다. 폐쇄망(`FULLOPS_OFFLINE`)과 Codex hook 이벤트 차이는 기존 동작을 유지합니다. 자세한 검토 항목은 원본 이슈 [#132](https://github.com/tourbut/Fullops-Squad-plugin/issues/132)를 참고합니다.

## 관련 저장소

| 저장소 | 역할 |
|---|---|
| [tourbut/fullops](https://github.com/tourbut/fullops) | 이 레포. Rust 본체 |
| [tourbut/fullops-spec](https://github.com/tourbut/fullops-spec) | 언어 무관 동작 명세와 실행기 |
| [tourbut/Fullops-Squad-plugin](https://github.com/tourbut/Fullops-Squad-plugin) | 기존 Python 플러그인(v1.24.0이 기준선). 전환 전까지 실사용 |
| [tourbut/FullOps-Squad](https://github.com/tourbut/FullOps-Squad) | 이전 저장소. 이 Rust판이 아님 |
