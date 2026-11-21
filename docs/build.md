# 빌드와 배포본

이 문서는 fullops를 빌드해 배포본을 만드는 절차를 적는다. 설계의 정본은 `docs/design.md` 10.4절(패키지 배치와 hook 등록)과 10.5절(빌드)이다.

## 준비물

- Rust 툴체인(에디션 2024를 지원하는 안정판)과 `cargo`.
- 시험에는 `git`이 필요하다. 그 밖의 바깥 도구는 필요 없다. 시험은 임시 홈과 가짜 실행 파일(`fake-orca`, `fake-host`, `fake-curl`)로 돈다.
- 외부 크레이트는 일곱 개다: `serde`, `serde_json`, `toml`, `fancy-regex`, `globset`, `sha2`, `lexopt`.

## 개발 빌드와 시험

```
cargo build
cargo test -p fullops --test bundle_f     # 설치·이행·배포 시나리오(묶음 F)
node --test npx/fullops.test.cjs          # npx 실행기(package.json의 test와 같다)
cargo check --target aarch64-apple-darwin
cargo check --target x86_64-unknown-linux-gnu
```

시험은 실제 호스트 CLI(`claude`·`codex`·`grok`·`agy`), `npx`, 사용자 홈을 건드리지 않는다. 설치 시나리오는 PATH를 가짜 실행 파일 폴더 하나로 좁혀 돌린다. 새 설치 시나리오를 쓸 때도 `install`·`doctor` 단계에는 `env = { PATH = "../bin" }`와 `raw = true`를 준다(`crates/fullops/tests/bundle_f.rs`의 `guard`가 확인한다).

## 배포본 만들기

```
cargo xtask dist [--root <폴더>] [--target <타깃>]... [--prebuilt <폴더>] [--no-validate]
```

만드는 것은 다음과 같다. `--root`의 기본은 `target/dist`다(`.gitignore`의 `/target/` 안이라 개발 중에 만든 배포본이 커밋되지 않는다). 저장소 루트에 만드는 것은 릴리스 때(`--root .`, 아래 '릴리스 워크플로')뿐이다.

| 자리(`<루트>` 기준) | 내용 |
|---|---|
| `dist/native/fullops/` | 통합 패키지. Claude Code와 Codex가 함께 읽고 grok·agy 패키지의 재료(`hosts/`)를 싣는다 |
| `dist/directory/fullops/` | 디렉터리판. Claude Code 전용이고 Codex 파일·`hosts/`·의존 플러그인 선언이 없다. hook이 `--in-place`로 돈다 |
| 두 폴더의 `SHA256SUMS` | 파일별 sha256(`<해시>  <상대 경로>`) |
| `.claude-plugin/marketplace.json`, `.agents/plugins/marketplace.json` | 통합 패키지 폴더(`./dist/native/fullops`)를 가리키는 마켓플레이스 카탈로그 둘 |

재료와 만들어지는 파일의 대응은 다음과 같다. 배포본을 손으로 고치지 않는다. 재료를 고치고 다시 만든다.

| 배포본의 파일 | 재료 |
|---|---|
| `.claude-plugin/plugin.json`, `.codex-plugin/plugin.json`, `hosts/grok/plugin.json`, `hosts/agy/plugin.json` | `plugin/plugin.json`(공통 값)에 `plugin/hosts/<호스트>/plugin.json`(호스트별 덧붙임)을 얹고 버전을 넣는다. grok과 agy는 그 호스트의 스키마가 받는 필드만 남긴다. 디렉터리판은 `plugin/hosts/claude-directory/plugin.json`을 쓴다 |
| `.claude-plugin/plugin.json`의 `dependencies`, `.mcp.json`, `hosts/grok/.mcp.json`, `hosts/agy/mcp_config.json` | `plugin/deps.toml`의 `[[plugins]]`(Claude Code가 대상인 것)과 `[[mcp]]` |
| `hooks/hooks.json`, `hooks/codex.json`, `hosts/agy/hooks.json` | `fullops_core::deps`의 표(등록 타임아웃 `HOOK_TIMEOUTS`, PreToolUse 매처 `pre_tool_matcher`)와 `xtask/src/dist.rs`의 등록 문자열. grok의 등록 파일은 배포하지 않고 `fullops install`이 쓴다(`grok_hook_json`) |
| `skills/` | `plugin/skills/`를 하위 폴더와 파일째 복사한다 |
| `bin/fullops` | `plugin/bin/fullops`(POSIX sh 런처) |
| `bin/fullops.exe`, `bin/fullops-darwin-arm64`, `bin/fullops-darwin-x64`, `bin/fullops-linux-x64` | 타깃별 릴리스 바이너리 |
| `deps.toml`, `INSTALL.md`, `releases/*.md`, `VERSION`, `LICENSE` | `plugin/deps.toml`, `plugin/INSTALL.md`, `crates/fullops/assets/releases/`, 워크스페이스 버전, 저장소 루트의 `LICENSE`(있을 때) |

