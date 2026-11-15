---
name: setup-fullops
description: 사용자가 현재 레포에 FullOps 하네스 setup 또는 초기화를 요청할 때 사용한다. 플러그인 설치만으로 실행하지 않는다.
---

# 레포별 setup

FullOps는 레포마다 켠다. 활성 표식은 `.fullops-squad/fullops.toml`이고 이 파일이 없는 레포에서는 hook이 아무 판정도 하지 않는다. setup은 사용자가 요청했을 때만 한다.

명령 표기: 이 문서의 `fullops`는 Claude Code 세션에서 그대로 실행된다. Codex·grok·agy 세션에서는 고정 위치 실행본을 전체 경로로 부른다: `~/.fullops/bin/fullops`(Windows는 `%USERPROFILE%\.fullops\bin\fullops.exe`).

## 0. 설치 확인

1. `fullops install --check`로 이 PC의 호스트마다 플러그인·의존 플러그인·사용자 스킬·설치 기록을 확인한다. 아무것도 설치하지 않는 명령이다. 빠진 것이 있으면 종료 코드가 2다.
2. 빠진 것이 있으면 사용자에게 알리고 승인을 받은 뒤 `fullops install`을 실행한다. 고정 위치 실행본이 아직 없는 첫 설치에서는 플러그인 폴더의 실행본으로 실행한다: `<플러그인 폴더>/bin/fullops install`. Windows에서는 `<플러그인 폴더>\bin\fullops.exe install`이다. 무엇을 부를지 먼저 보려면 `--dry-run`을 붙인다.
3. `fullops install`은 호스트 CLI의 설치 명령만 부른다. 신뢰는 사람이 한다. Codex는 세션에서 `/hooks`로 fullops hook을 한 번 신뢰한다. grok은 원본 레포의 폴더를 신뢰한다(`/hooks-trust`). agy는 다시 시작한다. 이 단계를 대신 하지 않고 사용자에게 알린다.
4. `fullops doctor`로 실행본, git·orca·curl, 호스트, 의존, hook 등록을 본다. 알림이 있으면 그 줄이 적은 명령을 따른다. 단 '다른 홈' 행은 사용자에게 알리기만 하고 그 홈에 설치하지 않는다(설치는 사람이 정한다).

## 1. 구성 정하기

- 운영 모드: `coor`(주 담당이 조율하고 역할 worker에게 위임한다)와 `dev`(주 담당이 직접 개발하고 필요한 전문가만 부른다) 가운데 고른다.
- 역할: 새 coor 구성은 `coor designer dev ops tester`를 먼저 제안하고 사용자가 더하거나 빼거나 이름을 바꾼 역할로 정한다. 고정 필수 역할은 없다. 역할 ID는 영문 소문자로 시작하고 소문자·숫자·밑줄·하이픈을 쓴다.
- 역할 종류: 설계 역할은 `design`, 조율 역할은 `coordinator`, 검증 역할은 `tester`, 그 밖은 `worker`다. `design`·`coordinator`·`tester`는 각각 하나까지다.
- 테스트 레벨: `minimal`·`lite`·`standard`·`full`·`exhaustive`. 새 레포의 기본은 `lite`다. 범위의 정본은 `.fullops-squad/rules/common/testing.md`다.
- 병렬 방침: `off`·`read`·`on`·`ultra`. 새 레포의 기본은 `on`(권장)이다. `on`은 나눌 수 있는 구현과 조사를 하위 에이전트로 동시에 진행하게 하고 사용량이 빨리 준다. `ultra`는 Claude Code worker를 울트라코드로 띄우며, worker의 권한 모드가 auto나 bypass가 아니면 Workflow 실행마다 승인 창에서 멈춘다. 정본은 `.fullops-squad/rules/delegation.md`다.
- 원격과 기준 브랜치: 기본은 `origin`과 `main`이다. 원격 없이 쓰려면 `--local-only`를 준다.

## 2. 초기화

```
fullops init --roles <역할,…> [--kind <역할>=<worker|design|tester|coordinator>]...
        [--mode coor|dev --primary-role <역할>] [--remote origin --base main | --local-only]
        [--test-level <레벨>] [--parallel off|read|on|ultra] [--dry-run]
```

1. `--dry-run`으로 만들 파일과 브랜치를 확인한 뒤 같은 명령에서 `--dry-run`을 빼고 실행한다.
2. `init`은 없는 파일만 만든다. 있는 파일은 고치지 않으므로 다시 실행하면 이어서 한다. `AGENTS.md`·`CLAUDE.md`·`GEMINI.md`에는 표식으로 감싼 관리 블록을 넣는다(없는 파일은 만든다). 원격 역할 브랜치 `fullops/<역할>`이 없으면 만들고 있는 브랜치는 옮기지 않는다. 커밋은 하지 않는다.
3. 역할 워크트리는 Orca로 만든다. 역할 브랜치 `fullops/<역할>`을 체크아웃한 워크트리가 있어야 그 역할에 배정할 수 있다.
4. 만든 파일을 확인하고 커밋한다.

## 3. 레포에 맞추기

1. `.fullops-squad/FULLOPS.md`와 `.fullops-squad/rules/common/README.md`, 거기 연결된 세 규칙을 읽고 프로젝트 정본을 `.fullops-squad/project.md`에 연결한다.
2. `.fullops-squad/lint/README.md`의 순서로 스택·패키지 매니저·기존 설정·CI를 확인해 `.fullops-squad/lint/lint.json`의 `commands`를 구성한다. 기존 사용자 설정은 보존한다.
3. `.fullops-squad/review/rule.json`에 제품별 리뷰 대상과 제외, 경로별 규칙 문장을 적는다.
4. `.fullops-squad/orca-agents.md`의 배정표에 역할마다 책임·워크트리·기동 방식을 적고, `## 모델 후보`에 역할마다 모델과 effort를 약한 것부터 적는다. 병합 전 독립 리뷰 세션의 모델은 `reviewer` 키로 적는다. 기계는 이 표를 읽지 않는다. lead가 읽고 고른 값을 `fullops route`·`fullops dispatch`가 기록한다.
5. UI 작업이 있는 레포는 `fullops-frontend`로 디자인 정본을 연결한다.

## 4. 모드 전환과 이행

- coor에서 dev로: `fullops mode dev --primary-role <역할>`. 떠 있는 세션은 정책 오류가 되므로 새 세션을 연다. dev에서 coor로 돌아가는 전환은 없다.
- Python판(`fullops-squad` 1.24.0)을 쓰던 레포: 역할 인박스를 모두 비우고 통합 대기를 정리한 과제 경계에서 `fullops migrate --from-python --dry-run`으로 보고서를 본다. 문제가 없으면 `--dry-run`을 빼고 실행한 뒤 변경과 보고서를 확인해 커밋한다. 되돌리기는 `fullops migrate --rollback`과 커밋이다.
- 기존 레포의 FullOps 갱신은 `update-fullops`를 쓴다.

## 5. 보고

만든 파일, 만든 원격 브랜치, 정하지 못한 배정, 확인하지 못한 것을 보고한다. 외부 키(Jev의 `TYPESAFE_API_KEY` 또는 `OPENROUTER_API_KEY`)는 `.fullops-squad/.env`나 환경 변수에 두며 값을 읽거나 출력하지 않는다. 키가 없으면 분류와 탐색이 로컬 후보로 동작한다는 것만 알린다.
