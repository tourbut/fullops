# 질문 전달과 사용자 결정 기록

`fullops-orca`의 참조 파일이다. worker 질문을 받았을 때나 사용자에게 물을 때 읽는다.

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
4. 사용자만 정할 수 있는 것(제품 규칙·범위, 항상 사람 승인인 네 가지)은 결정을 열고 사용자에게 묻는다: `fullops decision add --question "<질문>" --key <KEY> --blocks <KEY> --blocks <질문 메시지 ID>`. 질문 메시지는 `--ack`로 표시해 두고(답할 곳은 결정의 `blocks`에 남는다), 사용자에게는 같은 질문 글의 질문 카드로 묻는다(아래 "사용자 결정 기록". 같은 글이면 hook이 이 결정을 다시 쓴다). 사용자가 답하면 `fullops decision answer <ID> --answer "<답>"`으로 닫은 뒤 그 메시지 ID로 `reply`한다. 답을 받기 전에 추정으로 `reply`하지 않는다.

worker는 지시서가 정한 예외와 설계·범위·공유 계약·되돌릴 수 없는 일에서만 preamble의 `ask`로 묻는다. 질문에는 과제 키, 막힌 지점, 선택지와 각각의 영향을 적는다. 형제 worker에게 직접 보내지 않는다.

## 사용자 결정 기록

```
fullops decision add --question "<질문>" [--key <KEY>] [--option <선택지>]... [--blocks <과제 키|메시지 ID>]... [--sha <SHA>]
fullops decision answer <ID> --answer "<답>" [--by <사람>] [--sha <SHA>]
fullops decision drop <ID> --reason "<사유>"
fullops decision list [--open]
```

- 사용자에게 묻는 일은 질문 카드로만 한다(Claude Code는 `AskUserQuestion`). 글로 묻고 턴을 끝내지 않는다.
- 1번 선택지가 추천안이다. 그 label에 `(추천)`을 붙이고 이유는 설명에 적는다. Claude Code에서는 1번 선택지에 `(추천)`이 없는 카드를 hook이 거부한다.
- Claude Code에서는 카드를 내면 hook이 질문마다 열린 결정으로 적는다(질문 글, 선택지 순서, 질문 글에 적힌 과제 키). 따로 `decision add`를 하지 않는다. `--blocks`나 `--sha`가 필요한 결정만 먼저 `decision add`로 열고 같은 질문 글의 카드로 묻는다(같은 글이면 그 결정을 다시 쓴다).
- 답을 받으면 `fullops decision list --open`에서 ID를 보고 `fullops decision answer <ID> --answer "<고른 선택지>"`로 닫는다. 답하지 않은 카드는 열린 채 Orca 패널에 선택지와 함께 남는다.
- 새 세션에서 `fullops status`에 열린 결정이 나오면 같은 질문 글과 선택지의 카드로 다시 묻는다. 물을 일이 없어졌으면 `fullops decision drop <ID> --reason "<사유>"`로 닫는다.
- 그 밖의 호스트(Codex, grok, agy)는 묻기 전에 `fullops decision add --question "<질문>" --option "<1안 (추천)>" --option "<2안>"`으로 적고, 호스트에 질문 UI가 있으면 그것으로 묻는다.
- worker는 질문 카드를 쓰지 않는다(Claude Code에서는 hook이 거부한다). preamble의 `ask`로 lead에게 묻는다.
- 열린 결정은 `blocks`에 적힌 과제만 멈추고 나머지 과제는 계속한다.
- 항상 사용자 승인을 받는 것은 넷이다: 운영 배포, 운영 부하 테스트, backfill, 운영 데이터 정리. `PLANS.md`에는 `대기(사람: <무엇>)` 행으로 둔다.
- 되돌릴 수 있는 선택과 버그 수정 여부는 사용자에게 올리지 않는다.
- SHA를 고정한 결정은 24시간 뒤 만료되고 다시 묻는다. 카드의 답만으로는 닫히지 않는다: 사용자가 채팅에 답을 입력한 뒤 `decision answer`에 그 SHA를 주어 닫는다.
