# fullops 설치

fullops는 호스트 hook과 명령이 한 실행 파일(`fullops`)인 플러그인이다. 지원하는 호스트는 Claude Code, Codex, Grok Build(`grok`), Antigravity CLI(`agy`) 넷이다. 설치는 `fullops install` 하나가 한다. 이 PC의 PATH에 있는 호스트마다 fullops 플러그인을 등록하고, 의존 플러그인과 사용자 스킬을 설치한다.

## 준비물

- git 2.41 이상, Orca 1.4.256 이상.
- 쓰려는 호스트 CLI가 설치돼 있고 로그인돼 있다. fullops는 호스트 CLI 자체를 설치하거나 로그인하지 않는다. Claude Code는 2.1.295 이상이어야 한다.
- 사용자 스킬 설치에는 Node의 `npx`가 필요하다. 없으면 스킬 단계만 건너뛴다.
- Jev 분류와 탐색에는 `curl`과 키(`TYPESAFE_API_KEY` 또는 `OPENROUTER_API_KEY`)를 쓴다. 없으면 로컬 후보로 동작한다.

## 처음 설치하는 길

길은 둘이고 결과가 같다.

1. Claude Code나 Codex의 마켓플레이스로 fullops를 설치한 뒤 `fullops install`을 한 번 실행한다.
   - Claude Code: `claude plugin marketplace add tourbut/fullops` 뒤 `claude plugin install fullops@fullops`. 그다음 Claude Code 세션에서 `fullops install`을 실행한다.
   - Codex: `codex plugin marketplace add tourbut/fullops` 뒤 `codex plugin add fullops@fullops`. 그다음 `<플러그인 폴더>/bin/fullops install`을 실행한다.
2. 통합 패키지 폴더(이 폴더)를 받아 그 안의 실행본으로 설치한다: `<플러그인 폴더>/bin/fullops install`. grok이나 agy만 쓰는 PC는 이 길로 설치한다.

Windows에서는 `<플러그인 폴더>\bin\fullops.exe install`을 실행한다. `bin/fullops`는 macOS·Linux용 런처이고 PowerShell은 확장자 없는 경로를 실행하지 못한다.

설치가 끝나면 고정 위치 실행본이 `~/.fullops/bin/fullops`(Windows는 `%USERPROFILE%\.fullops\bin\fullops.exe`)에 생긴다. Claude Code 세션에서는 `fullops`라는 이름이 그대로 실행된다. Codex·grok·agy 세션과 호스트 밖 터미널에서는 고정 위치 실행본을 전체 경로로 부른다. fullops는 사용자 PATH를 바꾸지 않는다.

## `fullops install`이 하는 일

```
fullops install [--hosts <claude,codex,grok,agy>|all] [--only runtime|plugin|deps]... [--dry-run] [--json]
fullops install --check [--hosts …] [--json]
fullops install --prune | --rollback | --remove-hooks
```

| 단계 | 하는 일 | 네트워크 |
|---|---|---|
| `runtime` | 실행본과 의존 목록(`deps.toml`)을 `~/.fullops/`에 둔다 | 없음 |
| `plugin` | 호스트마다 fullops 플러그인을 등록한다. Claude Code와 Codex는 마켓플레이스로, grok과 agy는 이 패키지의 재료로 조립한 폴더(`~/.fullops/package/<버전>/`)의 경로로 설치한다. grok은 이어서 hook 등록 파일을 쓴다 | 있음 |
| `deps` | 호스트마다 의존 플러그인을 설치하고, 사용자 스킬을 스킬 설치 도구(`npx skills`)로 설치한다 | 있음 |

- `--dry-run`은 부를 명령과 쓸 파일의 경로만 낸다. 아무것도 부르지 않고 쓰지 않는다. 먼저 이것으로 계획을 본다.
- `--hosts`로 호스트를 고른다. 고른 호스트의 CLI가 PATH에 없으면 그 호스트를 건너뛰고 알린다.
- 실패한 명령이 있으면 호스트별 결과를 내고 종료 코드 4로 끝난다. 원인을 고친 뒤 다시 실행한다. 다시 실행해도 안전하다.
- 이미 등록된 마켓플레이스의 출처가 의존 목록의 출처와 다르면 그 호스트의 설치를 멈추고 알린다. 덮어쓰지 않는다.
- 스킬을 다시 설치하기 전에 있던 스킬 폴더는 `~/.fullops/skill-backups/`에 한 번 복사해 둔다. 이름을 `*`로 받는 묶음은 어느 스킬을 덮어쓸지 알 수 없어 그 호스트의 스킬 폴더에 있는 스킬을 모두 복사한다. 복사하지 못한 묶음은 설치하지 않고 실패로 적는다.
- `FULLOPS_OFFLINE=1`이면 `runtime` 단계만 하고 네트워크 단계를 건너뛴다. 망이 열린 곳에서 다시 실행한다.
- `--check`는 아무것도 설치하지 않고 빠진 플러그인·스킬, 낡은 설치 기록, grok의 hook 등록을 낸다. 빠진 것이 있으면 종료 코드 2다. `fullops doctor`가 같은 확인을 포함한다.
- `--prune`은 옛 실행본, 스킬 백업, 지난 버전의 조립 폴더를 지운다. `--rollback`은 실행본을 가장 최근 옛 파일로 되돌리고 그 버전에 고정한다(세션 시작의 자동 설치가 다시 올리지 않는다). 다음 `fullops install`이 고정을 푼다. 이 둘과 `--remove-hooks`에는 미리 보기가 없다. `--dry-run`이나 `--check`와 함께 주면 아무것도 하지 않고 거절한다.

