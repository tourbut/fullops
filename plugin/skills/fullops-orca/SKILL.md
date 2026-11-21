---
name: fullops-orca
description: FullOps setup이 완료된 레포에서 요청 라우팅·Orca worker 배정·bootstrap·dispatch·질문 전달·report·검토 및 병합을 운영할 때 사용한다.
---

# Orca 운영

lead(coor 모드의 coordinator, dev 모드의 주 담당)가 요청을 받아 분류하고, 역할 worker에게 배정하고, 완료 보고를 받아 병합하는 흐름이다. 배정·대기·병합은 `fullops` 명령이 Orca와 git을 감싸고 상태를 쓴다. 날것 `orca orchestration worker-start`와 터미널에 지시서를 붙여 넣는 배정은 hook이 거부한다.

이 본문은 흔한 경로만 담고 드문 경로는 참조 파일에서 필요할 때 읽는다(근거: 단일 에이전트 루프의 점진적 공개).

명령 표기: 이 문서의 `fullops`는 Claude Code 세션에서 그대로 실행된다. Codex·grok·agy 세션에서는 고정 위치 실행본을 전체 경로로 부른다: `~/.fullops/bin/fullops`(Windows는 `%USERPROFILE%\.fullops\bin\fullops.exe`).

세션을 시작하면 `fullops status`를 먼저 실행한다. 역할, 현재 인박스, 병렬 방침, 준비된 행, 열린 결정, 통합 대기와 보류, 열린 Run이 나온다. 막힌 이유는 `fullops why`로, 규칙 전체는 `fullops rules`로 본다.

## route: 요청을 받을 때

```
fullops route <KEY> (--request-file <경로> | --request "<글>")
        [--class direct|design|product|unresolved --role <역할> --reason "<근거>"]
        [--agent <CLI> --model <모델> --effort <effort> --why "<이유>"] [--deliverable <Dnn>]... [--force]
```

1. 과제 키를 정하고 `fullops route`로 분류한다. 결과는 `direct`(역할 worker에게 바로), `design`·`product`(설계 역할이 먼저), `unresolved` 가운데 하나다. 분류는 추천이다. 요청 글은 4,000자 이하로 요약한다.
2. 분류가 틀렸으면 `--force --class <값> --role <역할> --reason "<근거>"`로 바꾼다. `unresolved`는 배정할 수 없다. `direct`는 파일 소유권·완료 기준·검증 명령이 요청만으로 분명하고 한 역할 안에서 끝나는 작업이다(버그 한 건, 문구·수치 변경, 기존 패턴을 따르는 기능, 에셋 교체). 설계 판단이 필요하거나 공유 계약(API·스키마·데이터 형식·씬 구조)을 바꾸거나 여러 역할에 걸치거나 요청이 모호하면 `design`이다. 지시서를 쓰다가 셋 중 하나라도 못 정하면 `design`으로 바꾼다. `routing = "split"`이면 확정된 제품 요구 안의 기술 작업은 기술 난도와 무관하게 담당 worker(`direct`)에, 제품 규칙·범위 변경만 제품 기획 역할(`product`)에 보낸다.
3. 모델은 `fullops.toml`의 `[models].<역할>` 후보에서 고르고 `--agent --model --effort`를 셋 다 준다. 목록 밖이면 `--why "<근거>"`를 붙인다. 고르는 기준과 리뷰 모델은 [models.md](models.md)에 있다.
4. 분류만 하고 턴을 끝내면 종료가 막힌다. 배정하거나, 지금 할 수 없으면 `PLANS.md`에 대기 행으로 적는다(`fullops plan add`).
5. 지시서 작성은 `fullops-work`를 따른다. `design`·`product` 과제의 다른 역할 지시서는 설계 역할이 쓴다.

## dispatch

```
fullops dispatch <KEY> [--role <역할>] [--purpose build|fix|review|research] [--run <ID>]
        [--terminal <핸들>] [--agent <CLI> --model <모델> --effort <effort> --why "<이유>"] [--sha <40자>] [--reason limit] [--dry-run]
```

