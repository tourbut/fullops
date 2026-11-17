---
name: fullops-orca
description: FullOps setup이 완료된 레포에서 요청 라우팅·Orca worker 배정·bootstrap·dispatch·질문 전달·report·검토 및 병합을 운영할 때 사용한다.
---

# Orca 운영

lead(coor 모드의 coordinator, dev 모드의 주 담당)가 요청을 받아 분류하고, 역할 worker에게 배정하고, 완료 보고를 받아 병합하는 흐름이다. 배정·대기·완료 준비·병합은 `fullops` 명령이 Orca와 git을 감싸고 같은 명령이 상태를 쓴다. 날것 `orca orchestration worker-start`와 터미널에 지시서를 붙여 넣는 배정은 hook이 거부한다.

명령 표기: 이 문서의 `fullops`는 Claude Code 세션에서 그대로 실행된다. Codex·grok·agy 세션에서는 고정 위치 실행본을 전체 경로로 부른다: `~/.fullops/bin/fullops`(Windows는 `%USERPROFILE%\.fullops\bin\fullops.exe`).

세션을 시작하면 `fullops status`를 먼저 실행한다. 역할, 현재 인박스, 병렬 방침, 준비된 행, 열린 결정, 통합 대기와 보류, 열린 Run이 나온다. grok 세션과 hook이 돌지 않는 세션(agy의 데스크톱 앱 등)에서는 이 출력이 세션 시작 안내를 대신한다. 막힌 이유는 `fullops why`로, 규칙 전체는 `fullops rules`로 본다.

## bootstrap

역할 워크트리는 Orca로 만든다. 역할 브랜치는 `fullops/<역할>`이고 `fullops init`이 원격에 만들어 둔다. 그 브랜치를 체크아웃한 워크트리가 없으면 배정이 거절된다. 역할과 모델 후보는 `.fullops/guide/orca-agents.md`에 있다. grok worker는 원본 레포의 폴더 신뢰가 필요하고 사용자가 grok에서 승인한다.

역할 워크트리의 환경 파일(`.fullops/.env*`)은 `fullops dispatch`와 `fullops task recover`가 주 체크아웃에서 연결한다. 배정 전에 그 워크트리에서 직접 쓸 일이 있거나 연결이 빠졌으면 그 워크트리에서 `fullops env link`를 실행한다. 추적 파일과 이미 있는 파일은 건드리지 않는다.

## route: 요청을 받을 때

```
fullops route <KEY> (--request-file <경로> | --request "<글>")
        [--class direct|design|product|unresolved --role <역할> --reason "<근거>"]
        [--agent <CLI> --model <모델> --effort <effort> --why "<이유>"] [--deliverable <Dnn>]... [--force]
```

1. 과제 키를 정하고 `fullops route`로 분류한다. 결과는 `direct`(역할 worker에게 바로), `design`·`product`(설계 역할이 먼저), `unresolved` 가운데 하나다. 분류는 추천이다. 요청 글은 4,000자 이하로 요약한다.
2. 분류가 틀렸으면 `--force --class <값> --role <역할> --reason "<근거>"`로 바꾼다. `unresolved`는 배정할 수 없다.
3. 모델은 `.fullops/guide/orca-agents.md`의 모델 후보에서 고르고 `--agent --model --effort`를 셋 다 준다. 분류 없이 모델만 기록하려면 `fullops route <KEY> --tuple-only --role <역할|reviewer> --agent … --model … --effort …`다.
4. 분류만 하고 턴을 끝내면 종료가 막힌다. 배정하거나, 지금 할 수 없으면 `PLANS.md`에 대기 행으로 적는다(`fullops plan add`).
5. 지시서 작성은 `fullops-work`를 따른다. `design`·`product` 과제의 다른 역할 지시서는 설계 역할이 쓴다.

## dispatch

```
fullops dispatch <KEY> [--role <역할>] [--purpose build|fix|review|research] [--run <ID>]
        [--terminal <핸들>] [--agent <CLI> --model <모델> --effort <effort>] [--sha <40자>] [--reason limit] [--dry-run]
```

