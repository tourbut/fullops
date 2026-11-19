---
name: fullops-review
description: FullOps 활성 레포에서 worker 구현 완료 후 병합 전 독립 리뷰를 준비하거나 리뷰 기록을 검증할 때 사용한다.
---

# 병합 전 독립 리뷰

리뷰는 위험 기반이다. 대규모 변경에만 하고, 모든 변경을 리뷰하지 않는다. 판정과 기록 검사는 `fullops review`가 하고, 수락 판단은 검토자와 병합 책임자가 한다.

명령 표기: 이 문서의 `fullops`는 Claude Code 세션에서 그대로 실행된다. Codex·grok·agy 세션에서는 고정 위치 실행본을 전체 경로로 부른다: `~/.fullops/bin/fullops`(Windows는 `%USERPROFILE%\.fullops\bin\fullops.exe`).

## 리뷰 필요 판정

`fullops merge <KEY>`가 병합 전에 스스로 판정한다. 미리 보려면 `fullops review need <KEY>`(또는 `--from <ref> --to <ref>`)를 쓴다. 기준 시점(merge-base 커밋)의 `fullops.toml`의 `[review]`를 읽으므로 작업 브랜치가 기준을 낮추지 못한다. 기본값은 코드 400줄 이상, 코드 파일 15개 이상, `always` 경로다.

- `skip`: 리뷰하지 않는다. `merge`가 그대로 진행하고 판정 사유를 병합 기록 커밋에 남긴다.
- `ask`: `merge`가 결정을 열고 종료 코드 3으로 끝난다. 사용자에게 판정 사유, 예상 비용(별도 리뷰어 세션), 선택지(진행 / 생략)를 묻고 답을 `fullops decision answer <ID> --answer "<진행|생략>" --sha <SHA>`로 기록한다.
  - 진행: 아래 리뷰 절차를 따른다.
  - 생략: `fullops review waive <KEY> --decision <ID> --reason "<사용자 지시 요지>"`로 면제 기록을 남긴 뒤 `fullops merge <KEY>`를 다시 실행한다. 면제는 수락이 아니라 사용자 결정의 기록이다. lead가 스스로 면제를 정하지 않는다.
- `review`: `[review].auto`가 참이면 묻지 않고 리뷰한다.

dev 모드 주 담당의 직접 작업도 같은 기준을 넘으면 다른 세션의 독립 리뷰나 면제 기록이 있어야 턴을 끝낼 수 있다.

## 리뷰 절차

1. lead가 `fullops review snapshot <KEY>`를 실행한다. 보고 SHA의 읽기 전용 사본(detached 워크트리)과 리뷰 폴더 `.fullops/tasks/<KEY>/reviews/<head 12자>/`가 생긴다. 폴더에는 대상 목록 `targets.json`, 결과 뼈대 `result.json`, 보고서 틀 `report.md`, 찾을 수 있으면 구현자의 lint 결과 사본 `lint.json`이 들어간다. 대상과 경로별 규칙은 `.fullops/config/review.json`에서 계산한다.
2. lead가 리뷰어 모델을 기록하고 배정한다: `fullops route <KEY> --tuple-only --role reviewer --agent <CLI> --model <모델> --effort <effort>` 뒤 `fullops dispatch <KEY> --purpose review --sha <40자 SHA>`. 리뷰어는 구현자와 다른 세션이어야 한다. 구현자의 터미널을 `--terminal`로 주면 거절된다.
3. 리뷰어는 spec의 `Review SHA`와 `Review folder`를 확인하고 snapshot만 읽는다. lint와 테스트를 다시 돌리지 않고, 구현자 워크트리에 파일을 만들거나 고치지 않는다. snapshot에는 설치된 의존성(`node_modules` 등)이 없어 snapshot 안의 `fullops lint`는 거절된다. lint 증거는 `Review folder`의 `lint.json`(구현자의 결과 사본)으로 본다. 증거가 부족하면 수정 요청 finding으로 남긴다.
4. 리뷰어는 요구사항·설계·지시서의 완료 기준(인박스에 없으면 `.fullops/tasks/<KEY>/`의 끝난 지시서 `<역할>.r<차수>.md`에서 찾는다), `.fullops/guide/rules/common/`의 규칙, `targets.json`의 경로별 규칙 문장을 읽고 `merge_base`와 `head` 사이의 diff를 검토한다. 내부 코드는 관련 파일을 직접 읽고, 외부 SDK 근거는 Context7로 확인한다.
5. 리뷰어는 lead 체크아웃의 리뷰 폴더에 결과를 쓴다. `result.json`의 모든 파일에 `review_status`(`reviewed` 또는 `skipped`)를 적고 `skipped`에는 `reason`을 적는다. `reviewer_session`에 자기 세션 ID를, `conclusion`에 `accept` 또는 `reject`를 적는다. finding마다 `path`, `content`, `severity`(`critical`·`high`·`medium`·`low`), `resolved`, 줄 범위를 적는다. `report.md`에는 적용한 규칙과 기준 커밋, 예외, 검증 근거, 검토 결론을 적는다. `report.md`만 쓰고 `result.json`을 뼈대 그대로 두면 lead의 `check`가 실패한다.
6. 리뷰어는 보고하기 전에 snapshot 안에서 `fullops review check <KEY>`를 반드시 실행한다(lead 체크아웃의 리뷰 폴더를 찾아 검사한다). 기록이 완전하면 결론이 `accept`든 `reject`든 통과한다. 통과하지 못하면 알려 준 항목을 고쳐 통과할 때까지 다시 실행한다. 통과한 뒤 Orca preamble의 `worker_done`을 한 번 보낸다. 본문은 `[리뷰] <KEY> | SHA <head> | 결론 accept|reject | high <n>`이다. 리뷰어는 `fullops done`을 쓰지 않는다.
7. lead가 `fullops wait --run <R>`로 보고를 받고 `fullops review check <KEY>`를 실행한다. 기록이 대상 목록과 맞는지, 리뷰어와 구현자의 세션이 다른지, snapshot이 깨끗한지, 미해결 critical·high가 없는지, lint 증거가 그 SHA를 덮는지를 본다. 통과하고 결론이 `accept`면 `accepted.json`이 생긴다.
8. 수락이면 `fullops merge <KEY>`가 통과하고 리뷰 보고의 처리 표시와 snapshot 정리도 함께 한다. 거절이거나 미해결 critical·high가 있으면 수정 배정으로 간다: `fullops task reopen` 뒤 `fullops dispatch <KEY> --purpose fix`. 수정 커밋 뒤에는 새 SHA로 다시 리뷰한다(이전 SHA의 리뷰로는 병합되지 않는다).