1. `fullops dispatch <KEY>`가 선행 조건을 검사하고, 준비 커밋을 만들어 게시하고, 대상 워크트리를 기본 브랜치에 맞추고, 환경 파일을 연결하고, Orca에 worker를 띄운다. 걸린 조건은 `FullOps[D-nn]`과 탈출구로 한꺼번에 나온다.
2. 거절되면 출력의 탈출구를 따른다. 자주 걸리는 조건과 해결은 [recovery.md](recovery.md)에 있다.
3. 새 세션이 기본이다. 있는 터미널에 이어서 맡길 때만 `--terminal <핸들>`을 준다.
4. 결과를 확인하지 못했다고 나오면 같은 명령을 다시 실행한다. worker는 두 번 뜨지 않는다. Orca가 실패 영수증을 냈으면 Orca의 `next`를 읽고 정한다. 다른 CLI나 호스트로 자동 전환하지 않는다.
5. 한도로 worker가 끝났으면 같은 모델을 기다렸다가 `fullops dispatch <KEY> --reason limit`으로 다시 배정한다. 과제당 2회까지이고 3회째는 결정이 열려 사용자에게 올라간다. 다른 모델로 바꾸는 재배정은 거절된다.
6. 출력된 다음 할 일을 따른다. "턴을 끝내면 Orca가 깨운다"가 나오면 턴을 끝낸다.
7. worker가 도중에 끊겼으면 다시 배정하기 전에 [recovery.md](recovery.md)대로 워크트리를 살린다(`fullops task recover`).

## 대기

```
fullops wait [--run <ID>] [--ack <메시지 ID>]... [--max-minutes <N>] [--no-wait]
```

- worker가 실행 중이면 턴을 끝낸다. Orca가 메일이 올 때 lead를 깨운다. 깨어나면 `fullops wait --run <R>`로 받는다. Orca가 깨우는 세션에서는 이 명령이 기다리지 않고 받은 것만 돌려준다.
- Orca가 깨울 수 없는 세션(세션 핸들이 없는 세션)에서는 `fullops wait --run <R>`가 메시지가 올 때까지 기다린다. 대기 없이 턴을 끝내면 종료가 막힌다.
- 새 lead 세션은 `fullops wait --run <R>`로 이전 세션이 연 Run을 이어받는다. 열린 Run은 `fullops status`에 나온다.
- 반환값의 `actionable`이 처리할 메시지이고 `next`가 다음 할 일이다. heartbeat와 status는 세기만 한다.
- `ack_by`가 `merge`인 완료 보고는 `fullops merge <KEY>`가 처리 표시까지 한다. 그 밖(리뷰 보고, 실패 보고, 질문, escalation)은 처리한 뒤 `fullops wait --run <R> --ack <메시지 ID> --no-wait`로 표시한다. 표시하지 않은 메시지가 남으면 종료가 막힌다.

## 질문과 사용자 결정

worker 질문(`fullops wait`의 `actionable`에 `question`)을 받았거나 사용자에게 물을 일이 생기면 [questions.md](questions.md)를 읽는다. 요점: 답은 그 메시지 ID로 `orca orchestration reply`한 뒤 `--ack`로 표시한다. 사용자에게는 질문 카드로만 묻고 답은 `fullops decision answer <ID> --answer "<답>"`으로 닫는다.

## 메시지

worker의 완료 보고는 `fullops done`이 출력한 본문으로 Orca preamble의 `worker_done`을 한 번 보내는 것이다(`fullops-work`). 날것 `worker_done`도 인정된다. 보고에 SHA가 없어 기록이 비면 `fullops completion set-sha <메시지 ID> <SHA> [--key <KEY>]`로 채운다.

## merge: 병합 책임자

```
fullops merge <KEY> [--completion <메시지 ID>] [--accept-missing-lint "<사유>"] [--no-push] [--continue | --abort]
```