1. `fullops dispatch <KEY>`가 선행 조건을 검사하고, 준비 커밋을 만들어 게시하고, 대상 워크트리를 기본 브랜치에 맞추고, 환경 파일을 연결하고, Orca에 worker를 띄운다. 걸린 조건은 `FullOps[D-nn]`과 탈출구로 한꺼번에 나온다.
2. 자주 걸리는 것: 분류가 없다(`fullops route`), 인박스가 없거나 키가 다르다(`fullops task new`), 다른 키의 통합 대기가 있다(`fullops merge <그 키>` 또는 `fullops hold`), 그 역할에 끝나지 않은 다른 과제가 있다(끝난 뒤 배정하거나 `fullops plan add`), 대상 워크트리가 더럽다(그 worker의 완료 뒤 `fullops sync --role <역할>`).
3. 새 세션이 기본이다. 있는 터미널에 이어서 맡길 때만 `--terminal <핸들>`을 준다. 끝난 worker의 터미널은 Orca가 곧 놓으므로 거절되면 `--terminal`을 빼고 다시 배정한다.
4. 결과를 확인하지 못했다고 나오면 같은 명령을 다시 실행한다. 같은 요청 ID로 결과를 받으므로 worker가 두 번 뜨지 않는다. Orca가 실패 영수증을 냈으면 Orca의 `next`를 읽고 정한다. 다른 CLI나 호스트로 자동 전환하지 않는다.
5. 한도로 worker가 끝났으면 같은 모델을 기다렸다가 `fullops dispatch <KEY> --reason limit`으로 다시 배정한다. 과제당 2회까지이고 3회째는 결정이 열려 사용자에게 올라간다. 다른 모델로 바꾸는 재배정은 거절된다.
6. 출력된 다음 할 일을 따른다. "턴을 끝내면 Orca가 깨운다"가 나오면 턴을 끝낸다.
7. worker가 도중에 끊겼으면(터미널 종료 등) 다시 배정하기 전에 그 역할의 워크트리를 살린다: `fullops task recover --role <역할> --key <KEY> --repo <역할 워크트리>`. 미커밋 변경을 WIP 커밋으로 보존하고 기본 브랜치를 반영한다. 충돌하면 병합을 물리고 종료 코드 3으로 끝나므로 사람이 정한다.

## 대기

```
fullops wait [--run <ID>] [--ack <메시지 ID>]... [--max-minutes <N>] [--no-wait]
```

- worker가 실행 중이면 턴을 끝낸다. Orca가 메일이 올 때 lead를 깨운다. 깨어나면 `fullops wait --run <R>`로 받는다. Orca가 깨우는 세션에서는 이 명령이 기다리지 않고 받은 것만 돌려준다.
- Orca가 깨울 수 없는 세션(세션 핸들이 없는 세션)에서는 `fullops wait --run <R>`가 메시지가 올 때까지 기다린다. 대기 없이 턴을 끝내면 종료가 막힌다.
- 새 lead 세션은 `fullops wait --run <R>`로 이전 세션이 연 Run을 이어받는다. 열린 Run은 `fullops status`에 나온다.
- 반환값의 `actionable`이 처리할 메시지이고 `next`가 다음 할 일이다. heartbeat와 status는 세기만 한다.
- `ack_by`가 `merge`인 완료 보고는 `fullops merge <KEY>`가 처리 표시까지 한다. 그 밖(리뷰 보고, 실패 보고, 질문, escalation)은 처리한 뒤 `fullops wait --run <R> --ack <메시지 ID> --no-wait`로 표시한다. 표시하지 않은 메시지가 남으면 종료가 막힌다.

## 질문: lead가 전달한다

worker의 질문은 `fullops wait`의 `actionable`에 `question`으로 온다(메시지 ID, 과제, 역할, 본문). 답은 터미널에 붙여 넣지 않고 그 메시지 ID로 Orca에 답한다:

```
orca orchestration reply --id <질문 메시지 ID> --body "<답>"
fullops wait --run <R> --ack <질문 메시지 ID> --no-wait
```

`orca`는 이 세션의 Orca 실행 파일이다(`ORCA_CLI_COMMAND`가 있으면 그 값). Windows에서 `orca.cmd`는 `reply`를 거절하므로 `orca.exe`로 부른다. 20자를 넘는 `terminal send`는 hook이 거부한다.

