# 단일 에이전트 루프 원칙 적용 결정 기록

작성일: 2026-11-21. 요청자: 성철 님. 범위는 작고 되돌리기 쉬운 다섯 항목이다. 역할 접기(architecture·designer), 기본 모드 변경, tester 유지 판단은 이 기록의 범위가 아니다.

## 1. 참고 자료

1. Anthropic, [Building agents: When to fold subagents into your main agent loop](https://claude.com/resources/articles/building-agents-when-to-fold-subagents-into-your-main-agent-loop) (2026-10-22)
2. [anthropics/single-agent-loop-kit](https://github.com/anthropics/single-agent-loop-kit) (`release` 브랜치). `skill/SKILL.md`, `skill/steps/00-audit.md`·`01-baseline.md`·`03-fold.md`, `examples/support-desk/RUNBOOK.md`를 읽었다.
3. Claude Agent SDK, [Scale to many tools with tool search](https://code.claude.com/docs/en/agent-sdk/tool-search)
4. 이전 검토: Fullops-Squad-plugin `docs/proposals/single-agent-loop-review.md` (브랜치 `tourbut/single-agent-loop-review`, 커밋 `9d81c04`). 참고로만 썼다.
5. MyKnows 위키 `concepts/single-agent-loop.md`

## 2. 원문 핵심 지식

- 역할별로 나눈 라우터·플래너·서브에이전트 대신 메인 루프 하나를 기본으로 한다. 스킬과 도구는 짧은 설명만 먼저 보여 주고 전체 내용은 필요할 때 불러온다(점진적 공개). Anthropic은 이렇게 해서 품질·비용·지연이 함께 좋아졌고 Claude Code 시스템 프롬프트를 80% 넘게 줄였다고 한다.
- 역할 배치: 업무 절차는 스킬에, 지켜야 할 조건과 권한 검사는 도구 코드에, 기계적 반복은 일반 코드에 둔다. 키트의 말로는 "고정된 단계는 워크플로이고 워크플로는 일반 코드다".
- 도구 정의가 30~50개를 넘으면 고르는 정확도가 떨어진다. API 단위가 아니라 사용자 의도 단위로 묶는다. 도구가 약 10개 미만이면 미리 싣는 편이 빠르다(tool search 문서).
- 서브에이전트는 세 경우에만 남긴다. (a) 컨텍스트가 커져 성능이 떨어지는 것이 측정될 때, (b) 새 컨텍스트에서 따로 돌리고 결과만 받으면 되는 병렬 작업일 때, (c) 지시문끼리 부딪치거나 도구를 잘 못 고를 때. 나누는 기준은 역할이 아니라 컨텍스트를 떼어 낼 필요다. 키트는 `keep`을 "드물다"고 적는다.
- 접기 순서: 정답 예시 10개 정도로 평가 세트를 먼저 만든다. 서브에이전트를 한 번에 하나씩 접는다. 매번 앞뒤 점수를 비교하고, 원래 통과하던 과제가 실패로 바뀌었는지(뒤집힌 과제) 따로 본다.

## 3. 다시 검토한 결과

원문과 키트를 직접 읽고 이전 검토와 대조했다. 범위 1~5와 부딪치는 결론은 없었다. 이전 검토와 달리 본 점은 넷이다.

1. 키트의 평가 세트는 실제 모델로 시스템을 돌리고 과제마다 2회 이상 반복한다. 이번 기준선은 토큰 비용 때문에 모델을 부르지 않는다. 그래서 하네스 판정(hook·CLI)과 스킬 핵심 본문의 문구만 본다. 에이전트가 맞는 스킬을 골랐는지는 재지 못한다. 역할을 접는 다음 작업(architecture 등)에는 이 한계를 알고 쓴다.
2. Rust판의 verifier 문구(`templates/handover.md`)는 `delegation.md`의 `검증 하위 작업`을 가리켰지만 그 절은 이미 없었다. 끊긴 참조가 서브에이전트 호출만 남기고 있었다.
3. `parallel` 기본값을 낮추면 lead의 "담당 역할이 서로 다른 행은 한 턴에 함께 배정한다" 안내도 함께 꺼졌다. 이 안내는 서로 다른 워크트리가 필요한 병렬(남길 서브에이전트)이므로 `off`가 아니면 유지하게 고쳤다.
4. FullOps가 등록하는 도구 정의는 Context7 MCP 2개뿐이다. tool search 설정은 필요 없다.

## 4. 바꾼 것

| 항목 | 근거 원칙 | 바꾼 파일 | 줄어든 것 | 커밋 |
|---|---|---|---|---|
| 1. 행동 기준선 | 접기 전에 평가 세트 10개, 뒤집힌 과제를 따로 본다 | `xtask/src/baseline.rs`, `xtask/src/main.rs`, `xtask/src/compare.rs`(가시성만) | 추가. 모델 호출 0회 | `eea07f9` |
| 2. verifier를 일반 코드 실행으로 | 고정 절차는 일반 코드에 둔다 | `crates/fullops/assets/repo/templates/handover.md` | standard 이상 동작 변경 과제마다 verifier 서브에이전트 1회 | `25d7834` |
| 3. `parallel` 기본값 `on` → `read` | 기본은 한 루프로 접고 서브에이전트는 격리가 필요할 때만 | `init.rs`, `migrate.rs`, `messages/install.rs`, `rules/session.rs`, `status.rs`, `delegation.md`, `setup-fullops`·`fullops-work`·`fullops-orca` 스킬, `design.md` | 새 레포의 구현 병렬 서브에이전트 권장 문구가 기본 경로에서 빠진다 | `8357030`, `7e4dfd1` |
| 4. 큰 스킬 분할 | 점진적 공개. 스킬 본문은 짧게, 세부는 참조 파일 | `fullops-orca/{SKILL,questions,recovery,models}.md`, `fullops-work/{SKILL,explore,memo}.md`, `tests/bundle_f.rs` | `fullops-orca` 10,916 → 5,991자(−45%), `fullops-work` 7,532 → 4,760자(−37%) | `2dd89e8` |
| 5. 세션 시작 문서 축소 | 점진적 공개. 거의 항상 필요한 작은 것만 미리 싣는다 | `crates/fullops/assets/repo/guide/FULLOPS.md` | 세션 시작 때 무조건 읽던 목록 약 14,600자(`FULLOPS.md` 2,401 + 공통 규칙 4개 10,640 + `project.md` 1,559) → `fullops status`만 | `e8866fb` |

글자 수는 Python으로 센 문자 수다. 토큰 수는 재지 않았다.

### 4.1 항목별 내용

1. `cargo xtask baseline`이 고정 케이스 10개를 돌린다. 케이스는 기존 시나리오(rs 구동기)와 스킬 핵심 본문에 남아야 할 명령 문구 한 쌍이다. 채점은 기계로 한다: 시나리오의 모든 단계가 기대와 같고 문구가 모두 있으면 pass다. LLM 판정과 상시 CI는 없다. 케이스 표는 `xtask/src/baseline.rs`의 `CASES`다.
2. 지시서 템플릿의 `verifier` 줄을 `검증 실행` 줄로 바꿨다. 등록된 lint·빌드·시험은 `fullops lint`가 실행·기록한다. 증명 방법의 명령은 worker가 직접 실행한다. 출력 해석이 필요한 실패는 worker(부모)가 읽고, 독립 판단은 리뷰 판정이 요구할 때 reviewer가 맡는다. 새 명령은 만들지 않았다. `fullops lint`가 이미 그 일을 한다.
3. `fullops init`의 기본값을 `read`로 바꿨다. `migrate`는 1.24.0의 `lite`와 값이 없던 레포를 `read`로, 사람이 올린 `standard`·`full`만 `on`으로 옮긴다. `on`은 사람이 `fullops.toml`에서 고를 때만 쓴다. 이미 setup한 레포의 값은 바꾸지 않는다.
4. `fullops-orca` 본문에는 분류·배정·대기·병합·다음 배정만 남겼다. 질문 전달과 사용자 결정 기록, 배정 거절과 워크트리 복구, 모델 고르는 기준을 참조 파일로 옮겼다. `fullops-work`는 탐색과 역할 메모를 옮기고 병렬 진행은 정본 `delegation.md`를 가리키게 했다. 본문마다 "X일 때 Y를 읽는다"를 한 줄씩 둔다.
5. `FULLOPS.md`의 표 머리를 "그 작업을 할 때 읽을 문서"로 바꾸고 "세션을 시작하면 `fullops status`만 실행한다"를 넣었다. 공통 규칙과 `project.md`는 코드를 바꾸거나 검토할 때, `PLANS.md`는 행을 고칠 때만 연다. lint·리뷰 게이트와 hook 검사는 그대로다.

바꾼 스킬과 템플릿에는 근거 원칙을 한 줄씩 남겼다("근거: 단일 에이전트 루프의 점진적 공개" 등).

## 5. 서브에이전트로 남긴 것과 이유

| 실행 단위 | 남긴 이유(원문 조건) |
|---|---|
| 커밋을 고정한 독립 리뷰 세션 | (b) 새 컨텍스트에서 작성자와 떨어져 판단하고 결과만 받는다. 키트의 "다른 쪽이 어떻게 만들었는지 모른 채 검사" 조건이다. `review need`가 요구할 때만 띄운다 |
| 이슈 오토모드의 하위 실행 | (b) 병렬·격리. 허용 목록·lease·epoch 경계가 있다 |
| 서로 다른 워크트리가 필요한 병렬 작업(역할 단위 배정) | (b) Git 브랜치 격리가 필요한 병렬 팬아웃이다. 그래서 `read`에서도 함께 배정 안내를 유지한다 |
| `read` 방침의 읽기 전용 조사·리뷰 하위 에이전트 | (a)·(b) 넓은 탐색의 원본 로그를 버리고 결과만 받는다 |

architecture·designer·tester 역할과 기본 모드(coor/dev)는 건드리지 않았다. 성철 님이 따로 정한다.

## 6. 기준선 쓰는 법

역할을 접는 작업 앞뒤에만 돌린다. 상시 CI에 넣지 않는다.

```sh
cargo build -p fullops-testkit -p fullops
cargo xtask baseline --out target/baseline/before.tsv
# 변경 뒤
cargo xtask baseline --against target/baseline/before.tsv --out target/baseline/after.tsv
```

- 출력: 케이스 표, 통과 수, 스킬 핵심 본문 글자 수, 뒤집힌 케이스. 실패가 있거나 pass에서 FAIL로 뒤집힌 케이스가 있으면 종료 코드가 0이 아니다.
- 결정형이라 반복은 1회로 충분하다. 실제 모델로 돌리는 평가(키트의 2회 이상 반복)는 비용이 드는 별도 결정이다.
- 이번 결과: 변경 전 10/10. 스킬 첫 분할에서 B08이 pass → FAIL로 뒤집혔다(`fullops decision answer`가 본문에서 빠짐). 본문 요점에 되살린 뒤 10/10, 뒤집힌 케이스 없음.

| 케이스 | 시나리오 | 보는 스킬 |
|---|---|---|
| B01 과제 3개를 배정·완료·병합까지 | `k4` | `fullops-orca` |
| B02 dev 모드 세션 시작 안내 | `b3` | `fullops-work` |
| B03 병렬 방침 read | `d7-read` | `fullops-work` |
| B04 worker가 lint 없이 끝내면 막힘 | `g1` | `fullops-work` |
| B05 dev 큰 변경은 리뷰나 면제 기록 필요 | `g5` | `fullops-review` |
| B06 `fullops done`의 lint 증거와 완료 보고 | `h4` | `fullops-work` |
| B07 review need 경계 | `l1` | `fullops-review` |
| B08 대규모 변경 merge와 사용자 결정 | `l2` | `fullops-orca` |
| B09 merge와 워크트리 동기화 | `l5` | `fullops-orca` |
| B10 한도 재배정 | `d6` | `fullops-orca` |

## 7. 남은 위험

- 기준선은 에이전트가 참조 파일을 실제로 읽는지 재지 못한다. 드문 경로(질문 전달, 복구, 모델 선택)를 놓칠 수 있다. 질문 카드·결정 기록·모델 누락은 hook과 CLI 거절(D-09, D-14 등)이 막는다.
- 템플릿(`FULLOPS.md`, `handover.md`, `delegation.md`) 변경은 새 `init`에만 들어간다. 기존 레포에는 다음 릴리스 노트와 `update-fullops`로 알려야 한다. 릴리스 노트는 쓰지 않았다.
- testkit 시나리오의 기본 설정은 여전히 `parallel = "on"`이다. 제품 기본값과 다르다. 시나리오 기대값을 지키려고 그대로 두었다.
- `read`에서는 큰 과제의 완료 시간이 늘 수 있다. 그때는 사람이 `on`을 고른다.
- 토큰 수가 아니라 글자 수로 쟀다.
