# fullops 2.0.0 설치와 옛 판(fullops-squad 1.24.0) 정리 안내

작성: 2026-11-15. 상태: 초안. 레포 주인의 결정(5절)을 기다린다. 대상: 레포 주인(사람). 이 PC의 실제 호스트에서는 읽기 전용 확인(목록·조회 명령, 파일 읽기)만 했다. 이 문서의 설치·제거·전환 절차는 하나도 실행하지 않았고, 이 문서 말고 바꾼 파일은 없다.

표기
- **[확인]**: 코드·문서·이 PC에서 실행한 출력으로 확인한 것. 근거를 괄호에 적는다.
- **[추정]**: 확인하지 않았다. 무엇에서 미루어 봤는지 적는다.
- **실환경 미확인**: 실제 호스트·Orca에서 돌려 본 적이 없는 단계다.
- 경로의 `dev/`는 `C:/Users/shin/orca/workspaces/fullops/dev`, `old/`는 `D:/workspace/Fullops-Squad-plugin`이다.

---

## 0. 요약

1. 새 판은 레포 주인의 결정대로 `npx --yes github:tourbut/fullops install`로 설치한다. npx로 받는 것은 작은 실행기(.cjs)다. 실행기는 바이너리를 고르고 검증한 뒤 `fullops install`에 넘기기만 하고, 설치 로직은 Rust에 그대로 둔다.
2. npx와 Claude Code·Codex 마켓플레이스는 둘 다 `tourbut/fullops`의 **기본 브랜치(main)** 를 읽는다. 그래서 main에 `package.json`, 실행기, 마켓플레이스 카탈로그 둘, 그리고 바이너리가 든 `dist/native/fullops`가 있어야 한다. **추천: 릴리스 때만 main에 `dist/native/fullops`를 커밋한다.** 지금 main에는 이것들이 없다(`.gitignore`의 `/dist/`). 그래서 지금은 npx도 마켓플레이스 설치도 동작하지 않는다.
3. **Orca 패널(fullops-panel)도 따로 배포해야 한다.** 패널은 `panel/` 하위 폴더에 있다. 그런데 Orca의 git 직접 설치(`orca plugin install tourbut/fullops`)는 저장소 루트만 설치한다. 그래서 카탈로그(`orca-catalog.json`의 `source.path: "panel"`)를 거치거나, 패널을 루트에 둔 별도 저장소를 만들어야 한다. 패널은 코드를 실행하는 플러그인이라 설치할 때 사람이 Orca에서 승인(`--approve <지문>`)해야 한다. npx 실행기는 패널 설치 명령을 안내만 한다.
4. 옛 판은 여러 레포에서 계속 돈다. 두 판은 플러그인 이름과 레포 표식이 달라 함께 있어도 된다. 레포를 하나씩 `fullops migrate --from-python`으로 옮긴다. 옛 표식 `.fullops-squad/fullops.json`이 있는 곳은 다음과 같다 [확인: 파일 목록]:
   - `D:/workspace`의 레포 6개: `erden-godot`, `erden-unity`, `erden-s06-click-gui-sandbox-20261006`, `KnowsLink`, `jevpet`, `Fullops-Squad-plugin`(옛 판 원본 저장소).
   - `C:/Users/shin/orca/workspaces`의 워크트리 15개: `erden-godot/{arch,art,coor,dev,dev_harness,ops,tester}`, `KnowsLink/{coor,designer,dev,ops,tester}`, `KnowsLink/.fullops-review-…`, `jevpet/notify-nori-relay`, `Fullops-Squad-plugin/dev`. 이것들은 `D:/workspace` 레포의 git 워크트리다. 표식은 브랜치마다 있으므로 주 체크아웃을 옮겨도 각 워크트리의 브랜치가 전환 커밋을 받기 전까지는 옛 표식이 남는다.
   
   모든 레포와 워크트리를 옮기고 2주가 지난 뒤에 PC의 옛 판을 지운다. 옛 판 원본 저장소(`Fullops-Squad-plugin`)를 어떻게 할지는 5절 D8에서 정한다.
5. 릴리스, 태그, main에 바이너리 커밋, 카탈로그 공개는 모두 **사람 승인이 필요**하다. 이 문서는 그 순서만 제안한다.

---

## 1. 새 판 설치 방법

### 1.1 배포 구조: main에 무엇을 두나

`npx --yes github:tourbut/fullops`는 기본 브랜치 HEAD의 tarball을 받는다. 그 안의 `package.json`이 `bin`으로 가리키는 파일을 실행한다 [추정: npm의 git 의존 처리. 이 PC에서 npx는 실행하지 않았다]. Claude Code·Codex의 `marketplace add tourbut/fullops`도 같은 저장소를 읽는다(`dev/plugin/deps.toml` `[self].source = "tourbut/fullops"`, `dev/docs/design.md:2807`). 두 호스트가 브랜치나 ref를 받는지는 확인하지 않았다.

main 루트에 둘 것(제안):

| 경로 | 내용 | 지금 |
|---|---|---|
| `package.json` | `name: "fullops"`, `private: true`, `bin: { "fullops": "npx/fullops.cjs" }`, `files: ["npx/", "dist/native/fullops/"]`, `engines: { "node": ">=20.18.1" }`. 실행 의존 없음. **`prepare` 스크립트를 두지 않는다**(있으면 npx가 받을 때 devDependencies를 설치하고 실행한다 [추정]) | 없음 [확인: 루트 `ls`] |
| `npx/fullops.cjs` | 실행기(1.3절) | 없음 |
| `.claude-plugin/marketplace.json`, `.agents/plugins/marketplace.json` | `cargo xtask dist`가 만드는 카탈로그 둘. 플러그인 출처 `./dist/native/fullops` | 없음 [확인: `dev/docs/build.md:35`, design.md:2753] |
| `dist/native/fullops/` | 통합 패키지. 바이너리 넷(`fullops.exe`, `fullops-darwin-arm64`, `fullops-darwin-x64`, `fullops-linux-x64`), `bin/fullops` 런처, `SHA256SUMS`, `VERSION`, skills, hosts 등 | 없음. `.gitignore`의 `/dist/` [확인] |
| `orca-catalog.json`(패널을 카탈로그로 배포할 때) | 1.5절 | 없음 |

옛 판의 루트 `package.json`도 같은 꼴이었다: `bin`은 `bin/fullops-squad.cjs`, `files`는 `[bin/, dependencies.json]`, `engines`는 `node>=20.18.1`, `private: true` [확인: `old/package.json`].

### 1.2 바이너리를 어디서 얻나: 후보 비교

| 안 | 사용자 명령 | 좋은 점 | 나쁜 점 |
|---|---|---|---|
| **(가) main에 `dist/native/fullops`를 커밋(릴리스 때만)** | `npx --yes github:tourbut/fullops install` | npx와 Claude Code·Codex 마켓플레이스가 **같은 트리 한 벌**을 읽는다. 지금 코드(`[self].source = "tourbut/fullops"`, 카탈로그 `./dist/native/fullops`)를 바꿀 필요가 없다. 받은 뒤에는 따로 내려받을 것이 없다 | 릴리스마다 바이너리 네 개(하나 약 9MB [확인: realcheck의 `fullops.exe` 9.0M], 합계 압축 전 36MB 이상 [추정])가 이력에 쌓이고 지울 수 없다. npx와 마켓플레이스 클론이 매번 그만큼 받는다. `.gitignore`와 H16의 기본안(배포 전용 브랜치)을 바꿔야 한다 |
| (나) 같은 저장소의 GitHub Releases에서 받기 | 같음 | tarball이 작고 이력이 커지지 않는다 | 실행기가 네트워크로 바이너리를 받는다(실행기가 커지고 오프라인 설치가 깨진다). Claude Code·Codex 마켓플레이스는 어차피 git 트리에 바이너리가 든 패키지가 있어야 하므로 **바이너리가 두 곳**에 생긴다. 카탈로그가 다른 ref나 URL을 가리킬 수 있는지는 확인하지 않았다 |
| (다) 배포 브랜치를 `#ref`로 가리키기 | `npx --yes github:tourbut/fullops#dist install` | main이 깨끗하다. H3·H16 기본안과 맞는다 | 정한 명령 꼴(`github:tourbut/fullops`)과 다르다. 마켓플레이스 출처도 그 브랜치를 가리켜야 하는데, `marketplace add`가 ref를 받는지와 `deps.toml`에 ref를 적는 꼴은 확인하지 않았다(코드의 출처 정규화는 `#`을 다루지 않는다 [확인: 조사 결과 install 갈래]). GitHub 기본 브랜치를 배포 브랜치로 바꾸면 ref 없이 되지만, 그러면 개발 브랜치가 기본이 아니게 된다 |