1. 경로·핸들·순서·재시도 같은 운영 문제는 lead가 지시서와 정본을 근거로 바로 `reply`한다.
2. 설계·범위·공유 계약·지시서 해석 문제는 lead가 직접 답하지 않고 설계 역할에 넘긴다(`routing = "split"`이면 제품 규칙·범위·공유 제품 기준 질문만 제품 기획 역할에 넘기고, 기술 질문은 구현 담당이 스스로 정하게 돌려준다). 설계 역할이 없는 레포에서는 lead가 답한다.
   - 질문마다 새 키로 설계 과제를 분류하고 배정한다. 요청 글에 질문 원문, 질문 메시지 ID, 과제 키, 지시서 경로를 넣는다:
     `fullops route <KEY>-Q<번호> --request "<질문 원문 | 메시지 ID | 과제 키 | 지시서 경로>" --class design --role <설계 역할> --reason "worker 질문 전달" --agent <CLI> --model <모델> --effort <effort>`(split이면 `--class product`) 뒤 `fullops dispatch <KEY>-Q<번호> --role <설계 역할>`.
   - 설계 역할은 답을 `fullops done`이 출력한 본문 뒤에 이어 적어 `worker_done`으로 보낸다. 지시서를 고쳤으면 커밋하고 경로를 함께 적는다.
   - lead는 그 보고를 `fullops merge <KEY>-Q<번호>`로 처리하고, 답을 줄이거나 해석하지 않고 원래 질문의 메시지 ID로 `reply`한 뒤 질문을 `--ack`로 표시한다. 답이 지시서 변경을 담고 있으면 worker에게 병합된 SHA를 반영하라고 적는다.
3. 되돌릴 수 있는 디자인·연출·문구·배치 선택과 버그 수정 여부는 사용자에게 올리지 않는다. 설계 역할이 추천안으로 정하고, worker 자기 과제 안의 선택은 worker가 정해 완료 보고에 선택과 되돌리는 방법을 적는다.
4. 사용자만 정할 수 있는 것(제품 규칙·범위, 항상 사람 승인인 네 가지)은 결정을 열고 사용자에게 묻는다: `fullops decision add --question "<질문>" --key <KEY> --blocks <KEY> --blocks <질문 메시지 ID>`. 질문 메시지는 `--ack`로 표시해 두고(답할 곳은 결정의 `blocks`에 남는다), 사용자가 답하면 `fullops decision answer <ID> --answer "<답>"`으로 닫은 뒤 그 메시지 ID로 `reply`한다. 답을 받기 전에 추정으로 `reply`하지 않는다.

worker는 지시서가 정한 예외와 설계·범위·공유 계약·되돌릴 수 없는 일에서만 preamble의 `ask`로 묻는다. 질문에는 과제 키, 막힌 지점, 선택지와 각각의 영향을 적는다. 형제 worker에게 직접 보내지 않는다.

## 메시지

worker의 완료 보고는 `fullops done`이 출력한 본문으로 Orca preamble의 `worker_done`을 한 번 보내는 것이다(`fullops-work`). 날것 `worker_done`도 인정된다. 보고에 SHA가 없어 기록이 비면 `fullops completion set-sha <메시지 ID> <SHA> [--key <KEY>]`로 채운다.

## 사용자 결정 기록

```
fullops decision add --question "<질문>" [--key <KEY>] [--option <선택지>]... [--blocks <과제 키|메시지 ID>]... [--sha <SHA>]
fullops decision answer <ID> --answer "<답>" [--by <사람>] [--sha <SHA>]
fullops decision drop <ID> --reason "<사유>"
fullops decision list [--open]
```

- 사용자에게 묻기 전에 기록한다. 열린 결정은 `blocks`에 적힌 과제만 멈추고 나머지 과제는 계속한다.
- 항상 사용자 승인을 받는 것은 넷이다: 운영 배포, 운영 부하 테스트, backfill, 운영 데이터 정리. `PLANS.md`에는 `대기(사람: <무엇>)` 행으로 둔다.
- 되돌릴 수 있는 선택과 버그 수정 여부는 사용자에게 올리지 않는다.
- SHA를 고정한 결정은 24시간 뒤 만료되고 다시 묻는다.

## merge: 병합 책임자

```
fullops merge <KEY> [--completion <메시지 ID>] [--accept-missing-lint "<사유>"] [--no-push] [--continue | --abort]
```

