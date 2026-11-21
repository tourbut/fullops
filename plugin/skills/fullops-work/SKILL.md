---
name: fullops-work
description: FullOps setup이 완료된 레포에서 기능 개발을 역할별로 나누거나 worker 핸드오버 작성·완료 기록을 처리할 때 사용한다.
---

# 핸드오버와 작업 기록

활성 레포(`.fullops/fullops.toml`이 있는 레포)에서만 쓴다. 표식이 없으면 `setup-fullops`를 안내한다. 세션을 시작하면 `fullops status`를 먼저 실행한다. 역할, 현재 인박스, 병렬 방침, lead의 할 일이 거기 나온다.

명령 표기: 이 문서의 `fullops`는 Claude Code 세션에서 그대로 실행된다. Codex·grok·agy 세션에서는 고정 위치 실행본을 전체 경로로 부른다: `~/.fullops/bin/fullops`(Windows는 `%USERPROFILE%\.fullops\bin\fullops.exe`). 배정받은 worker는 spec의 끝 문장에 적힌 실행 명령을 그대로 쓴다.

worker는 받은 지시서와 거기 적힌 `먼저 읽을 문서`로 범위를 확인한다. 코드를 바꿀 때 `.fullops/guide/rules/common/README.md`와 연결된 규칙을, 프로젝트 정본이 필요할 때 `.fullops/guide/project.md`를 읽는다. 문서를 쓰거나 검토할 때는 `.fullops/guide/agents/document-writing.md`를 따른다. 이 본문은 흔한 경로만 담고 드문 경로는 참조 파일에서 필요할 때 읽는다(근거: 단일 에이전트 루프의 점진적 공개).

dev 모드의 주 담당은 기술 계획·구현·검증을 직접 한다. 직접 작업도 과제 키, 완료 조건, 검증 결과를 남기고 lint를 통과시킨다. 변경이 리뷰 기준을 넘으면 다른 세션의 독립 리뷰를 받는다(`fullops-review`).

## 역할 인박스 규칙

현재 작업의 정본은 역할마다 `.fullops/inbox/to_<역할>.md` 하나다. 파일 이름에는 역할만 쓰고 과제 키와 내용은 본문에 쓴다. 같은 과제의 후속 판단·수정·검증은 이 인박스에 반영한다.

1. 인박스는 `fullops task new --role <역할> --key <KEY> --goal "<한 줄 목표>" [--base <ref>]`로만 만든다. 파일을 직접 만들거나 과제 이름으로 된 지시서 파일을 만들지 않는다(hook이 거부한다). 먼저 `fullops route <KEY> --request "<요청>"`로 분류가 있어야 한다.
2. 그 역할의 인박스가 사용 중이면 새 과제는 `PLANS.md`의 과제 표에 행으로 둔다: `fullops plan add <KEY> --role <역할> --goal "<목표>" --source "<지시와 날짜>" [--after <선행 KEY>,…] [--status "<상태>"]`. 상태 기본값은 `대기`다. 사람을 기다리는 행은 상태를 `대기(사람: <무엇>)`으로 적는다. 그 행은 준비된 행에서 빠지고 뒤 행을 막지 않는다.
3. 이미 끝난 같은 키의 수정은 `fullops task reopen --role <역할> --key <같은 KEY> --goal "<수정 목표>"`로 연다. 차수가 하나 오르고 이전 보고는 `.fullops/tasks/<KEY>/`에 그대로 남는다. 수정 차수가 예산을 넘으면 명령이 알리고, 그때는 사용자에게 올린다.
4. 인박스는 worker의 `fullops done`만 없앤다. 진행 중인 인박스를 다른 과제로 덮어쓰거나 손으로 비우지 않는다.
5. 끝난 과제의 지시서와 완료 보고 전문은 `.fullops/tasks/<KEY>/<역할>.r<차수>.md`에 남는다. 그 과제의 분류 기록(`route.json`), 요청 원문(`request.md`), 리뷰(`reviews/`), QA 보고서(`qa/`)도 같은 폴더에 모인다.

## 작성과 전달

1. 코드와 운영 상태를 확인하고 과제 키와 주 역할을 정한다. 여러 역할에 걸치면 파일 소유권과 선행 조건을 나눈다. 과제 2개 이상이나 역할 2개 이상에 걸치는 묶음은 `.fullops/docs/planning/intents/<묶음 키>.md`를 `.fullops/templates/intent.md`로 쓰고 사용자 승인을 받은 뒤 배정한다.
2. `fullops task new`가 만든 인박스에 현재 상황과 확인 근거, 적용 기준과 예외, 먼저 읽을 문서, 해야 할 일과 파일 소유권, 완료 기준과 검증, 갱신할 산출물, 제약·협업·후속을 채운다. 파일 소유권·완료 기준·검증 명령 가운데 하나라도 정하지 못하면 `fullops route <KEY> --force --class design --role <설계 역할> --reason "<근거>"`로 설계 과제로 바꾼다.
3. 지시서 문장 규칙:
   - 목표와 완료 조건을 한 지시서에 모두 적고 끝까지 맡긴다. 잘게 나눠 매번 확인받게 하지 않는다.
   - 멈출 조건은 예외로만 적는다: 레포·워크트리 밖 변경, 데이터·파일 삭제, force-push, 파일 소유권 밖 수정, 승인 범위를 넘는 행위, 설명되지 않는 검증 실패.
   - 검증은 `.fullops/guide/rules/common/testing.md`의 테스트 레벨을 따르고 레벨 밖의 검사·캡처를 더하지 않는다.
   - "신중히 생각해" 같은 사고 지시를 넣지 않는다. 난이도는 모델과 effort 선택으로 조절한다.