끝에 G4를 검사한다. 두 배포 폴더에서 `.py`·`.js`·`.cjs`·`.mjs`를 찾아 `skills/fullops-motion/render.mjs` 밖의 것이 있으면 실패한다. 검사 대상은 두 배포 폴더뿐이다. Orca 패널(`panel/`)은 hook·CLI와 따로 배포하는 Orca 플러그인이라 배포 폴더에 복사하지 않으므로 그 안의 `.mjs`·`.js`는 이 검사에 걸리지 않는다. 시나리오 구동기(`scenarios/drivers/py_driver.py`)도 배포 폴더에 들어가지 않는다.

### 타깃별 바이너리

타깃은 넷이다: `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`. `--target`을 주지 않으면 넷 모두를 싣는다.

- 한 PC에서 네 타깃을 모두 링크할 수는 없다. CI(OS별 러너)가 타깃마다 `cargo build --release -p fullops --target <타깃>`으로 빌드하고, 산출물을 패키지 안의 이름(`fullops.exe`, `fullops-darwin-arm64`, `fullops-darwin-x64`, `fullops-linux-x64`)으로 한 폴더에 모은 뒤 `cargo xtask dist --prebuilt <그 폴더>`를 실행한다.
- `--prebuilt` 폴더에 없는 타깃은 그 자리에서 빌드한다. 빌드가 실패하면 멈추고 어느 타깃인지 알린다.
- 한 타깃만 실어 시험해 보려면 `--target <타깃>`을 준다.
- 릴리스 프로필은 `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `panic = "unwind"`, `strip = true`다(`Cargo.toml`). `panic = "unwind"`는 hook의 `catch_unwind` 때문에 바꾸지 않는다.

### 실행 비트

`bin/fullops`(런처)와 macOS·Linux 바이너리는 실행 비트가 있어야 한다. `xtask dist`는 POSIX에서 실행했을 때만 실행 비트를 준다(Windows의 파일 시스템에는 실행 비트가 없다). 그래서 릴리스 워크플로는 배포본을 Linux 러너에서 만든다(아래 '릴리스 워크플로'). 저장소의 `plugin/bin/fullops`도 같은 모드(100755)로 커밋한다.

### 검증

`--no-validate`를 주지 않으면 만든 뒤 호스트 CLI의 검증 명령을 돌린다. 각각 그 CLI가 PATH에 있을 때만 돌리고, 없으면 건너뛰고 알린다.

| 명령 | 대상 |
|---|---|
| `claude plugin validate <통합 패키지>` | 통합 패키지 |
| `claude plugin validate --strict <디렉터리판>` | 디렉터리판 |
| `grok plugin validate <폴더>` | 통합 패키지의 재료로 임시 폴더에 조립한 grok 패키지 |
| `agy plugin validate --strict <폴더>` | 같은 방법으로 조립한 agy 패키지 |

- 조립은 `fullops install`이 쓰는 것과 같은 표(`fullops_core::deps::package_items`)로 한다.
- Codex의 검증 명령은 없다.
- 호스트 CLI에 `--help`를 붙여 부르지 않는다. agy의 `plugin` 하위 명령은 `--help`를 인자 값으로 받아 실제로 동작한다.
- 시험과 개발 중에는 `--no-validate`를 준다. 실제 호스트 CLI를 부르는 것은 배포 담당이 배포 직전에 한 번 한다.

## 폐쇄망 빌드

네트워크 없이 빌드하려면 의존 크레이트를 미리 받아 저장소 밖 보관소에 둔다.

1. 망이 열린 PC에서 저장소 루트에서 `cargo vendor <보관소>/vendor`를 실행한다. 출력의 `[source]` 설정을 적어 둔다.
2. `<보관소>/vendor` 폴더와 `Cargo.lock`을 폐쇄망 PC로 옮긴다. 보관소는 저장소 밖에 둔다(저장소에 커밋하지 않는다).
3. 폐쇄망 PC의 사용자 설정(`~/.cargo/config.toml`)이나 빌드할 때만 쓰는 설정 파일에 1번의 `[source]` 설정을 넣는다. 저장소의 `.cargo/config.toml`은 고치지 않는다.

   ```toml
   [source.crates-io]
   replace-with = "vendored-sources"

   [source.vendored-sources]
   directory = "<보관소>/vendor"
   ```

4. `cargo build --release --offline -p fullops`로 빌드한다. 시험은 `cargo test --offline`이다. 교차 타깃의 표준 라이브러리(`rustup target add`)도 망이 열린 곳에서 미리 받아 둔다.
5. `Cargo.lock`이 바뀌면 1번부터 다시 한다.

실행할 때의 폐쇄망 동작은 따로다. `FULLOPS_OFFLINE=1`이면 `fullops install`은 실행본 단계만 하고, Jev 분류와 탐색은 로컬 후보로 동작한다. 통합 패키지는 바이너리를 싣고 있으므로 폐쇄망에서도 내려받기 없이 설치된다.

## 릴리스 워크플로

`.github/workflows/release.yml`이 릴리스 커밋을 만들어 PR로 올린다(`docs/install-and-retire.md` 1.8절, 5절 D1·D2·D4·D5·D10). 릴리스 커밋과 패널 카탈로그 항목은 손으로 만들지 않는다. 사람이 PR을 보고 머지하는 것이 공개 승인이다. 워크플로는 main에 바로 push하지 않는다. 릴리스 PR이 머지되면 같은 워크플로의 `publish` 잡이 머지 커밋에 태그 `v<버전>`과 GitHub Release를 만든다. 서명 없이 시작하고 출처는 `tourbut/fullops`(`plugin/deps.toml`의 `[self].source`)다.

- 릴리스 커밋에는 `dist/native/fullops`(바이너리 넷 포함), 카탈로그 둘(`.claude-plugin/marketplace.json`, `.agents/plugins/marketplace.json`), Orca 패널 카탈로그 `orca-catalog.json`만 싣는다. `dist/directory/`는 싣지 않는다. 개발 커밋은 이것들을 건드리지 않는다.
- 돌리기 전에 `Cargo.toml`의 workspace 버전과 `package.json`의 `version`을 함께 올리고 `crates/fullops/assets/releases/<버전>.md`를 써서 main에 머지해 둔다. 패널을 바꿨으면 `panel/orca-plugin.json`의 `version`도 올린다. 그 뒤 GitHub Actions에서 `release`를 main에서 수동 실행(workflow_dispatch)하고 버전을 넣는다.

| 잡 | 하는 일 |
|---|---|
| `check` | main에서 돌았는지, 입력 버전이 `Cargo.toml`·`package.json`의 버전과 같은지, 릴리스 노트가 있는지 본다. `cargo xtask orca-catalog --root .`로 패널 버전이 겹치지 않는지도 네 빌드 전에 미리 본다 |
| `test` | `ci.yml`을 부른다(Windows·macOS·Linux의 빌드·시험, 교차 검사, 패널과 npx 실행기의 `node --test`) |
| `build` | 타깃마다 `cargo build --release -p fullops --target <타깃>`. Windows 러너가 `x86_64-pc-windows-msvc`, macOS 러너가 `aarch64-apple-darwin`과 `x86_64-apple-darwin`, Linux 러너가 `x86_64-unknown-linux-gnu`를 빌드하고 패키지 안의 이름으로 아티팩트에 올린다 |
| `dist` | Linux 러너에서 바이너리 넷을 모아 `cargo xtask dist --root . --prebuilt prebuilt --no-validate`, 이어 `cargo xtask orca-catalog --root . --commit <이 실행의 main 커밋>`. 새 브랜치 `release/v<버전>-<실행 번호>`에 커밋하고 main으로 PR을 연다 |
| `publish` | 수동 실행이 아니라 `release/v<버전>-<실행 번호>` PR이 main에 머지될 때(`pull_request` `closed`, `merged`) 돈다. 브랜치 이름에서 버전을 읽어 `gh release create v<버전> --target <머지 커밋> --title "fullops <버전>" --notes-file crates/fullops/assets/releases/<버전>.md`. 앞의 네 잡은 이 이벤트에서 건너뛴다 |

- POSIX 러너에서 만들므로 런처와 macOS·Linux 바이너리의 실행 비트가 커밋에 잡힌다(위 '실행 비트').
- 저장소 설정에서 'Allow GitHub Actions to create and approve pull requests'가 켜져 있어야 PR을 열 수 있다. `GITHUB_TOKEN`이 연 PR에서는 `ci` 워크플로가 다시 돌지 않는다(이미 `test` 잡에서 돌았다).
- 호스트 CLI 검증은 CI에서 하지 않는다(`--no-validate`). 머지 전에 사람이 그 브랜치를 받아 위 '검증'의 명령을 돌린다.
- 태그는 워크플로 시점이 아니라 머지 뒤에 건다. 사용자가 받는 배포본은 main에 머지된 트리이고, 머지 전에는 그 커밋이 없다(2.0.x 태그도 릴리스 PR의 머지 커밋에 걸었다). 머지하지 않고 닫은 PR은 태그를 남기지 않는다. GitHub Release에는 2.0.x처럼 바이너리 자산을 붙이지 않는다. 바이너리는 git 트리의 `dist/native/fullops/bin`에 있다.
- 태그나 Release가 이미 있으면 `publish`는 덮지 않고 실패한다. 빠진 판은 손으로 만든다: `gh release create v<버전> --target <머지 커밋> --title "fullops <버전>" --notes-file crates/fullops/assets/releases/<버전>.md`(태그가 이미 있으면 `--target`은 무시된다).

### Orca 패널 카탈로그

패널은 hook·CLI와 따로 Orca 플러그인으로 배포한다. Orca의 git 직접 설치는 저장소 루트만 읽으므로, 저장소 루트의 `orca-catalog.json`(카탈로그 v2)이 `source.path: "panel"`로 패널 폴더를 가리킨다. 설치 키는 `tourbut.fullops-panel`이다.

```
cargo xtask orca-catalog [--root <폴더>] [--commit <SHA>]
```

- `--root`의 기본은 `xtask dist`와 같은 `target/dist`다. 저장소 루트(`--root .`)에 쓰는 것은 릴리스 워크플로뿐이다. `--commit`을 빼면 HEAD를 쓰는데, 개발 브랜치의 커밋은 squash 머지 뒤 main에 남지 않아 그 항목으로는 사용자가 패널을 설치하지 못한다.
- `orca plugin release-entry panel`이 내는 releases 항목을 같은 규칙으로 만든다(CI에는 orca CLI가 없다). 그 커밋의 `panel/` git 객체를 읽어 Orca의 내용 해시(`contentHash`)를 재고, `orca-plugin.json`에서 `version`·`execution`·`minimumOrca`·`name`·`contributes`·`capabilities`를 옮긴다. 해시 규칙은 Orca 1.4.256의 `hashPluginTree`와 대조했다(`xtask/src/orca_catalog.rs`의 시험).
- `source`는 `{kind: "git", url: "https://github.com/tourbut/fullops.git", ref: "main", path: "panel"}`다. `ref`는 Orca가 받아 오는 브랜치이고, 패널 버전은 각 release의 `commit`(40자 SHA)으로 가리킨다(D5). 본체 태그 `v<버전>`과 겹치지 않는다.
- `commit`은 릴리스 커밋이 아니라 워크플로가 돈 main 커밋이다. 릴리스 커밋은 PR이 squash 머지되면 main에 남지 않지만, 그 부모인 main 커밋은 남는다. 그래서 `release`를 main에서만 돌린다.
- 이미 `<루트>/orca-catalog.json`이 있으면 releases를 이어 쓴다. 패널 내용이 그대로면 항목을 늘리지 않고, 내용이 바뀌었는데 `panel/orca-plugin.json`의 `version`이 이미 있으면 멈춘다. 새 항목은 맨 앞에 두고 32개까지 남긴다.
- 패널이 `panels` 밖의 기여(명령·단축키 등)를 더하면 그 요약 규칙을 `xtask/src/orca_catalog.rs`에 더해야 한다. 그 전까지는 멈춘다(요약이 매니페스트와 다르면 Orca가 설치를 거절한다).
- 첫 공개 전에 사람이 패널을 Orca에서 실제로 돌려 본다(`panel/README.md` '확인', 미정-M4). 머지 전에 `orca plugin validate orca-catalog.json --json`으로 카탈로그를 확인하고, 할 수 있으면 그 커밋을 체크아웃해 `orca plugin release-entry panel --json`과 releases 맨 앞 항목을 비교한다.

사용자 설치는 `orca plugin catalog add tourbut/fullops`(카탈로그 승인) → `orca plugin install tourbut.fullops-panel`(코드 실행 승인)이다(`panel/README.md` '설치').

## 유통

레포 주인이 정했다(2026-11-15, `docs/design.md` 17절 H3·H14·H16, `docs/install-and-retire.md` 5절).

- 출처는 `tourbut/fullops`의 기본 브랜치(main) 하나다. `npx --yes github:tourbut/fullops install|update|check`와 Claude Code·Codex의 `marketplace add tourbut/fullops`가 같은 트리를 읽는다. 서명은 하지 않는다.
- main 루트에 두는 것: `package.json`과 `npx/fullops.cjs`(실행기. 개발 커밋에서도 바뀐다), 그리고 **릴리스 커밋에서만 바뀌는** `dist/native/fullops/`(바이너리 넷 포함)와 카탈로그 둘(`.claude-plugin/marketplace.json`, `.agents/plugins/marketplace.json`), Orca 카탈로그 `orca-catalog.json`.
- 개발 커밋은 `dist/`와 세 카탈로그를 건드리지 않는다. `xtask dist`의 기본 루트가 `target/dist`라 평소에는 저장소 루트에 생기지 않는다. `dist/directory/`는 `.gitignore`가 막는다(디렉터리판은 저장소에 두지 않는다).
- 실행기(`npx/fullops.cjs`)는 저장소 루트에 있고 배포 폴더 밖이라 G4 검사와 부딪치지 않는다. Node 내장 모듈만 쓰고 npm 의존과 `prepare` 스크립트가 없다(있으면 npx가 받을 때 그것을 설치·실행한다).
- 실행기는 바이너리를 고르고 `SHA256SUMS`로 검증한 뒤 셸 없이 `fullops install`에 넘기기만 한다. `SHA256SUMS`는 전송 손상만 잡는다.

### 릴리스에서 사람이 하는 일

릴리스 절차는 위 '릴리스 워크플로' 하나다. 릴리스 커밋, 카탈로그, 패널 항목을 손으로 만들어 main에 커밋하지 않는다. 사람이 하는 일은 다음뿐이다(`docs/install-and-retire.md` 1.8절, D10).

1. 한 번만: 저장소 설정에서 'Allow GitHub Actions to create and approve pull requests'를 켠다.
2. 버전(`Cargo.toml`, `package.json`, 패널을 바꿨으면 `panel/orca-plugin.json`)을 올리고 릴리스 노트를 써서 main에 머지한 뒤, `release`를 main에서 수동 실행한다.
3. **(승인)** 워크플로가 연 PR을 검토한다. 호스트 CLI(`claude`·`grok`·`agy`)가 있는 PC에서 그 브랜치를 받아 `cargo xtask dist --root <임시 폴더> --prebuilt dist/native/fullops/bin`을 돌려 출력에 '검증을 건너뛴다'가 없는지 본다(PR의 바이너리로 다시 조립해 위 '검증'을 모두 돌린다). `orca plugin validate orca-catalog.json --json`과 바이너리 모드(`git ls-files -s dist/native/fullops/bin`이 100755)도 본다. 통과하면 머지한다. 머지가 공개 승인이다.
4. 머지하면 `publish` 잡이 태그 `v<버전>`과 GitHub Release를 만든다. Actions에서 그 실행이 성공했는지 본다.
5. 깨끗한 PC나 임시 HOME에서 `npx --yes github:tourbut/fullops install --dry-run` → 실제 설치 → `install --check` → `doctor` 순서로 확인한다.

이력이 커지는 것이 문제가 되면 기본 브랜치를 배포 전용 브랜치로 바꾸는 쪽으로 옮긴다. 사용자가 등록한 마켓플레이스 출처(`tourbut/fullops`)는 그대로 둘 수 있다.