1. 완료 보고마다 `fullops merge <KEY>`로 바로 처리한다. 리뷰 판정, lint 증거 확인, 병합, 병합 뒤 lint(트리가 달라졌을 때만), 병합 기록 커밋, push, 처리 표시, 역할 워크트리 동기화, 리뷰 snapshot 정리를 차례로 한다.
2. 종료 코드 3은 사람 손이 필요하다는 뜻이다. 대규모 변경이면 리뷰 결정이 열린다(`fullops-review`). 충돌이면 기본 워크트리에서 해결한 뒤 `fullops merge <KEY> --continue`, 포기는 `--abort`다.
3. 종료 코드 4(fetch·push 실패)는 다시 실행하면 남은 단계만 한다.
4. 병합 뒤 lint가 실패하면 병합이 되돌려진다. 수정 배정(`fullops task reopen` 뒤 `fullops dispatch <KEY> --purpose fix`)이나 보류로 간다.
5. 지금 병합하지 않을 보고는 `fullops hold <KEY> --reason "<사유>" --owner "<담당>" --resume "<재개 조건>"`으로 보류한다. 재개는 `fullops resume <KEY>`다. 보류는 병합 완료가 아니다.
6. 날것 `git merge`·`git push`로 통합해도 조상 관계로 인식한다. 원격에 이미 들어간 것을 한꺼번에 확인하려면 `fullops settle`을 쓴다.
7. 예약된 워크트리 동기화는 `fullops sync [--role <역할>]`이고 lead 자신의 브랜치는 `fullops sync --self`다. 강제 reset과 삭제는 쓰지 않는다.
8. 미통합 완료 보고를 두고 턴을 끝내면 종료가 막힌다. 병합하거나 보류한다.

## 설계 완료 뒤

설계 역할의 완료 보고(`[설계] <KEY> | … | 지시서: <경로>`)를 받으면 `fullops merge <KEY>`로 통합한다. 병합으로 lead 체크아웃에 들어온 지시서(`.fullops/inbox/to_<역할>.md`)마다 그 역할의 모델을 골라 배정한다:

```
fullops dispatch <KEY> --role <역할> --agent <CLI> --model <모델> --effort <effort>
```

지시서가 여러 역할에 걸쳐 있으면 역할마다 반복한다. tester의 지시서는 구현 역할의 완료 보고를 병합한 뒤에 배정한다. 설계 역할이 만든 지시서는 lead가 고쳐 쓰지 않는다(바꿀 것이 있으면 설계 역할에 다시 넘긴다).

## 다음 배정

병합 뒤에는 계속할지 사용자에게 묻지 않고 준비된 행을 배정한다. 준비된 행은 `fullops plan ready`로 본다(`merge`의 출력에도 나온다). 행마다 `fullops task new` → `fullops dispatch` 순서다. 준비된 행을 두고 턴을 끝내면 종료가 한 번 막힌다. `대기(사람: …)` 행은 건너뛰고 뒤 행을 배정한다. 사용자가 맡긴 범위는 `PLANS.md`의 `## 위임 범위`에 원문으로 있다.

## 병렬 진행

방침은 `fullops.toml`의 `parallel`이다(`fullops-work`의 "병렬 진행" 표, 정본은 `.fullops/guide/rules/delegation.md`).

- 서로 다른 워크트리가 필요한 독립 과제는 역할 단위로 배정한다. 방침이 `on`·`ultra`면 담당 역할이 서로 다른 준비된 행은 한 턴에 함께 배정한다.
- 같은 종류의 일을 동시에 여럿 돌리려면 역할(워크트리)을 늘린다(예: `dev1`, `dev2`). 역할당 인박스는 하나이고 과제 이름으로 된 지시서 파일은 만들지 않는다.
- worker는 spec의 `Parallel:`과 병렬 문장을 보고 하위 에이전트를 쓴다. Orca의 preamble은 task가 요구하지 않으면 하위 에이전트를 쓰지 말라고 하므로, 병렬 진행의 요청은 `fullops dispatch`가 spec에 넣는 문장으로 간다. spec을 손으로 고쳐 쓰지 않는다.
- worker의 하위 배정(`fullops dispatch <KEY> --brief "<글>"`)의 보고는 lead가 아니라 그 worker에게 간다. lead가 받는 완료 보고 하나가 하위 worker의 변경까지 담고, 통합은 `fullops merge` 한 번이다. lead는 `--brief`를 쓰지 않는다(lead의 지시는 인박스에 있다).
- lead의 선택형 위임(조사 `--purpose research`, 필수가 아닌 리뷰)은 방침이 `off`면 거절되고 `read`면 읽기 전용으로만 간다. 리뷰 판정이 요구한 독립 리뷰(`--purpose review --sha <SHA>`)는 방침과 무관하게 배정된다.
- `ultra`는 Claude Code worker를 새 터미널로 띄울 때 울트라코드로 띄운다. 사용량이 빨리 준다. worker의 권한 모드가 auto나 bypass가 아니면 Workflow 실행마다 승인 창에서 멈춘다.
- 리뷰와 테스트는 병렬 수단으로 늘리지 않는다.
