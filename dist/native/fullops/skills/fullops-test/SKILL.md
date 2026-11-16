---
name: fullops-test
description: FullOps setup이 완료된 레포에서 구현된 동작을 검증하거나 테스트 코드·시나리오를 작성·실행하고 검증 증거를 남길 때 사용한다.
---

# 동작 검증

활성 레포(`.fullops-squad/fullops.toml`이 있는 레포)에서만 쓴다. 표식이 없으면 `setup-fullops`를 안내한다. 어느 역할이든 동작 검증에 이 문서를 쓴다. tester 역할은 제품 코드를 고치지 않고 테스트 코드, 시나리오, 검증 증거만 만든다.

명령 표기: 이 문서의 `fullops`는 Claude Code 세션에서 그대로 실행된다. Codex·grok·agy 세션에서는 고정 위치 실행본을 전체 경로로 부른다: `~/.fullops/bin/fullops`(Windows는 `%USERPROFILE%\.fullops\bin\fullops.exe`).

## 범위

- 범위는 테스트 레벨이 정한다. `fullops.toml`의 `test_level`과 지시서가 고른 레벨을 읽고 `.fullops-squad/rules/common/testing.md`의 `minimal`·`lite`·`standard`·`full`·`exhaustive` 범위를 적용한다. 레벨 밖의 검사·캡처 폭·테마를 더하지 않는다.
- 지시서의 검사별 담당·대상·시점·통과 조건을 확인한다. 구현 역할의 관련 회귀와 tester의 전체 수락 QA를 구분한다.
- 정한 검사가 통과하면 인계한다. 새 변경, 실패, 증거 결함이 있을 때만 반복한다.
- 범위 밖이거나 실행하지 못한 검사는 사유를 남기고 통과로 표시하지 않는다.
- 테스트를 하위 에이전트로 더 돌리지 않는다(`.fullops-squad/rules/delegation.md`).

## 싼 수단부터

앞 단계로 판정할 수 있으면 뒤 단계로 가지 않는다.

1. 확정 검사: 값·문구·상태처럼 코드로 판정할 수 있는 것은 테스트 코드로 검증한다. 레포의 테스트 명령은 `.fullops-squad/project.md`와 `.fullops-squad/lint/lint.json`의 `commands`에 있다. 등록된 명령은 `fullops lint --from <기준>`이 바뀐 파일과 레벨에 맞춰 골라 돌린다. 한 번 더 높은 레벨로 돌리려면 `fullops lint --level <레벨>`을 쓴다.
2. 조작 검증: 사람이 조작해야 드러나는 흐름은 레포가 가진 E2E 도구(예: Playwright 테스트)로 고정 시나리오를 만들어 돌린다. 시나리오는 `docs/evaluations/scenarios/`에 기능 단위로 두고 커밋한다. 통과시키려고 검사 조건을 약하게 고치지 않는다. 브라우저·게임 조작 도구는 플러그인이 싣지 않는다. 레포가 직접 설치한 도구를 쓰고, 없으면 미실행으로 적는다.
3. 큰 모델의 눈: 이미지로만 판단할 수 있는 시각 품질과 앞 단계 실패의 원인 분석에만 쓴다. 캡처를 판정 근거로 쓰면 파일 경로를 남긴다.

UI 검증의 디자인 정본과 수락 기준은 `fullops-frontend`를, 모션그래픽 영상의 검수는 `fullops-motion`의 `delivery.md`를 따른다. 도구가 설치돼 있다는 것이나 디자인 비평만으로 브라우저 QA를 통과 처리하지 않는다.

## 증거 규칙

- 한 과제의 검증 결과 정본은 `.fullops-squad/docs/evaluations/qa-reports/<KEY>-test/report.md` 하나다. 수정 차수마다 새 폴더(`<KEY>-FIX-test` 등)를 만들지 않고 같은 보고서에 덧붙인다(새 폴더는 hook이 거부한다).
- 통과한 검사는 명령·실행 위치·SHA·종료 코드를 한 줄로 적는다. 실패·미실행·한계만 자세히 적는다. `minimal`·`lite`의 보고서는 40줄을 넘기지 않는다(넘으면 `fullops done`이 알린다).
- 캡처와 영상은 지정된 시각 판정에 필요한 것만 만든다. 보고서가 링크하지 않은 미디어는 커밋하지 않는다(hook이 거부한다). 원본 로그와 대용량 자료는 `raw/`에 두고 커밋하지 않으며 보고서에서 링크하지 않는다.
- 재사용할 증거와 이번에 실행한 범위를 구분해 적는다. 같은 SHA의 통과한 lint 결과는 다시 돌리지 않고 재사용한다.
- 검증이 끝나면 `fullops-work`의 완료 순서(커밋 → `fullops lint` → 완료 보고 → `fullops done`)를 따른다. 완료 보고의 검증 항목에는 실제 출력의 발췌나 보고서 링크를 적는다.
