# fullops 패널

Orca 오른쪽 사이드바에서 fullops의 진행 상황을 보여 주는 플러그인이다. `<git-common-dir>/fullops/status.json`(설계 6.2.3절, `docs/formats.md` 4.4절)을 읽어 그리기만 하고 어떤 상태도 쓰지 않는다. hook·CLI와 따로 배포한다.

## 구성

| 파일 | 내용 |
|---|---|
| `orca-plugin.json` | 매니페스트. `execution`이 `trusted-worker`이고 `main`이 worker, 패널이 하나다. 권한은 `workspace:read` 하나만 청한다. |
| `src/worker.mjs` | Node worker. Orca의 워크스페이스 목록에서 Git 공용 디렉터리를 찾아 `status.json`을 읽고, 패널의 `plugin.request('status.read')`에 답한다. |
| `src/index.html` | 패널 화면. HTML·CSS·JS가 이 파일 하나에 들어 있고 빌드 단계가 없다. 색은 Orca의 `--orca-*` 변수를 따른다. |
| `src/preview.js` | 미리보기의 표본 데이터. `index.html`을 Orca 밖에서 열었을 때만 불린다. |
| `preview.html` | 미리보기. 밝은 테마와 어두운 테마, 그리고 파일이 없거나 깨진 경우를 한 화면에 늘어놓는다. |
| `test/` | `node --test`로 도는 시험. npm 의존이 없다. |

## 동작

- 패널(샌드박스 프레임)은 파일을 읽지 못한다. 그래서 패널이 5초마다 worker에게 묻고 worker가 파일을 읽어 답한다. 초점이 다른 워크스페이스로 옮겨 가면 바로 다시 묻는다.
- worker는 `workspace.list`가 준 경로마다 위로 올라가며 `.git`을 찾는다. 워크트리는 `.git` 파일의 `gitdir:`와 그 안의 `commondir`로 공용 디렉터리를 찾으므로, 한 레포의 워크트리 여럿은 한 건으로 모인다. git 프로세스는 띄우지 않는다.
- 활성 표식(`.fullops/fullops.toml`)도 `status.json`도 없는 레포는 보여 주지 않는다. fullops를 쓰는 레포가 여럿이면 초점이 있는 워크스페이스의 레포를 먼저 보여 주고 목록에서 고를 수 있다.
- `status.json`이 없으면 "아직 기록된 상태가 없다", JSON이 아니거나 `v`가 없으면 "읽지 못했다", `v`가 1보다 크면 "새 형식이다"를 보여 준다.
- 통합 대기 건은 `pending[]`의 `id`로 가리킨다(과제 키와 SHA는 없을 수 있다).
- 진행 중인 배정·과제·통합 대기·준비된 과제의 키를 누르면 그 과제의 글을 연다(`plugin.request('task.read', { repo, key, role })`). worker가 그 레포의 체크아웃마다(열린 워크스페이스와 원본 체크아웃) 읽는다: 지시서는 그 키의 `.fullops/inbox/to_<역할>.md` 가운데 가장 늦게 바뀐 것(worker가 자기 워크트리에 적는 진행이 보인다), 끝난 차수는 `tasks/<KEY>/<역할>.r<n>.md`의 가장 늦은 차수, 요청 원문 `tasks/<KEY>/request.md`, 분류 `tasks/<KEY>/route.json`의 class·role·reason. 키·역할은 `TaskKey`·`RoleId` 문법만 받고(경로가 될 수 없다), 심볼릭 링크는 읽지 않으며, 파일마다 앞 64KiB만 보낸다. 열린 동안 5초마다 다시 읽고, "← 목록"이나 Esc로 돌아간다.
- 사람의 결정을 기다리는 질문은 선택지와 함께 보여 준다. 1번 선택지의 label에 `(추천)`(또는 `(Recommended)`)이 있으면 그 표시를 "추천" 배지로 그린다. 결정의 과제 키도 눌러서 연다.
- 파일에서 온 글은 모두 이스케이프해 그린다.

## 확인

```sh
cd panel
node --test          # worker와 화면 시험
```