## 리뷰 정책

검토자의 판단 기준이다. `fullops review check`는 이 절을 판정하지 않는다.

한 세션에서 세 관점으로 읽는다. 관점마다 검토자를 따로 띄우지 않는다. finding의 `content` 앞에 `[버그]`·`[보안]`·`[준수]` 또는 `[nit]`을 하나 붙인다.

- 버그: 논리 오류, 경계 조건, 회귀. 성능·시험 공백·유지보수성 지적도 여기에 적는다.
- 보안: `.fullops/guide/rules/common/security.md`.
- 준수: 지시서의 완료 기준과 증명 방법, 요구사항·설계 정본, UI 디자인 정본을 diff와 대조한다. 완료 보고와 검증 기록이 사실과 맞는지도 본다.

심각도와 처분:

- critical·high: 병합 전에 반드시 고친다. 미해결이면 `check`가 실패한다.
- medium: 실제 영향이 있지만 병합을 막을 정도는 아니다. 미해결이면 `report.md`의 검토 결론에 처분(병합 전 수정, 후속 과제 키, 사용자 결정)을 적는다.
- low: 영향이 작은 결함이나 개선 제안이다. 수락을 막지 않는다. 미해결이면 `PLANS.md`의 후속으로 남긴다.
- nit: 정확성에 영향이 없는 표기·명명·문구다. `severity`는 `low`로 적는다. 리뷰당 5건까지 적고 나머지는 `report.md`에 건수만 적는다.

## 리뷰 snapshot의 수명

- `fullops merge`가 끝나면 그 과제의 snapshot을 스스로 정리한다. 손으로 정리하려면 `fullops review cleanup <KEY> --finished`다. 리뷰어 터미널이 아직 살아 있으면 지우지 않고 사유만 남기며, 터미널이 닫힌 뒤의 `fullops merge`·`fullops settle`·`fullops review prune`이 마저 정리한다.
- 리뷰를 그만둘 때는 `fullops review cleanup <KEY> --abandon --reason "<사유>"`다. 살아 있는 터미널이 있거나 미커밋·미추적 파일이 있으면 지우지 않고 사유를 남긴다(git이 무시하는 `__pycache__` 같은 생성 캐시는 막지 않고 함께 지운다).
- 실패하거나 중단된 이전 SHA의 리뷰를 후속 리뷰가 대체했으면 `fullops review supersede <KEY> --old <head 12자> --new <head 12자>`로 잇는다. 새 SHA가 옛 SHA의 후손이고 새 리뷰가 수락이어야 한다. 옛 기록은 고치지 않는다.
- `fullops review prune`은 정리가 보류된 snapshot을 다시 정리해 보고, 사라진 snapshot의 기록을 닫고, 남은 것을 낸다.
- 리뷰 기록은 덮어쓰지 않는다. 같은 과제를 다시 리뷰하면 SHA가 달라 폴더가 달라진다.