1. 완료 보고마다 `fullops merge <KEY>`로 바로 처리한다. 리뷰 판정, lint 증거 확인, 병합, 병합 뒤 lint(트리가 달라졌을 때만), 병합 기록 커밋, push, 처리 표시, 역할 워크트리 동기화, 리뷰 snapshot 정리를 차례로 한다.
2. 종료 코드 3은 사람 손이 필요하다는 뜻이다. 대규모 변경이면 리뷰 결정이 열린다(`fullops-review`). 충돌이면 기본 워크트리에서 해결한 뒤 `fullops merge <KEY> --continue`, 포기는 `--abort`다.
3. 종료 코드 4(fetch·push 실패)는 다시 실행하면 남은 단계만 한다.
4. 병합 뒤 lint가 실패하면 병합이 되돌려진다. 수정 배정(`fullops task reopen` 뒤 `fullops dispatch <KEY> --purpose fix`)이나 보류로 간다.
5. 지금 병합하지 않을 보고는 `fullops hold <KEY> --reason "<사유>" --owner "<담당>" --resume "<재개 조건>"`으로 보류한다. 재개는 `fullops resume <KEY>`다. 보류는 병합 완료가 아니다.
6. 예약된 워크트리 동기화는 `fullops sync [--role <역할>]`이고 lead 자신의 브랜치는 `fullops sync --self`다. 강제 reset과 삭제는 쓰지 않는다.
7. 미통합 완료 보고를 두고 턴을 끝내면 종료가 막힌다. 병합하거나 보류한다.

## 설계 완료 뒤

설계 역할의 완료 보고(`[설계] <KEY> | … | 지시서: <경로>`)를 받으면 `fullops merge <KEY>`로 통합한다. 병합으로 lead 체크아웃에 들어온 지시서(`.fullops/inbox/to_<역할>.md`)마다 그 역할의 모델을 골라 배정한다:

```
fullops dispatch <KEY> --role <역할> --agent <CLI> --model <모델> --effort <effort>
```

지시서가 여러 역할에 걸쳐 있으면 역할마다 반복한다. tester의 지시서는 구현 역할의 완료 보고를 병합한 뒤에 배정한다. 설계 역할이 만든 지시서는 lead가 고쳐 쓰지 않는다(바꿀 것이 있으면 설계 역할에 다시 넘긴다).

## 다음 배정

병합 뒤에는 계속할지 사용자에게 묻지 않고 준비된 행을 배정한다. 준비된 행은 `fullops plan ready`로 본다(`merge`의 출력에도 나온다). 행마다 `fullops task new` → `fullops dispatch` 순서다. 준비된 행을 두고 턴을 끝내면 종료가 한 번 막힌다. `대기(사람: …)` 행은 건너뛰고 뒤 행을 배정한다. 사용자가 맡긴 범위는 `PLANS.md`의 `## 위임 범위`에 원문으로 있다.

## 병렬 진행

방침은 `fullops.toml`의 `parallel`이다(기본 `read`). 하위 에이전트·하위 배정·울트라코드의 규칙은 `.fullops/guide/rules/delegation.md`에 있고 쓸 때 읽는다.

- 서로 다른 워크트리가 필요한 독립 과제는 역할 단위로 배정한다. 방침이 `off`가 아니면 담당 역할이 서로 다른 준비된 행은 한 턴에 함께 배정한다.
- worker에게 가는 병렬 진행의 요청은 `fullops dispatch`가 spec에 넣는 문장이다. spec을 손으로 고쳐 쓰지 않고, lead는 `--brief`를 쓰지 않는다.
- lead의 선택형 위임(조사 `--purpose research`, 필수가 아닌 리뷰)은 방침이 `off`면 거절되고 `read`면 읽기 전용으로만 간다. 리뷰 판정이 요구한 독립 리뷰(`--purpose review --sha <SHA>`)는 방침과 무관하게 배정된다.