**추천: (가).** 지금 코드와 가장 적게 부딪치고, npx와 마켓플레이스가 한 트리를 읽는다. 커밋 규칙을 정해 둔다. `dist/`는 릴리스 커밋에서만 바꾸고, 개발 커밋은 `dist/`를 건드리지 않는다. 이력이 커지는 것이 문제가 되면 (다)(기본 브랜치를 배포 브랜치로 바꾸기)로 옮긴다. 다만 사용자가 등록한 마켓플레이스 출처는 나중에 바꾸기 어렵다(design.md H3 '위험' 칸). 이것은 H3·H16을 바꾸는 결정이라 사람이 정한다(5절 D1·D2).

### 1.3 실행기(`npx/fullops.cjs`)가 하는 일

범위: **바이너리를 고르고 검증한 뒤 `fullops install`에 넘기는 것까지만** 한다. 호스트 CLI, 마켓플레이스, 스킬, 출처 충돌 검사는 모두 Rust `fullops install`이 이미 한다(`dev/crates/fullops/src/cmd/install.rs`, `dev/crates/fullops-core/src/deps.rs`). 실행기에서 다시 만들지 않는다.

1. 인자를 검사한다(1.4절 표). 모르는 하위 명령·호스트, 값 없는 `--hosts`·`--repo`는 거절하고 아무것도 실행하지 않는다.
2. `process.platform`·`process.arch`로 `dist/native/fullops/bin/{fullops.exe | fullops-darwin-arm64 | fullops-darwin-x64 | fullops-linux-x64}`를 고른다. 이름 규칙은 `install.rs:48-57`과 같다. 지원하지 않는 조합이면 멈추고 알린다.
3. `dist/native/fullops/SHA256SUMS`(`<해시>  <상대 경로>`)로 그 파일을 검증한다. 다르면 멈춘다. 이 검사는 전송 손상만 잡고 출처 위조는 막지 못한다(서명은 H3).
4. POSIX에서는 실행 비트를 준다(npm 꾸러미는 `bin` 밖 파일의 실행 비트를 보장하지 않을 수 있다 [추정]).
5. 바이너리를 **셸 없이** 인자 배열로 `spawn`한다. 플러그인 루트는 넘기지 않는다. 바이너리가 `<X>/bin/`에 있고 `<X>/VERSION`이 있으면 `<X>`가 플러그인 루트가 된다 [확인: `dev/crates/fullops/src/cli.rs:127-132`].
6. 바이너리의 종료 코드를 그대로 돌려준다(0 성공, 2 빠짐·거절, 4 외부 명령 실패 [확인: `cli.rs:25`, INSTALL.md]).
7. 끝에 Orca 패널 설치 안내를 한 줄 낸다(1.5절). `orca`는 부르지 않는다.

옛 실행기에 있던 안전장치(`old/tests/npm-installer.cjs`가 단언하는 것)와 새 실행기의 대응:

| 옛 안전장치 | 새 판에서 |
|---|---|
| 인자 검증(모르는 호스트, 값 없는 `--repo` 거절, 공백 경로 보존) | 실행기가 한다. `claude-code`는 `claude`로 바꿔 넘긴다(새 판은 `claude, codex, grok, agy, all`만 받는다 [확인: `install.rs:300-312`]) |
| python3 3.10 확인 | 필요 없다(Rust 바이너리) |
| 모든 호스트의 출처 충돌을 먼저 검사하고 충돌하면 등록하지 않음 | Rust가 호스트마다 한다(`deps.rs:219-243`). 단, 충돌하면 옛 판은 **전체**를 멈췄고 새 판은 **그 호스트만** 멈춘다 |
| `--trust`를 넘기지 않음 | Rust가 넘기지 않는다(INSTALL.md '하지 않는 것') |
| `--dry-run`은 바꾸는 것 0건 | 실행기는 `--dry-run`을 그대로 넘긴다. Rust의 `--dry-run`은 조회 명령도 부르지 않는다 [확인: 임시 HOME 실행, `install.rs:532-548`] |
| 한 CLI의 실패가 다른 CLI를 막지 않음 | Rust가 호스트별로 결과를 낸다. 실패가 있으면 종료 코드 4 |
| 하향(낮은 버전) 설치 거부, 설치 뒤 목록 버전 확인 | **새 판에는 없다** [확인: `install.rs`에 하향 검사 없음, grep]. 제안 P2(4절) |
| Windows에서 셸 메타문자를 데이터로 보존 | 실행기는 `.exe`를 셸 없이 직접 띄우므로 PowerShell로 감쌀 필요가 없다. Rust는 인자 배열로 호스트 CLI를 부른다(design.md 9.5) |
| Codex 홈 4곳(`CODEX_HOME`, `~/.codex`, `--codex-home`, Orca의 `codex-runtime-home/home`)을 모두 처리 | **새 판은 현재 환경의 `CODEX_HOME` 하나만 본다** [확인: `fullops-ext/src/hosts.rs:186-187`, design.md:3022]. 1.7절에서 손으로 하는 법을 적고, 5절 D6에서 정한다 |
| 실제 실행기를 가짜 CLI로 돌려 모든 호출이 임시 홈 안에 머무는지 시험 | 새 실행기에도 같은 시험을 하나 둔다(가짜 바이너리로 인자 전달과 종료 코드 확인) |

### 1.4 사용자가 치는 명령(옛 판과 같은 꼴)

| 사용자 명령 | 실행기가 부르는 것 |
|---|---|
| `npx --yes github:tourbut/fullops install [--hosts claude,codex,grok,agy\|all] [--dry-run]` | `fullops install [--hosts …] [--dry-run]` |
| `npx --yes github:tourbut/fullops update [--hosts …] [--dry-run]` | `fullops install [--hosts …] [--dry-run]`. 새 판의 갱신은 다시 설치하는 것이다(INSTALL.md '갱신') |
| `npx --yes github:tourbut/fullops check [--hosts …]` | `fullops install --check [--hosts …]` |
| `… install\|update --repo <경로>` | 설치 뒤 `<경로>`를 작업 폴더로 `fullops upgrade`를 부른다. `--verify` 없이 부르면 레포를 고치지 않는다 [확인: `upgrade.rs:1-2`]. 그 레포가 아직 Python판(`fullops.json`)이면 `fullops migrate --from-python --dry-run`을 안내만 한다 |
| `… help` | 사용법 |

Windows에서 npx를 거치지 않을 때는 `<폴더>\bin\fullops.exe install`처럼 확장자까지 쓴다. PowerShell은 확장자 없는 경로를 실행하지 못한다(INSTALL.md).

주의 [확인: `install.rs:136-139`, `cli.rs:136-143`]: npx 캐시 폴더에서 install을 돌리면 `~/.fullops/install.json`의 `source`가 npx 캐시 경로가 된다. 캐시가 지워진 뒤 고정 위치 실행본으로 `fullops install`·`doctor`를 부르면 플러그인 루트를 찾지 못한다. 그러면 grok·agy 조립을 건너뛴다. hook과 Claude Code·Codex 동작에는 영향이 없다. 대책은 실행기가 아니라 Rust에 둔다(4절 P1).

### 1.5 Orca 패널 플러그인(fullops-panel) 배포·설치·갱신·제거

패널: `dev/panel/orca-plugin.json` = id `fullops-panel`, version `2.0.0`, `execution: "trusted-worker"`, 권한 `workspace:read`, `engines.orca >= 1.4.256` [확인]. hook·CLI와 따로 배포한다(`panel/README.md`, design.md 14절). `fullops install`은 Orca를 건드리지 않는다.

**걸림돌** [확인: Orca 1.4.256 설치 폴더의 CLI 코드. `out/shared/plugins/installation/plugin-git-install-input.js`의 pageRef, `plugin-install-source-info.js`의 `path: null`, `out/shared/plugins/catalog/plugin-marketplace.js:60-80`]: `orca plugin install <owner/repo>`나 tree URL로 하는 직접 설치에는 하위 경로가 없다. 저장소 루트의 `orca-plugin.json`을 쓴다. `…/tree/main/panel`은 ref `main/panel`로 읽힌다. 하위 폴더는 카탈로그 v2의 `source.path`로만 지정한다. 그래서 `orca plugin install tourbut/fullops`로는 패널이 설치되지 않는다 [추정: 설치 본체는 압축 번들이라 읽지 못했고, CLI·공유 코드에서 미루어 봤다].

**배포(레포 주인이 하는 일). 어느 안이든 사람 승인이 필요하다:**