## fullops가 호스트 쪽에 하는 것과 하지 않는 것

- 호스트 CLI에는 설치와 목록 명령만 부른다. `--trust` 같은 신뢰 인자를 넘기지 않고 npm 전역 설치를 하지 않는다.
- 호스트 쪽 폴더에 fullops가 직접 쓰는 파일은 하나다: grok의 전역 hook 폴더의 `<GROK_HOME 또는 ~/.grok>/hooks/fullops.json`. 계획과 결과에 경로가 나오고 `fullops install --remove-hooks`가 지운다. 이미 있는 호스트 설정 파일은 고치지 않는다.
- 플러그인 제거는 사람이 호스트의 명령으로 한다. grok은 플러그인을 지워도 hook 등록 파일이 남으므로 `fullops install --remove-hooks`를 함께 실행한다.

## 설치 뒤에 사람이 하는 일

| 호스트 | 할 일 |
|---|---|
| Claude Code | 떠 있는 세션은 `/reload-plugins`를 하거나 새 세션을 연다 |
| Codex | 세션에서 `/hooks`로 fullops hook을 한 번 신뢰한다. 신뢰하기 전에는 hook이 돌지 않는다. hook 등록 문자열이 바뀐 갱신 뒤에도 다시 신뢰한다 |
| grok | hook과 worker는 원본 레포의 폴더 신뢰가 필요하다(`/hooks-trust`). fullops 플러그인을 신뢰해야 Context7 MCP가 켜진다. grok에서는 세션 시작 안내가 모델에게 닿지 않으므로 세션을 시작하면 `fullops status`를 먼저 실행한다(`AGENTS.md`의 관리 블록이 같은 내용을 적는다) |
| agy | agy를 다시 시작해야 플러그인이 보인다. agy의 hook은 CLI에서만 돈다(IDE와 데스크톱 앱에서는 돌지 않는다) |

그다음 `fullops doctor`로 실행본, 바깥 도구, 호스트, 의존, hook 등록을 확인한다.

## 함께 설치되는 것

목록의 정본은 이 폴더의 `deps.toml`이다. 버전을 고정하지 않고 설치 시점의 최신판을 받는다.

| 종류 | 이름 | 호스트 |
|---|---|---|
| 플러그인 | ponytail | Claude Code, Codex |
| 플러그인 | mattpocock-skills | Claude Code(그 밖의 호스트에는 같은 것을 스킬로 넣는다) |
| 플러그인 | pstack | Claude Code만 |
| 사용자 스킬 | caveman, typesafe-ai, diagram-design, frontend-design, ui-ux-pro-max, impeccable, web-design-guidelines | 네 호스트 |
| MCP | Context7(원격 HTTP) | 네 호스트 |
| 선택형 | playwright-cli, impeccable 엔진, shadcn, cuawright-web | 설치하지 않는다. 안내만 하고 쓰는 레포가 직접 설치한다 |

pstack 주의: pstack의 "사람을 기다리지 않는다"와 "다른 모델로 재시도" 규칙은 FullOps 규칙과 다르다. FullOps를 적용한 레포에서는 FullOps 규칙이 우선한다.

## 레포에 적용

설치만으로는 아무 레포에서도 동작하지 않는다. 레포마다 `setup-fullops` 스킬로 켠다(`fullops init`). Python판(`fullops-squad` 1.24.0)을 쓰던 레포는 `fullops migrate --from-python`으로 넘어온다. 두 플러그인은 이름과 활성 표식이 달라 한 PC에 함께 있어도 서로 간섭하지 않는다.

## 갱신과 디렉터리판

- 갱신은 새 패키지로 `fullops install`을 다시 실행하는 것이다. agy에는 갱신 명령이 없고 grok의 로컬 경로 설치는 자동 갱신되지 않으므로 두 호스트는 이 명령으로만 갱신된다. 레포 적용은 `update-fullops` 스킬(`fullops upgrade`)을 따른다.
- 디렉터리판(Anthropic 플러그인 디렉터리로 받은 Claude Code 전용 폴더)은 자기 폴더 밖에 쓰지 않는다. hook이 플러그인 폴더의 실행본을 그대로 쓰고 실행본을 고정 위치로 복사하지 않으며 의존도 설치하지 않는다. 의존과 나머지 호스트가 필요하면 사람이 `<플러그인 폴더>/bin/fullops install`(Windows는 `<플러그인 폴더>\bin\fullops.exe install`)을 실행한다.
