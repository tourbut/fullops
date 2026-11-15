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
- 활성 표식(`.fullops-squad/fullops.toml`)도 `status.json`도 없는 레포는 보여 주지 않는다. fullops를 쓰는 레포가 여럿이면 초점이 있는 워크스페이스의 레포를 먼저 보여 주고 목록에서 고를 수 있다.
- `status.json`이 없으면 "아직 기록된 상태가 없다", JSON이 아니거나 `v`가 없으면 "읽지 못했다", `v`가 1보다 크면 "새 형식이다"를 보여 준다.
- 통합 대기 건은 `pending[]`의 `id`로 가리킨다(과제 키와 SHA는 없을 수 있다).
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
orca plugin shot panel --panel status --theme dark --out panel-dark.png --json
```

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

## 확인하지 못한 것

- worker가 `node:fs`로 Git 공용 디렉터리를 읽을 수 있는가(미정-M4). Orca 1.4.256의 코드로는 worker가 실행 인자 없이 `fork`된 Node 프로세스이고 권한 모델을 켜지 않으므로 읽을 수 있다고 본다. 실제로 돌려 보지는 않았다. 읽지 못하면 설계대로 `fullops status --json`을 띄우는 쪽으로 바꾼다.
- `workspace.readContext`의 `displayName`이 `workspace.list`의 `name`과 같은 값인가. 다르면 초점 레포를 찾지 못해 첫 레포를 보여 준다.