| 안 | 배포 절차 | 사용자 설치 | 갱신 |
|---|---|---|---|
| **(A) 카탈로그. tourbut/fullops 루트에 `orca-catalog.json`(v2)** | `orca plugin validate panel` → `orca plugin release-entry panel`로 releases 항목과 `source.path` 출력 → 그 항목(`source {url, ref: <커밋>, path: "panel"}`)을 `orca-catalog.json`에 넣어 main에 커밋 | `orca plugin catalog add tourbut/fullops`(카탈로그 승인 `cat1-…` 지문) → `orca plugin install tourbut.fullops-panel`(코드 실행 승인) | 카탈로그가 새 releases를 내면 자동이다(`pluginUpdatePolicy` 기본 auto [확인: `plugin-update-policy.js`]) |
| (A') 레포 주인이 이미 운영하는 카탈로그가 있으면 거기에 항목 추가 | 같음(source의 url은 tourbut/fullops, path는 `panel`) | 이미 그 카탈로그를 등록한 사람은 `orca plugin install tourbut.fullops-panel`만 하면 된다 | 같음 |
| (B) 패널만 루트에 둔 별도 저장소(예: `tourbut/fullops-panel`) | `panel/`을 그 저장소로 옮기거나 동기화한다 → 태그 `v2.0.0` → 저장소에 `orca-plugin` 토픽 | `orca plugin install tourbut/fullops-panel`(승인) | `orca plugin update tourbut.fullops-panel`(최신 `vX.Y.Z` 태그로) |
| (C) 로컬 폴더(개발용) | - | `orca plugin install <경로>\panel`. 키가 `local.fullops-panel`이 되어 나중의 `tourbut.fullops-panel`과 별개로 남는다 | 폴더를 다시 복사한다 |

이 PC에는 `tourbut.claude-pin`(카탈로그 설치)과 `tourbut.grokbot-notify`(git 설치)가 이미 있다 [확인: `%APPDATA%/orca/plugins/plugins.lock.json`의 키. 값은 열지 않았다]. 레포 주인은 두 방식을 다 써 봤다. (A')에 쓸 카탈로그가 이미 있는지는 확인하지 않았다.

**추천: (A) 또는 (A').** 패널 원본을 `panel/` 한 곳에 두고, H14 기본안('이 저장소의 `panel/` 하위 폴더')과도 맞는다. 설치 키는 어느 안이든 `tourbut.fullops-panel`이다. 주의할 점: 태그 `v2.0.0`은 fullops 본체 릴리스와 겹친다. 그래서 (A)는 태그가 아니라 카탈로그의 커밋 ref로 패널 버전을 가리킨다. Orca 공개 목록(onorca.dev, `orca-plugin` 토픽)이 하위 폴더 플러그인을 올려 주는지는 확인하지 못했다.

**배포 전에 할 일(실환경 미확인)**: 패널을 Orca 안에서 실제로 돌려 본 적이 없다(`panel/README.md` '확인하지 못한 것': worker의 `node:fs` 읽기(M4), `workspace.readContext`와 `workspace.list`의 이름 일치). 먼저 사람이 다음을 확인한다:
```
orca plugin validate panel --json
orca plugin check panel --json
orca plugin dev load panel --json          # approval_required와 지문이 나온다. 사람이 승인한다
orca plugin dev request panel status.read --json
orca plugin dev unload panel
```

**설치 승인 절차** [확인: Orca 안내서 `bundled-skill-guides.js` 925-945행 부근, `plugin-approval-rerun.js`]: 코드를 실행하는 플러그인의 `orca plugin install`은 **아무것도 쓰지 않고** `approval_required`(종료 코드 3)로 끝난다. 오류에 허가 내용, publisher, 저장소, 그리고 `--approve <지문>`을 붙인 같은 명령이 나온다. 사람이 내용을 보고 그 명령을 다시 실행한다. 지문은 플러그인·버전·파일·권한에 묶인다. 이것이 달라지면 `approval_stale`과 새 지문이 나온다. Orca가 떠 있고 플러그인 시스템이 켜져 있어야 한다 [추정: CLI가 Orca 런타임에 요청한다. 꺼져 있으면 `plugin_system_disabled`].

**갱신**: `orca plugin update tourbut.fullops-panel`. 권한·실행 단계·패널이 늘거나 저장소가 바뀌면 다시 승인해야 하고, 승인하기 전까지는 있던 버전이 돈다. **제거**: `orca plugin uninstall tourbut.fullops-panel`. 승인이 필요 없고 파일·승인·테마 선택이 함께 지워진다.

**npx 실행기와 사람의 나눔**:

| 단계 | 누가 |
|---|---|
| 패널 출처 결정(H14), 패널 실환경 점검, 카탈로그·태그 공개 | 레포 주인(승인 필요) |
| `orca plugin catalog add …`, `orca plugin install …` 명령 안내 | npx 실행기(출력만 한다. `orca`를 부르지 않는다) |
| 승인(`--approve <지문>`으로 다시 실행) | 사용자. 에이전트가 대신 승인하지 않는다(Orca 안내서가 '사람이 정확히 요청하지 않았다면 묻는다'고 한다) |
| 갱신·제거 | 사용자가 Orca 명령으로 한다 |

패널 실환경 점검이 끝나고 출처가 정해지면, 실행기에 `orca plugin install`을 `--approve` **없이** 부르는 계획 단계를 더할 수 있다(4절 P7). 승인하기 전에는 아무것도 쓰지 않으므로 안전하다.

### 1.6 지금 당장 할 수 있는 길(개발자용, 로컬 배포본)

지금 실행 파일을 처음 얻는 방법은 **로컬 빌드 하나뿐**이다. GitHub에는 아직 배포본이 없다.

```powershell
# dev 체크아웃에서 (Windows 타깃 하나만). 산출물은 저장소 밖 임시 폴더에 만든다
$out = Join-Path $env:TEMP "fullops-dist"
cargo xtask dist --root $out --target x86_64-pc-windows-msvc
# 산출물: $out\dist\native\fullops\ (bin\fullops.exe, bin\fullops, SHA256SUMS, VERSION, …)
$fx = "$out\dist\native\fullops\bin\fullops.exe"

# 1) 계획만 본다. 아무것도 부르지 않는다
& $fx install --dry-run --hosts all

# 2) 실행본만 둔다(네트워크 없음)
& $fx install --only runtime

# 3) grok·agy는 이 폴더의 재료로 조립해 설치한다 (실환경 미확인)
& $fx install --hosts grok,agy
```

- `--root`를 주는 이유 [확인: `dev/docs/build.md:25-35`, `xtask/src/dist.rs:400,410`]: `--root`의 기본은 저장소 루트다. 기본값으로 돌리면 `dist/` 말고도 저장소 루트에 `.claude-plugin/marketplace.json`과 `.agents/plugins/marketplace.json`이 생긴다. `.gitignore`는 `/dist/`만 무시하므로 이 둘은 추적되지 않은 파일로 남는다. 이것을 커밋하면 `dist/` 없이 카탈로그만 공개된다. `--root` 없이 돌렸다면 이 둘을 커밋하지 말고 지운다.
- `--dry-run --hosts all`은 임시 HOME에서 종료 코드 0으로 계획을 냈다 [확인: realcheck의 `fullops.exe`, 조사 결과 install 갈래].
- 2)를 하면 `install.json`의 `source`가 이 임시 폴더가 된다. 임시 폴더를 지우면 1.4절 '주의'와 같은 일이 생긴다.
- **Claude Code·Codex는 이 길로 안 된다.** 이 둘은 로컬 폴더가 아니라 마켓플레이스 `tourbut/fullops`에서 받는다(`deps.rs:309-314`). main에 카탈로그와 dist가 없으니 `marketplace add`가 실패할 것이다 [추정]. 개발 중에 Claude Code는 실환경 점검처럼 `claude --plugin-dir <dist\native\fullops>`로 띄워 확인한다 [확인: 실환경 점검 기록 `realcheck/logs`]. Codex를 로컬 폴더로 설치하는 꼴은 계약 문서에 없다.
- macOS·Linux 바이너리는 한 PC에서 만들 수 없다. 네 타깃을 다 실으려면 CI가 OS별로 빌드한 뒤 `cargo xtask dist --prebuilt <폴더>`를 실행한다(`dev/docs/build.md:53-57`).

### 1.7 호스트별 순서, 사람이 할 승인, 확인

`fullops install`의 순서 [확인: 임시 HOME `--dry-run` 출력]: runtime(`~/.fullops/bin/fullops.exe`, `deps.toml`, `install.json`) → claude → codex → grok → agy → skills 9묶음(`npx --yes skills@latest add …`) → 스킬 백업과 `deps-state.json`.

| 호스트 | `fullops install`이 부르는 명령(design.md 9.5 표) | 설치 뒤 사람이 할 일(INSTALL.md) | 상태 |
|---|---|---|---|
| Claude Code | `claude plugin marketplace add tourbut/fullops` → `claude plugin install fullops@fullops` → `claude plugin update fullops@fullops --scope user`. 의존 ponytail·mattpocock-skills·pstack도 같은 꼴 | `/reload-plugins` 또는 새 세션 | 마켓플레이스 설치는 실환경 미확인(`--plugin-dir`로 hook 동작만 확인) |
| Codex | `codex plugin marketplace add tourbut/fullops` → `codex plugin add fullops@fullops`. 의존 ponytail | 세션에서 `/hooks`로 fullops hook을 신뢰한다. 등록 문자열이 바뀐 갱신 뒤에도 다시 신뢰한다 | 실환경 미확인(Codex hook 전체) |
| grok | `~/.fullops/package/2.0.0/grok` 조립 → `grok plugin install <그 경로>`(`--trust` 없음) → `~/.grok/hooks/fullops.json` 쓰기 | 원본 레포 폴더 신뢰(`/hooks-trust`), Context7을 쓰려면 플러그인 신뢰. 세션 시작 뒤 `fullops status` | 전역 hook 파일 방식은 실환경 미확인(점검은 레포 `.grok/hooks`로 했다) |
| agy | `~/.fullops/package/2.0.0/agy` 조립 → `agy plugin install <그 경로>` | agy 다시 시작. hook은 CLI에서만 돈다. **agy 하위 명령에 `--help`를 붙이지 않는다**(`agy plugin uninstall --help`가 실제로 제거를 실행했다 [확인: `dev/docs/contracts/agy.md:30`]) | `fullops install`을 거친 설치는 실환경 미확인(점검은 레포 `.agents/hooks.json`으로 했다) |
| Orca 패널 | (fullops install이 하지 않는다) | 1.5절 | 실환경 미확인 |

이 PC에 특유한 것 [확인: 조사 결과 install·pc 갈래]:
- Orca 터미널에는 `CLAUDE_CONFIG_DIR`가 Orca 프로필(`%APPDATA%/orca/claude-profiles/<id>/home`)로 잡혀 있다. 프로필은 넷이고, 플러그인 폴더는 `~/.claude/plugins`로 가는 링크라 함께 쓴다. `settings.json`의 `enabledPlugins`는 프로필마다 따로다. 그래서 어느 터미널에서 설치했느냐에 따라 fullops가 켜지는 프로필이 다를 수 있다 [추정]. 설치한 뒤 프로필마다 `claude plugin list --json`으로 확인한다.
- Orca의 Codex 홈(`%APPDATA%/orca/codex-runtime-home/home`)에 fullops를 넣으려면 `CODEX_HOME`을 바꿔 다시 실행한다(design.md:3022). 실환경 미확인. `$env:CODEX_HOME`은 그 PowerShell 창이 닫힐 때까지 남는다. 그대로 두면 뒤의 확인 명령이 모두 Orca 홈을 보게 되므로, 새 창에서 하거나 끝에 반드시 지운다:
  ```powershell
  $env:CODEX_HOME = "$env:APPDATA\orca\codex-runtime-home\home"
  & "$env:USERPROFILE\.fullops\bin\fullops.exe" install --hosts codex
  codex plugin list --json          # Orca Codex 홈 확인
  Remove-Item Env:CODEX_HOME
  codex plugin list --json          # ~/.codex 확인
  ```
- 설치 기록 `~/.fullops/deps-state.json`은 호스트 이름(`codex`)으로만 기록한다 [확인: `install.rs:683-695`]. 그래서 두 번째 설치가 첫 홈의 기록을 덮어쓰고, 기록에는 마지막으로 설치한 홈 하나만 남는다. 두 홈이 모두 갖춰졌는지는 기록이 아니라 홈마다 `codex plugin list --json`으로 본다.

확인 명령(`CODEX_HOME`을 지운 창에서):
```powershell
& "$env:USERPROFILE\.fullops\bin\fullops.exe" install --check   # 빠진 것이 있으면 종료 코드 2
& "$env:USERPROFILE\.fullops\bin\fullops.exe" doctor            # 실행본·호스트·의존·hook 등록·신뢰
claude plugin list --json; codex plugin list --json; grok plugin list --json; agy plugin list --json
```

### 1.8 릴리스 절차(제안. 각 단계에 사람 승인이 필요)

1. 버전을 올리고 `crates/fullops/assets/releases/<버전>.md`를 쓴다.
2. CI가 네 타깃을 빌드한다 → `cargo xtask dist --prebuilt <폴더>` → `claude plugin validate dist/native/fullops` 등을 검증한다(design.md:2745).
3. **(사람 승인)** `dist/native/fullops`와 카탈로그 둘을 main에 커밋한다(D1·D2가 정해진 뒤).
4. **(사람 승인)** 태그 `v<버전>`을 만들고, 필요하면 GitHub Release를 만든다.
5. **(사람 승인)** 패널: `orca plugin release-entry panel`의 출력으로 `orca-catalog.json`을 갱신해 커밋한다(1.5절 (A)).
6. 깨끗한 PC나 임시 HOME에서 `npx --yes github:tourbut/fullops install --dry-run`을 돌리고, 실제 설치 → `install --check` → `doctor` 순서로 확인한다.

---

## 2. 옛 판이 남긴 것

### 2.1 PC 쪽

근거는 `old/plugins/fullops-squad/scripts/` 코드와 이 PC의 목록 출력이다. 크기는 `du` 값이다.

| 무엇 | 어디 | 누가 만들었나 | 새 판도 쓰나 | 언제 지워도 되나 |
|---|---|---|---|---|
| Claude 플러그인 `fullops-squad@fullops-squad` (user 1.24.0, project 1.9.0 `projectPath C:\Users\shin`) | `~/.claude/plugins/cache/fullops-squad/fullops-squad/` 16개 버전(33M), `installed_plugins.json`, `~/.claude/settings.json`과 Orca 프로필 4개의 `enabledPlugins` | 옛 설치기(`fullops-squad.cjs:150`), project 범위는 누가 만들었는지 모른다 | 아니다. `fullops@fullops`가 대신한다 | 모든 레포를 옮기고 2주 뒤(design.md 13.4) |
| Claude 모드 `fullops-meter@fullops-squad` (user 0.12.1, project 0.10.1) | `…/cache/fullops-squad/fullops-meter/` | 사람(`/plugin install`) | 아니다. Orca 패널이 대신한다 | 사람이 원할 때. FullOps 밖에서도 쓰일 수 있어 따로 정한다 |
| Claude 마켓플레이스 `fullops-squad`(`tourbut/Fullops-Squad-plugin`) | `known_marketplaces.json`, `~/.claude/plugins/marketplaces/fullops-squad`(13M), 프로필의 `extraKnownMarketplaces` | 옛 설치기 | 아니다 | 위 두 플러그인을 모든 범위·프로필에서 지운 뒤. `fullops-meter`를 남기면 마켓플레이스도 남긴다 |
| Claude 플러그인 데이터 폴더(비어 있음) | `~/.claude/plugins/data/fullops-squad-fullops-squad` | Claude Code | 아니다 | 언제든. 같은 곳의 `fullops-inline`은 새 판 점검이 만든 것이다 |
| Codex 플러그인 `fullops-squad@fullops-squad` 1.24.0과 hook 신뢰 9건 | `~/.codex/config.toml`과 Orca Codex 홈 `config.toml`의 `[marketplaces.fullops-squad]`, `[plugins."fullops-squad@fullops-squad"]`, `[hooks.state]`. `~/.codex/plugins/cache/fullops-squad`(1.9M), `~/.codex/.tmp/marketplaces/fullops-squad`, `~/.codex/plugins/repos/Fullops-Squad-plugin`(2.6M) | 옛 설치기(홈 두 곳), 신뢰는 사람 | 아니다(ponytail 신뢰 3건은 새 판도 쓴다) | Codex로 쓰는 레포를 모두 옮긴 뒤. 홈 두 곳 모두에서 |
| grok 플러그인 `fullops-squad` 1.24.0 | `~/.grok/installed-plugins/c--users-shin--grok-marketplace-cache-…-fullops-squad-…`, `registry.json`, `~/.grok/marketplace-cache/f52eabb98417f682` | 옛 설치기 | 아니다 | grok으로 쓰는 레포를 모두 옮긴 뒤 |
| 설치 기록 `fullops-deps.json` 8개 | `~/.claude`, Orca Claude 프로필 4개, `~/.codex`, Orca Codex 홈, `~/.grok` | 옛 `deps.py`(`deps.py:103-107`) | 아니다(새 판은 `~/.fullops/deps-state.json`) | 그 홈의 옛 플러그인을 지운 뒤 |
| 스킬 백업 `fullops-skill-backups` | `~/.codex`(763M), Orca Codex 홈(408M), `~/.grok`(45M) | 옛 `deps.py:188-212` | 아니다(새 판은 `~/.fullops/skill-backups`) | 되살릴 스킬이 없다고 사람이 확인하면 언제든. 합계 약 1.2G로 가장 크다 |
| hook 런타임 | `~/.fullops-squad/runtime/fullops-squad`(143M, 스냅샷 66개) | 옛 `deps.py`·`hook_runtime.py` | 아니다 | **모든 홈의 옛 플러그인을 지우고 옛 세션을 모두 닫은 뒤.** 옛 hook이 이 파일을 python3로 직접 부른다(`old/plugins/fullops-squad/hooks/hooks.json`의 hook 등록 10곳이 `Path.home()/'.fullops-squad/runtime/fullops-squad/scripts/hook_runtime.py'`를 부른다). 먼저 지우거나 옮기면 옛 플러그인이 켜진 모든 세션의 옛 hook이 실패한다 |
| Jev 캐시 | `~/.cache/fullops-squad/jev`(18M) | 옛 `jev_observe.py:266` | 아니다(새 판은 `~/.fullops/cache/jev`) | 캐시라 언제든 지울 수 있다. 전환 뒤에 지우면 비용을 아낀다 |
| npm 전역 `@upstash/context7-mcp` | `%APPDATA%/npm/node_modules/@upstash`, `context7-mcp{,.cmd,.ps1}` | 옛 `deps.py:53` [추정: 시각 일치] | 아니다(새 판 Context7은 원격 HTTP) | 모든 홈의 옛 플러그인을 지운 뒤. `~/.claude/skills/context7-mcp`(사람 것)가 이것에 기대는지 먼저 본다 |
| npm 전역 `@alibaba-group/open-code-review`(`ocr`), `~/.opencodereview` | `%APPDATA%/npm/…` | 옛 `deps.py` | 아니다(H6에서 버림) | 모든 레포를 옮긴 뒤 |
| 스킬 `open-code-review-delegate` | `~/.agents/skills/…`, `~/.claude/skills`·`~/.grok/skills`의 링크, `.skill-lock.json` | 옛 `deps.py` | 아니다 | 모든 레포를 옮긴 뒤. skills CLI로 지워야 lock과 링크가 함께 정리된다 [추정] |
| **공용**: 공통 스킬 7종(caveman, typesafe-ai, diagram-design, frontend-design, ui-ux-pro-max, impeccable, web-design-guidelines), mattpocock 스킬, grok의 ponytail 스킬, `~/.agents/.skill-lock.json` | `~/.agents/skills`, `~/.claude/skills`, `~/.grok/skills` | 옛 `deps.py`(사람 것과 섞여 있다) | **쓴다**(`plugin/deps.toml [[skills]]`) | **지우지 않는다** |
| **공용**: 플러그인 ponytail·mattpocock-skills·pstack과 마켓플레이스 ponytail·claude-plugins-official·cursor-plugins | `~/.claude/plugins`, `~/.codex` | 옛 `deps.py`(사람이 먼저 깔았을 수 있다) | **쓴다**. 출처가 `deps.toml`과 같아 충돌하지 않는다 [추정] | **지우지 않는다** |
| **공용**: grok 폴더 신뢰 5개 | `~/.grok/trusted_folders.toml` | 옛 `grok_trust.py` 또는 grok 화면(가릴 수 없다) | **쓴다** | **지우지 않는다** |
| agy | (없음) | - | - | 옛 판은 agy에 아무것도 남기지 않았다 [확인: `agy plugin list --json`, `~/.agy` 없음] |

### 2.2 레포 쪽(레포마다. jevpet은 파일 이름만 확인)

| 무엇 | 어디 | 누가 만들었나 | migrate가 하는 일 / 새 판도 쓰나 | 언제 지워도 되나 |
|---|---|---|---|---|
| 표식·설정 `fullops.json` | `.fullops-squad/` | `setup.py` | `fullops.toml`로 옮기고 `git mv`로 `fullops.json.py-1.24.0`이 된다 | 되돌리기가 더는 필요 없을 때(2주 뒤). 그 전에는 `--rollback`의 원본이다 |
| `orca-agents.md` 라우팅 표식 줄 | `.fullops-squad/orca-agents.md` | `setup.py` | `kind`·`routing`·`about`으로 옮긴다. 문서는 남는다 | 지우지 않는다 |
| 관리 블록 `<!-- fullops-squad:start/end -->` | `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` | `setup.py` | 새 블록 `<!-- fullops:start -->`로 바꾼다 | 자동 |
| 템플릿 12종(1.24.0 기본 글) | `.fullops-squad/…` | `setup.py` | 기본 글이면 새 템플릿으로 바꾸고 front matter를 남긴다. 사람이 고친 글이면 두고 `python3` 줄을 보고서에 낸다 | 지우지 않는다. 고친 글은 손으로 고친다 |
| `PLANS.md`, `handovers/logs/`, `lint/lint.json`, `review/rule.json`, 규칙·산출물 문서 | `.fullops-squad/` | 사람·setup | 그대로 쓴다(design.md 13.2) | 지우지 않는다 |
| 빈 인박스 `handovers/to_<역할>.md`(0바이트) | `.fullops-squad/handovers/` | `setup.py` | 지운다(`--rollback`이 다시 만든다) | 자동 |
| 옛 상태 폴더 5개(`fullops-flow`, `-integration`, `-decisions`, `-autonomy`, `-issues`) | `<git-common-dir>/` | 옛 판 | `<common>/fullops/py-backup/`에 **복사**한다. 열린 결정과 hold는 `state.json`으로, 선행 키는 `archive.jsonl`로 가져온다. 원본은 지우지 않는다 | 2주 뒤 사람이 지운다(4절 P5) |
| 사람 턴 기록 `fullops-flow/human-turns.jsonl` | 위 폴더 안 | 옛 판 | 가져오지 않는다. 지표는 0부터 다시 센다 | 위와 같다 |
| `fullops-snapshots/`, 작업 트리별 `fullops-gate/`, `fullops-search/` | `<git-common-dir>`, `<git-dir>` | 옛 판 | 건드리지 않는다 | 모든 역할 워크트리가 새 판으로 넘어간 뒤 |
| `board/`, 옛 `handovers/<KEY>-*.md`, `contexts/`, `docs/agents/frontend.json`, `test/unity-play.json`, 옛 리뷰 폴더, `docs/evaluations/jev/`의 옛 탐색 기록 | 레포 | 옛 판 | 건드리지 않는다. 읽기 전용 이력이다 | 사람이 판단한다(이력으로 남겨도 해가 없다) |
| 되돌리기 기록 `migrate.json` | `<common>/fullops/py-backup/` | `fullops migrate` | `--rollback`이 쓴다. git 공통 폴더에만 있어 전환을 실행한 PC·클론에만 있다 | `--rollback`이 필요 없어진 뒤 |

---

## 3. 단계별 정리 절차

### 0단계: 공존(지금)

- 옛 판은 여러 레포(0절 4번 목록)에서 계속 쓴다. 새 판은 이 PC에 아직 없다(`~/.fullops` 없음 [확인]).
- 두 판은 간섭하지 않는다. 근거는 셋이다. 플러그인과 마켓플레이스 이름이 다르다(`fullops` / `fullops-squad`, design.md 13.1). 각 hook은 자기 표식만 보고, 표식이 없는 레포에서는 빈 판정을 낸다(`fullops.toml` / `fullops.json`, design.md 13.1). 새 판의 출처 충돌 검사는 `deps.toml`의 이름(fullops, ponytail, claude-plugins-official, cursor-plugins)만 본다(`deps.rs:219-243`).
- 남는 겹침: 공용 스킬을 새 판이 다시 설치하면 덮어쓴다. 새 판은 덮어쓰기 전에 `~/.fullops/skill-backups/`에 백업한다(INSTALL.md). 그리고 두 판의 hook이 같은 세션에서 함께 돈다(표식이 없는 쪽은 빈 판정을 낸다). caveman 안내가 두 번 붙을 수 있다 [추정].
- 이 단계에서 할 일: 1.6~1.7절로 새 판을 설치하고 `fullops doctor`를 본다. 옛 판은 건드리지 않는다.

**되돌리기(새 판을 걷어 낼 때).** 실환경 미확인. 새 판에는 설치 전체를 되돌리는 명령이 없다. 아는 것은 다음과 같다 [확인]:
- `fullops install`은 공용 스킬을 `npx --yes skills@latest add`로 다시 설치하고, 그 전에 있던 스킬 폴더를 `~/.fullops/skill-backups/<호스트>/<이름>/<해시 앞 12자>/`에 한 번 복사해 둘 뿐이다(`install.rs:253-283`, `651-658`). 되살리는 명령은 없다.
- `fullops install --rollback`은 고정 위치 실행본만 바로 앞 것으로 바꾼다(`install.rs:918-949`).
- `fullops install --remove-hooks`는 fullops가 직접 쓴 grok hook 등록 파일 `~/.grok/hooks/fullops.json` 하나만 지운다(`install.rs:951-966`). 호스트의 `plugin uninstall`은 부르지 않는다.
- 새 hook은 고정 위치 실행본(`~/.fullops/bin/fullops.exe`)이 없으면 플러그인 폴더에서 스스로 다시 설치한다(`hook/mod.rs:295-307`). 그래서 어느 프로필·홈에든 fullops 플러그인이 켜져 있으면 `~/.fullops`를 지워도 다음 hook이 다시 만든다.

순서:
1. **스킬 백업을 먼저 보존한다.** `~/.fullops/skill-backups/`를 홈 밖의 폴더(예: `D:/backup/fullops-skill-backups-<날짜>`)로 옮긴다. `~/.fullops`를 지우면 이 백업도 함께 사라진다. 스킬을 되살리려면 `<보존 폴더>/<호스트>/<이름>/<해시>/`의 내용을 원래 자리(예: `~/.agents/skills/<이름>`, `~/.claude/skills/<이름>`, `~/.grok/skills/<이름>`)에 손으로 복사한다. 백업 경로에는 호스트 이름만 남고 어느 폴더(`~/.agents/skills`인지 호스트 설정 폴더인지)에서 왔는지는 남지 않는다(`hosts.rs:170-190`). 같은 이름에 해시가 여럿이면 옛 것이 어느 것인지 폴더 시각을 보고 사람이 고른다.
2. **새 `fullops` 플러그인을 모든 호스트·범위·프로필에서 지운다.** 2단계 2-2와 같은 주의가 붙는다. **Claude Code·Codex의 제거 명령 꼴은 `docs/contracts/`에 없다.** 실행하기 전에 각 호스트 문서로 확인한다(실환경 미확인).
   - Claude Code: `fullops@fullops`를 `~/.claude`와 Orca 프로필 4개 모두에서 지운다. 플러그인 폴더는 함께 쓰지만 `enabledPlugins`는 프로필마다 따로다. 프로필마다 `CLAUDE_CONFIG_DIR`를 그 프로필로 두고 CLI로 지운다. 그 뒤 마켓플레이스 `fullops`를 지운다.
   - Codex: `fullops@fullops`와 마켓플레이스 `fullops`를 `~/.codex`와 Orca Codex 홈 **두 곳**에서 지운다(`CODEX_HOME`을 바꿔 두 번. 1.7절처럼 끝에 `Remove-Item Env:CODEX_HOME`).
   - grok: `grok plugin uninstall fullops`(꼴은 `dev/docs/contracts/grok.md:35`).
   - agy: `agy plugin uninstall <폴더 이름>`(꼴은 `dev/docs/contracts/hosts-delta.md:194`). `--help`를 붙이지 않는다.
3. `fullops install --remove-hooks`로 grok hook 등록 파일을 지운다.
4. 확인: 프로필마다(`~/.claude`와 Orca 프로필 4개) `CLAUDE_CONFIG_DIR`를 바꿔 `claude plugin list --json`을 보고 `fullops@fullops`가 하나도 없는지 본다. Codex 두 홈, `grok plugin list --json`, `agy plugin list --json`도 본다. 새 판 세션을 모두 닫는다.
5. 4에서 어디에도 없다고 확인한 뒤에만 `~/.fullops`를 지운다. 플러그인이 없으면 `~/.fullops`는 아무것도 하지 않으므로 지우지 않고 두어도 된다.

되돌릴 수 없는 것: `fullops install`이 함께 설치·갱신한 의존 플러그인(ponytail, mattpocock-skills, pstack. Claude Code는 `plugin update --scope user`로 올린다)과 그 마켓플레이스는 설치 전 버전으로 되돌리지 못한다. 공용 스킬도 1에서 보존한 백업을 손으로 복사하지 않으면 다시 설치된 최신판이 그대로 남는다. 4절 P9(`install --restore-skills`)가 이 손작업을 줄이는 안이다.

### 1단계: 레포 전환(레포마다, 사람 승인 뒤)

design.md 13.4의 순서를 따른다.

1. 과제 경계에서 인박스를 비우고, 옛 판 `integration.py settle`을 돌린다(통합 대기 0).
2. 미리 본다. 쓰는 것이 없다:
   ```
   fullops migrate --from-python --dry-run
   ```
   보고서에서 읽지 못한 표식 줄, 컴파일 실패 lint 규칙, `PLANS.md` 문법 오류, 사람이 고친 템플릿의 `python3` 줄을 정리한다.
3. 실행: `fullops migrate --from-python`. 커밋은 하지 않으니 사람이 `git status`와 보고서를 보고 커밋·push한다. 역할 워크트리를 동기화하고 새 세션을 연다. 표식은 브랜치마다 있으므로 각 워크트리(`git worktree list`)의 브랜치가 전환 커밋을 받아야 그 워크트리에서도 옛 표식이 사라진다.
4. 확인:
   - `.fullops-squad/fullops.toml`이 있고 `fullops.json.py-1.24.0`이 있다.
   - `fullops upgrade`(레포를 고치지 않는다)의 필수 적용 검사가 모두 통과한다.
   - 과제 하나를 끝까지 돌린 뒤 `fullops why`로 판정을 본다.
   - Orca 패널이 있으면 그 레포가 보인다.
5. 문제가 있으면 `fullops migrate --rollback`으로 되돌리고 커밋한다. 옛 플러그인을 지우지 않았으므로 옛 판이 바로 돈다. `fullops.toml`은 `fullops.toml.rolled-back`으로 남는다. 전환 뒤 생긴 결정·hold는 화면에 나오고, 사람이 옛 판 `decision.py add`·`integration.py hold`로 옮긴다(design.md 13.3). 주의 [확인: `migrate.rs`]:
   - 역할 인박스(`handovers/to_<역할>.md`)가 비어 있지 않거나 새 상태에 통합 대기가 있으면 `--rollback`은 거절한다(`migrate.rs:1003-1025`). 먼저 인박스를 비우고 대기는 `fullops merge`나 `fullops hold`로 정리한다.
   - 되돌리기 기록 `migrate.json`은 git 공통 폴더(`<common>/fullops/py-backup/`)에만 있다(`migrate.rs:547-549`). 전환을 실행하지 않은 다른 클론·PC에서 `--rollback`을 돌리면 표식만 되돌린다. 그때 엔트리 문서(`AGENTS.md` 등)와 템플릿은 전환 커밋을 `git revert`로 되돌린다(`migrate.rs:1034-1038`).

레포에 남는 옛 흔적과 지울 시점: `fullops.json.py-1.24.0`, `py-backup/`, `migrate.json`, 옛 상태 폴더 5개는 전환 뒤 2주가 지나고 되돌리기가 필요 없다고 정한 다음 지운다. 옛 이력 파일(2.2절의 '건드리지 않는다' 줄)은 남겨도 된다.

주의 [확인: 조사 결과 repo 갈래]:
- design.md 13.2 표의 '되돌리기' 칸은 `--rollback`이 백업과 `state.json`·`archive.jsonl`을 지운다고 적는다. 하지만 코드와 13.3절은 남긴다. 코드가 맞다고 보고 문서를 고친다(D9).
- `migrate`의 '작업 트리가 깨끗하다' 검사는 주 체크아웃만 본다. 옛 판은 모든 워크트리를 봤다. 전환 전에 역할 워크트리에 커밋하지 않은 변경이 없는지 사람이 확인한다.

### 2단계: PC 정리(모든 레포를 옮기고 2주 뒤, 따로 승인받아)

**2-0. 전제 확인**
- 이 PC의 모든 레포와 워크트리에 `.fullops-squad/fullops.json`이 없다(남은 것은 `fullops.json.py-1.24.0`뿐이다). 다음 두 가지를 모두 한다.
  1. 레포마다 `git worktree list`로 모든 워크트리를 얻고, 각 워크트리에 `.fullops-squad/fullops.json`이 없는지 본다. 표식은 브랜치마다 있으므로 주 체크아웃만 보면 놓친다. 대상 레포: `D:/workspace`의 `erden-godot`, `erden-unity`, `erden-s06-click-gui-sandbox-20261006`, `KnowsLink`, `jevpet`, `Fullops-Squad-plugin`.
  2. `git worktree list`에 잡히지 않는 것까지 보려고 `C:/Users/shin/orca/workspaces` 아래를 따로 훑는다(`marketplace-cache`는 뺀다). 2026-11-15 현재 15곳에 옛 표식이 있다(0절 4번 목록). `KnowsLink/.fullops-review-…` 같은 임시 리뷰 워크트리는 전환 대신 지워도 되는지 사람이 정한다.
- `Fullops-Squad-plugin`(옛 판 원본)은 D8에서 정한 대로 한다. 옛 판 개발을 계속하기로 했다면 2단계 전체를 미룬다.
- 옛 판 세션(Claude Code, Codex, grok)을 모두 닫는다.

**2-1. 설정 파일 사본.** 이 단계에서는 **복사만** 하고 아무것도 옮기거나 지우지 않는다. `~/.fullops-squad`는 옛 hook이 부르는 runtime이라, 옛 플러그인이 남아 있는 동안 옮기면 옛 플러그인이 켜진 모든 세션의 hook이 실패한다(2.1절 hook 런타임 줄).
- 사본을 한 폴더에 모은다:
  - Claude: `~/.claude/settings.json`, `~/.claude/plugins/installed_plugins.json`, `known_marketplaces.json`, Orca 프로필 4개의 `settings.json`
  - Codex: `~/.codex/config.toml`, Orca Codex 홈의 `config.toml`
  - grok: `~/.grok/installed-plugins/registry.json`, `~/.grok/config.toml`
- 되돌리는 법: 사본을 제자리에 복사하고(2-4에서 옮긴 폴더가 있으면 그것도 제자리로), 옛 설치기를 다시 실행한다(`npx --yes github:tourbut/Fullops-Squad-plugin install`). 이 설치기가 플러그인, 마켓플레이스, runtime, deps를 다시 만든다.

**2-2. 플러그인 제거.** 호스트 명령으로 한다. 파일을 손으로 지우지 않는다.

| 호스트 | 지울 것 | 명령 |
|---|---|---|
| grok | `fullops-squad` 플러그인과 마켓플레이스 | `grok plugin uninstall fullops-squad`, 이어서 `grok plugin marketplace remove <url>`(꼴은 `dev/docs/contracts/grok.md:32,35`). url은 `grok plugin marketplace list --json`에서 본다 |
| Claude Code | `fullops-squad@fullops-squad`(user, project). `fullops-meter@fullops-squad`(user, project)는 사람이 지우기로 정했을 때만. 마켓플레이스 `fullops-squad`는 **`fullops-meter`도 지울 때만** 지운다 | **제거 명령의 꼴은 `docs/contracts/`에 없다.** 실행하기 전에 Claude Code 문서로 확인한다(후보 `claude plugin uninstall <id> --scope <범위>`, 실환경 미확인). Orca 프로필 4개의 `enabledPlugins`에도 남으므로, 프로필마다 `CLAUDE_CONFIG_DIR`를 그 프로필로 두고 CLI로 지운다. 마켓플레이스는 그 뒤에, `fullops-meter`까지 지웠을 때만 지운다. `fullops-meter`를 남기면 마켓플레이스 `fullops-squad`도 남긴다. 마켓플레이스를 지우면 그 마켓플레이스에서 설치한 플러그인이 함께 빠질 수 있다 [추정: 확인하지 않았다] |
| Codex | `fullops-squad@fullops-squad`, 마켓플레이스 `fullops-squad`. `~/.codex`와 Orca Codex 홈 **두 곳** | 제거 명령의 꼴은 계약 문서에 없다. Codex 문서로 확인한 뒤 `CODEX_HOME`을 바꿔 두 번 실행한다(실환경 미확인). 끝에 `Remove-Item Env:CODEX_HOME` |
| agy | 없음 | - |

**2-3. 확인.** 허용된 목록 명령만 쓴다. Claude Code는 `~/.claude`와 Orca 프로필 4개에서 `CLAUDE_CONFIG_DIR`를 바꿔 가며, Codex는 두 홈에서 `CODEX_HOME`을 바꿔 가며 각각 본다.
```
claude plugin list --json; claude plugin marketplace list --json
codex plugin list --json; grok plugin list --json; agy plugin list --json
```
- 어느 호스트·범위·프로필·홈에도 `fullops-squad@fullops-squad`(grok은 `fullops-squad`)가 없어야 한다. `fullops-meter`를 남기기로 했다면 Claude Code의 `fullops-meter@fullops-squad`와 마켓플레이스 `fullops-squad`는 남아 있는 것이 맞다.
- `fullops@fullops`, ponytail, mattpocock-skills, pstack은 남아 있어야 한다.
- 이어서 `fullops install --check`와 `fullops doctor`가 통과해야 한다. 공용 의존이 그대로인지 확인하는 것이다.

**2-4. 홈에 남은 것 정리.** 아래 순서로 한다. runtime은 반드시 2-3에서 옛 플러그인이 어디에도 없다고 확인한 뒤에 다룬다.
1. `fullops-deps.json` 8개, `~/.claude/plugins/data/fullops-squad-fullops-squad`, `~/.cache/fullops-squad`
2. 큰 폴더를 백업 폴더로 **옮긴다**: `~/.fullops-squad`(runtime)와 `fullops-skill-backups` 3곳. `~/.fullops-squad`는 2-3에서 `fullops-squad`가 어느 호스트·범위·프로필에도 없다고 확인했고 옛 세션이 하나도 없을 때만 옮긴다. 옮긴 `~/.fullops-squad`는 되돌릴 일이 없다고 정한 뒤 지운다.
3. npm 전역 `@upstash/context7-mcp`, `@alibaba-group/open-code-review`, `~/.opencodereview`. 다른 용도로 쓰는지 사람이 먼저 확인한다. 명령은 npm의 전역 제거다(이 조사에서 실행하지 않았다).
4. `open-code-review-delegate` 스킬. skills CLI의 제거 명령으로 지운다(꼴은 확인하지 않았다).
5. 옮겨 둔 `fullops-skill-backups`. 되살릴 스킬이 없다고 확인한 뒤 지운다.

**지우지 않는 것**: 공통 스킬 7종, mattpocock·ponytail 스킬, `~/.agents/.skill-lock.json`, ponytail·mattpocock-skills·pstack 플러그인과 그 마켓플레이스 셋, grok 폴더 신뢰, `~/.config/context7` 같은 사람 설정. `fullops-meter`를 남기기로 했다면 그것과 마켓플레이스 `fullops-squad`.

---

## 4. 새 판에 더하면 좋은 것(구상. 구현하지 않음)

| 안 | 왜 필요한가 | 대략의 크기 |
|---|---|---|
| **P0 npx 실행기**(`package.json` + `npx/fullops.cjs` + 시험 하나) | 레포 주인의 결정(2026-11-15). 1.3·1.4절 | .cjs 약 150줄, 시험 1개. 설계 10.3절 끝 문장('Node 설치기는 두지 않는다')과 14절 표에 한 줄 |
| **P1 `install`이 배포본을 `~/.fullops/package/<버전>/source`로 복사하고 `install.json`의 `source`를 거기로 적기** | npx 캐시가 지워지면 플러그인 루트를 잃는다(1.4절 주의). 실행기에 설치 로직을 두지 않으려면 Rust에서 고친다 | `install.rs` runtime 단계에 복사 하나, 시험 1개 |
| **P2 하향 거부** | 옛 판은 낮은 버전 설치를 거부했다. 새 판은 runtime 단계에서 그대로 바꾼다 | `install.json`의 version과 비교. `--rollback`과 겹치지 않게 한다. 작다 |
| **P3 `doctor`의 'Python판 잔재' 행(알림만)** | 지금 새 판 코드에는 `fullops-squad`를 가리키는 곳이 없어 잔재를 알리지 않는다 [확인: grep] | 이미 부르는 `plugin list` 결과에서 `fullops-squad`를 찾고, `~/.fullops-squad/runtime`, `fullops-deps.json` 경로가 있는지만 본다. 지우지 않는다. 작다 |
| **P4 `fullops install --retire-python`(가칭)** | 2단계를 손으로 하면 홈 8곳과 프로필 4개를 돌아야 해서 빠뜨리기 쉽다 | 중간. 안전장치는 아래와 같다 |
| P5 `fullops migrate --finalize`(가칭) | 2주 뒤 `py-backup/`, `migrate.json`, 옛 상태 폴더 5개를 정리하고 `fullops.json.py-1.24.0`의 `git rm`을 안내한다. 지금은 손으로 한다 | 작다. 커밋은 사람이 한다 |
| P6 여러 홈 처리 | 옛 설치기는 Codex 홈 4곳을 돌았고, 이 PC에는 Orca Codex 홈과 Claude 프로필 4개가 따로 있다. 새 판은 하나만 본다. `deps-state.json`도 호스트 이름으로만 기록해 마지막 홈만 남는다(1.7절) | `doctor`가 다른 홈을 알리기만 할지(작다), `install`이 알려진 Orca 홈을 돌지(중간) 정해야 한다(D6) |
| P7 실행기의 Orca 패널 단계 | 패널 출처(H14)와 패널 실환경 점검이 끝나면 `orca plugin install <출처>`를 **`--approve` 없이** 부르고, 승인 명령을 사람에게 보여 준다. 승인하기 전에는 Orca가 아무것도 쓰지 않는다 | 작다. 기본은 끔(`--with-panel`) |
| P8 릴리스 자동화 | 네 타깃 빌드와 `xtask dist --prebuilt`, 패널 `release-entry`를 CI가 하고, 결과를 PR로 올려 사람이 승인한다 | CI 워크플로 1개 |
| P9 `fullops install --restore-skills`(가칭) | 새 판은 공용 스킬을 덮어쓰기 전에 `~/.fullops/skill-backups/`에 백업만 하고 되살리는 명령이 없다(3절 0단계 되돌리기). 백업에 원래 폴더가 남지 않아 손으로 되살리기 어렵다 | 백업할 때 원래 경로를 함께 적고, `--restore-skills`가 그 경로로 되돌린다. 기본은 계획만 내고 `--apply`로 실행. 중간 |

P4 `--retire-python`의 안전장치:
- 기본은 계획만 낸다(dry-run). 실제 실행은 `--apply`를 줄 때만 한다.
- **1.24.0 코드가 쓰는 자리로 정한 목록**에 있는 것만 다룬다: `deps.py:103-107`의 기록, `:201` 백업, `hook_runtime.py:15` runtime, `jev_observe.py:266` 캐시, 그리고 이름이 `fullops-squad`인 플러그인과 마켓플레이스. 목록 밖은 건드리지 않는다. `fullops-meter`를 남기는 설정이면 마켓플레이스 `fullops-squad`도 남긴다.
- `plugin/deps.toml`에 있는 플러그인·마켓플레이스·스킬은 계획에서 뺀다(공용 의존은 남긴다).
- 이 PC에서 아는 레포와 그 모든 워크트리 가운데 `fullops.json`이 남은 곳이 하나라도 있으면 거절한다. 아는 레포의 목록을 어디서 얻을지(Orca 워크스페이스 목록, 또는 `--repos`로 받기)는 정해야 한다. 워크트리는 레포마다 `git worktree list`로 얻는다.
- 지우지 않고 `~/.fullops/py-retired/<시각>/`로 옮긴다. `--restore`로 되돌린다.
- runtime은 플러그인 제거가 확인된 뒤 마지막에 다룬다.
- 호스트 명령은 `docs/contracts/`에서 확인된 하위 명령만 부른다. 어떤 하위 명령에도 `--help`를 붙이지 않는다(agy의 전례). Claude Code·Codex의 제거 명령은 계약 문서에 확인해 넣기 전까지 안내만 한다.
- npm 전역 패키지는 지우지 않고 안내만 한다(다른 용도일 수 있다).

---

## 5. 열린 결정(사람이 정할 것)

| # | 결정 | 선택지와 이 문서의 추천 |
|---|---|---|
| D1 (H3) | 바이너리 유통·서명·마켓플레이스 출처 | 추천: main에 릴리스 때만 `dist/native/fullops` 커밋, 서명 없이 시작, 출처 `tourbut/fullops`. 대안: Releases 내려받기, 배포 브랜치 `#ref` |
| D2 (H16) | `dist/`와 카탈로그 둘을 어느 브랜치에 두나 | D1을 따른다. (가)를 고르면 `.gitignore`의 `/dist/`를 고친다. 예: `/dist/directory/`만 무시하고, `dist/native/`는 릴리스 커밋만 바꾼다는 규칙을 둔다 |
| D3 (새) | 설계 10.3절 'Node 설치기는 두지 않는다'와 14절 표, INSTALL.md·README의 설치 길을 npx 결정에 맞게 고치기 | 고친다. 실행기는 G4 검사 밖(저장소 루트)이라 그 검사와는 부딪치지 않는다(`dev/docs/build.md:49`) |
| D4 (H14) | Orca 패널의 배포 자리 | 추천: 카탈로그 (A) `tourbut/fullops`의 `orca-catalog.json`, `source.path: "panel"`, 또는 (A') 이미 운영하는 카탈로그. 대안: (B) 별도 저장소. 그 전에 패널 실환경 점검(M4)을 한다 |
| D5 (새) | 패널 버전과 태그 | 본체 태그 `v<버전>`과 겹치지 않게, 카탈로그의 커밋 ref로 패널 버전을 가리킨다. (B)를 고르면 별도 저장소의 태그를 쓴다 |
| D6 (새) | Orca Codex 홈·Claude 프로필 4개 처리 | 손으로 `CODEX_HOME`·`CLAUDE_CONFIG_DIR`를 바꿔 다시 실행할지, 새 판에 P6을 넣을지 |
| D7 (새) | npx 캐시를 `source`로 적는 문제 | P1로 고칠지 |
| D8 (새) | 옛 판 정리 시점, `fullops-meter` 처분, 옛 판 원본 저장소 | design.md 13.4: 모든 레포·워크트리 전환과 2주 뒤 따로 승인. `fullops-meter`는 FullOps 밖 사용 여부를 보고 정한다. 남기면 Claude Code 마켓플레이스 `fullops-squad`도 남는다. `Fullops-Squad-plugin`(옛 판 원본, `D:/workspace`와 워크트리 `Fullops-Squad-plugin/dev`)은 자기 자신을 옛 판으로 쓰고 있다. (1) 이 레포도 새 판으로 전환하거나, (2) 옛 판 개발이 끝날 때까지 2단계(PC 정리)를 미룬다. 둘 중 하나를 정한다 |
| D9 (새) | 설계와 코드의 어긋남 | (1) 출처 충돌의 종료 코드: 설계 10.3은 2라고 적고, 코드는 4로 끝난다(`deps.rs:241`, `install.rs:633-635,722`). (2) 13.2 표의 되돌리기 칸과 13.3·코드의 차이. (3) `migrate`의 깨끗한 작업 트리 검사가 주 체크아웃만 보는 것 |
| D10 (새) | 릴리스·태그·main 커밋·카탈로그 공개의 승인자와 순서 | 1.8절 순서. 각 단계를 사람이 승인한다 |
| D11 (H3 관련) | SmartScreen·macOS 공증이 npx 캐시의 바이너리 실행을 막는지 | 확인하지 않았다. 첫 공개 전에 깨끗한 Windows·macOS에서 확인한다 |

---

## 부록: 이 문서가 확인하지 않은 것

- npx의 git 의존 처리(files 필터, 실행 비트, prepare, 캐시 재사용). npx를 실행하지 않았다.
- `claude`·`codex`의 `marketplace add`가 ref를 받는지, 두 호스트의 플러그인 제거 명령의 꼴, 마켓플레이스를 지울 때 그 플러그인이 함께 빠지는지.
- `fullops install`을 통한 실제 설치 전체, 마켓플레이스 설치, grok 전역 hook 파일, Codex hook, macOS·Linux.
- 0단계 되돌리기 절차 전체와 손으로 하는 스킬 되살리기.
- Orca 패널의 Orca 안 실행, 실제 `approval_required` 출력, 하위 폴더 플러그인의 공개 목록 등재.
- `fullops-squad`·`fullops-meter`의 project 범위 항목을 누가 만들었는지. 공용 스킬을 사람이 먼저 깔았는지(`.skill-lock.json` 값은 열지 않았다).
- 옛 표식 목록은 `D:/workspace/*`와 `C:/Users/shin/orca/workspaces/*/*` 깊이만 훑었다. 더 깊은 자리나 다른 드라이브의 워크트리는 `git worktree list`로 2-0에서 다시 본다.
