---
name: update-fullops
description: 사용자가 FullOps 업데이트 또는 활성 레포에서 update 수행을 요청하면 설치 버전 이후의 릴리스 내용을 확인하고 해당 레포에 필요한 변경을 적용한다.
---

# 릴리스 업데이트와 레포 적용

요청은 이 PC의 fullops 플러그인 갱신과 현재 활성 레포의 후속 적용을 함께 뜻한다. 다른 제품이나 라이브러리의 업데이트는 이 스킬의 대상이 아니다.

명령 표기: 이 문서의 `fullops`는 Claude Code 세션에서 그대로 실행된다. Codex·grok·agy 세션에서는 고정 위치 실행본을 전체 경로로 부른다: `~/.fullops/bin/fullops`(Windows는 `%USERPROFILE%\.fullops\bin\fullops.exe`).

1. 갱신 전의 버전을 확인해 적어 둔다: `fullops --version`(실행본), `fullops doctor`의 실행본 줄(고정 위치와 플러그인 루트), 레포의 `.fullops/fullops.toml`의 `applied_version`(레포 적용 버전). 버전을 추정하지 않는다. 진행 중인 worker와 사용자 변경은 건드리지 않는다.
2. 플러그인과 의존을 갱신한다: `fullops install`. 호스트마다 fullops 플러그인을 다시 등록하고 의존 플러그인과 사용자 스킬을 최신판으로 받는다. 다시 실행해도 안전하다. 새 통합 패키지 폴더를 받았으면 그 폴더의 실행본으로 실행한다: `<플러그인 폴더>/bin/fullops install`. Windows에서는 `<플러그인 폴더>\bin\fullops.exe install`이다.
3. 호스트별로 새 플러그인을 읽게 한다. 에이전트가 대신 할 수 없는 것은 사용자에게 요청한다.
   - Claude Code: 사용자에게 `/reload-plugins`를 요청한다. `CLAUDE.md`의 관리 블록이 바뀌었거나 세션 시작 안내를 새로 받아야 하면 새 세션으로 이어 간다.
   - Codex: hook 등록 문자열이 바뀐 갱신 뒤에는 사용자가 `/hooks`에서 fullops hook을 다시 신뢰해야 hook이 돈다. 새 세션에서 이어 간다.
   - grok: 새 세션에서 이어 간다. hook은 전역 등록 파일이 고정 위치 실행본을 부르므로 실행본만 갱신되면 된다.
   - agy: agy를 다시 시작한다. agy의 hook은 agy가 복사해 간 패키지의 실행본을 쓰므로 `fullops install`을 다시 하기 전에는 옛 버전으로 돈다.
4. `fullops install --check`와 `fullops doctor`로 빠진 것과 낡은 것이 없는지 본다. `doctor`의 '다른 홈' 행은 사용자에게 알리기만 하고 그 홈에 설치하지 않는다(설치는 사람이 정한다).
5. 레포 적용: `fullops upgrade`를 실행한다. "옛 자리(`.fullops-squad/`)다"라는 안내만 나오면 2.0.x의 레포다. `setup-fullops`의 "모드 전환과 이행"대로 `fullops migrate --layout`으로 먼저 옮기고(사람 승인 뒤) 다시 실행한다. `applied_version` 이후의 릴리스 노트와 필수 적용 검사의 결과가 나온다. 이 명령은 레포를 고치지 않는다.
6. 필수 적용 항목은 모두 적용한다. 해당 없음이나 보류로 분류해 넘기지 않는다. 적용할 수 없으면 멈추고 사유를 사용자에게 보고한다. 릴리스 노트의 나머지 항목은 오래된 것부터 읽고 이 레포에 적용, 해당 없음, 보류로 나눠 적용할 수 있는 것을 수행한다. 없는 템플릿 파일은 `fullops init`을 기존 역할로 다시 실행해 더한다(있는 파일은 고치지 않는다).
7. `.fullops/docs/exec-plans/phases/FULLOPS-UPDATE-<대상 버전>.md`에 갱신 전 버전, 레포 적용 버전, 대상 버전, 릴리스별 판정과 근거, 바꾼 파일, 검증, 보류의 담당과 재개 조건을 적는다.
8. `fullops upgrade --verify`가 종료 코드 0이면 `applied_version`이 올라간다. 통과하지 못한 필수 적용이 있으면 버전은 그대로이고 다음 요청에서 이어 간다. 버전은 낮추지 않는다.
9. 변경을 커밋하고 허가된 범위에서만 push와 워크트리 동기화(`fullops sync`)를 한다. 설치 버전, 레포 적용 버전, 반영한 내용, 검증, 보류와 기록 경로를 보고한다.

새 레포는 설치를 확인한 뒤 `setup-fullops`로 구성한다. Python판(`fullops-squad`)에서 넘어오는 레포의 전환은 `setup-fullops`의 "모드 전환과 이행"을 따른다.