`preview.html`을 브라우저에서 열면 Orca 없이 화면을 볼 수 있다. 한 프레임만 보려면 `src/index.html?theme=dark`처럼 연다. `case=none|broken|newer|empty|quiet`로 상태를 고른다.

Orca에서의 확인은 사람이 한다. 플러그인은 코드를 실행하므로 올릴 때 승인이 필요하다(`orca plugin dev load`가 승인할 내용과 지문을 알려 준다).

```sh
orca plugin validate panel --json
orca plugin check panel --json
orca plugin dev load panel --json                    # approval_required와 지문이 나온다
orca plugin dev request panel status.read --json     # worker의 답을 본다(미정-M4의 확인)
orca plugin shot local.fullops-panel --panel status --theme dark --out panel-dark.png --json
```

- `shot`은 폴더 경로를 받지 않고 플러그인 키를 받는다(`Pass a plugin key such as acme.dusk`). 개발 폴더로 올린 패널의 키는 `local.fullops-panel`이다.
- 이 패널은 HTML 패널이라 `shot`이 페이지가 뜨자마자 찍는다. 처음 연 패널이나 방금 바뀐 `status.json`은 worker의 답이 그려지기 전일 수 있으니, 5초(폴링 간격) 넘게 기다렸다가 한 번 더 찍는다.
- Orca 창이 화면에 보이면 `--theme`이 먹지 않고 지금 테마로 찍힌다(`themeApplied: false`). 밝은 테마는 Orca 창을 숨기거나 내린 뒤 찍는다.

## 설치

패널은 저장소 `tourbut/fullops`의 카탈로그(루트의 `orca-catalog.json`)로 배포한다. Orca의 git 직접 설치(`orca plugin install tourbut/fullops`)는 저장소 루트만 읽으므로 이 하위 폴더의 패널을 설치하지 못한다. `fullops install`과 npx 실행기는 Orca를 건드리지 않는다. Orca가 떠 있고 설정에서 플러그인이 켜져 있어야 한다.

```sh
orca plugin catalog add tourbut/fullops --json         # approval_required와 cat1-… 지문이 나온다
orca plugin catalog add tourbut/fullops --approve <지문> --json
orca plugin install tourbut.fullops-panel --json       # 코드를 실행하므로 approval_required와 지문이 나온다
orca plugin install tourbut.fullops-panel --approve <지문> --json
```

- 두 번 모두 사람이 나온 내용(허가, publisher, 저장소)을 보고 Orca가 준 지문으로 다시 실행한다. 에이전트가 대신 승인하지 않는다.
- 갱신: 카탈로그에 새 release가 올라오면 Orca가 따라간다. 손으로는 `orca plugin update tourbut.fullops-panel`. 권한이나 패널이 늘면 다시 승인해야 하고, 승인 전에는 있던 버전이 돈다.
- 제거: `orca plugin uninstall tourbut.fullops-panel`(승인 필요 없음). 카탈로그까지 지우려면 `orca plugin catalog list --json`에서 그 카탈로그를 찾아 `orca plugin catalog rm <id|이름>`.
- 카탈로그 항목은 릴리스 워크플로가 `cargo xtask orca-catalog`로 만든다(`docs/build.md` '릴리스 워크플로'). 손으로 고치지 않는다.

## 확인한 것

- worker가 `node:fs`로 Git 공용 디렉터리를 읽는다(미정-M4). 2026-11-16 macOS의 Orca 1.4.261에서 개발 폴더로 올려 확인했다. 워크트리 `dev-mac`의 공용 디렉터리에 시험용 `status.json`을 두자 `dev request … status.read`가 `state: "ok"`와 그 내용을 돌려주었고, 같은 레포의 워크트리 둘(`main`, `dev-mac`)이 한 건으로 모였다. 패널은 정상 상태, 레포가 없는 상태, 깨진 JSON(`status.json을 읽지 못했다`와 파서 오류)을 그렸다.

## 확인하지 못한 것

- `workspace.readContext`의 `displayName`이 `workspace.list`의 `name`과 같은 값인가. 다르면 초점 레포를 찾지 못해 첫 레포를 보여 준다.