4. UI 작업은 `fullops-frontend`, 코드 기반 모션그래픽 영상은 `fullops-motion`으로 정한 내용을 인박스에 연결한다. 영상 지시서에는 본문에 `템플릿: #<번호> <이름>` 한 줄을 적는다.
5. 배정은 `fullops-orca`의 dispatch를 따른다. 인박스는 `fullops dispatch`의 준비 커밋으로 worker의 워크트리에 전달된다.
6. 낯선 코드라 먼저 읽을 문서를 찾아야 하면 [explore.md](explore.md)를 읽는다(`fullops find`·`fullops context`, 선택).

## 작업과 완료

worker는 지시서의 완료 기준을 채울 때까지 진행 보고나 승인 대기 없이 작업한다. 지시서가 정한 예외와 설계·범위·공유 계약·되돌릴 수 없는 일에서만 멈추고 Orca preamble의 `ask`로 묻는다(과제 키, 막힌 지점, 선택지와 각각의 영향을 적는다. 추측으로 진행하지 않는다). 되돌릴 수 있는 디자인·문구·배치 선택과 버그 수정 여부는 묻지 않고 추천안으로 정한다. `해야 할 일` 항목을 끝낼 때마다 인박스에 체크한다.

순서는 구현 커밋 → lint → 완료 보고 작성 → `fullops done` → `worker_done` 전송이다.

1. 구현과 함께 영향을 받는 원천 문서를 `.fullops/docs/`에 갱신한다. 검증 결과의 정본은 `.fullops/tasks/<KEY>/qa/report.md` 하나다(`fullops-test`). 지시서의 `갱신할 산출물`은 `fullops-deliverables`로 갱신한다.
2. 새 코드 파일은 맨 위 주석이나 docstring 1~3줄에 무엇을 하는지 적는다(없으면 lint의 DOC-001 경고다).
3. 변경을 커밋하고 `fullops lint --from <기준>`을 통과시킨다. 코드를 바꾸고 통과 기록 없이 턴을 끝내면 종료가 한 번 막힌다(dev 직접 작업은 계속 막힌다). lint 통과 뒤 기록 경로(`.fullops/` 아래 `PLANS.md`, `inbox/`, `tasks/`)만 바꾼 커밋은 다시 돌리지 않아도 된다.
4. 인박스의 `## 완료 보고`에 변경 이유, 지시와 달라진 판단과 범위, 검증한 것과 못 한 것, 스스로 정한 선택과 되돌리는 방법, 산출물과 후속을 항목당 1~2줄로 쓴다.
5. `fullops done`을 실행한다. 완료 기록을 `.fullops/tasks/<KEY>/`로 옮겨 커밋하고, 원격이 있으면 역할 브랜치를 push하고, 보낼 본문 한 줄을 출력한다. 다른 역할이나 다른 과제로 넘길 일은 `--followup "<역할>: <글>"`로 준다(반복할 수 있다). 실패로 끝내려면 `--failed "<사유>"`를 준다. 적용할 검사가 없는 변경은 `--no-checks "<사유>"`를 준다.
6. 출력된 본문으로 Orca preamble의 `worker_done` 명령을 한 번 보낸다. `fullops done`은 보내지 않는다. `--failed`로 끝냈으면 preamble 명령에 `--outcome failed`를 쓴다.
7. 역할 메모 `.fullops/guide/roles/<역할>.md`에 남길 것이 있거나 `fullops doctor`가 메모를 알리면 [memo.md](memo.md)를 읽는다.

## 병렬 진행

방침은 `fullops.toml`의 `parallel`이고(기본 `read`) `fullops status`의 첫머리와 spec의 `Parallel:`에 나온다. 하위 에이전트나 하위 배정(`fullops dispatch <자기 KEY> --brief "<맡길 파일 묶음과 완료 조건>"`)을 쓸 때 정본 `.fullops/guide/rules/delegation.md`를 읽는다.

- 나눌 수 없는 일과 방침이 허용하지 않는 일은 직접 한다.
- 호스트의 워크트리 격리는 쓰지 않는다. 결과는 부모가 직접 확인해 이 워크트리에서 커밋하고 lint를 통과시킨다. 완료 보고는 부모 하나가 한다.
- 리뷰와 테스트를 하위 에이전트로 더 돌리지 않는다. 검증만 맡는 하위 에이전트도 띄우지 않는다. 등록된 명령은 `fullops lint`가 실행하고 증명 방법은 worker가 직접 실행한다.
