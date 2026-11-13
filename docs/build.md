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
cargo check --target aarch64-apple-darwin
cargo check --target x86_64-unknown-linux-gnu
```

시험은 실제 호스트 CLI(`claude`·`codex`·`grok`·`agy`), `npx`, 사용자 홈을 건드리지 않는다. 설치 시나리오는 PATH를 가짜 실행 파일 폴더 하나로 좁혀 돌린다. 새 설치 시나리오를 쓸 때도 `install`·`doctor` 단계에는 `env = { PATH = "../bin" }`와 `raw = true`를 준다(`crates/fullops/tests/bundle_f.rs`의 `guard`가 확인한다).

## 배포본 만들기

```
cargo xtask dist [--root <폴더>] [--target <타깃>]... [--prebuilt <폴더>] [--no-validate]
```

만드는 것은 다음과 같다. `--root`의 기본은 저장소 루트다.

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

끝에 G4를 검사한다. 두 배포 폴더에서 `.py`·`.js`·`.cjs`·`.mjs`를 찾아 `skills/fullops-motion/render.mjs` 밖의 것이 있으면 실패한다.

### 타깃별 바이너리

타깃은 넷이다: `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`. `--target`을 주지 않으면 넷 모두를 싣는다.

- 한 PC에서 네 타깃을 모두 링크할 수는 없다. CI(OS별 러너)가 타깃마다 `cargo build --release -p fullops --target <타깃>`으로 빌드하고, 산출물을 패키지 안의 이름(`fullops.exe`, `fullops-darwin-arm64`, `fullops-darwin-x64`, `fullops-linux-x64`)으로 한 폴더에 모은 뒤 `cargo xtask dist --prebuilt <그 폴더>`를 실행한다.
- `--prebuilt` 폴더에 없는 타깃은 그 자리에서 빌드한다. 빌드가 실패하면 멈추고 어느 타깃인지 알린다.
- 한 타깃만 실어 시험해 보려면 `--target <타깃>`을 준다.
- 릴리스 프로필은 `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `panic = "unwind"`, `strip = true`다(`Cargo.toml`). `panic = "unwind"`는 hook의 `catch_unwind` 때문에 바꾸지 않는다.

### 실행 비트

`bin/fullops`(런처)와 macOS·Linux 바이너리는 실행 비트가 있어야 한다. `xtask dist`는 POSIX에서 실행했을 때만 실행 비트를 준다(Windows의 파일 시스템에는 실행 비트가 없다). 그래서 배포 커밋은 POSIX 러너에서 만든 배포본으로 한다. Windows에서 만든 배포본을 커밋해야 하면 `git update-index --chmod=+x`로 `bin/fullops`, `bin/fullops-darwin-arm64`, `bin/fullops-darwin-x64`, `bin/fullops-linux-x64`의 모드를 직접 잡는다. 저장소의 `plugin/bin/fullops`도 같은 모드(100755)로 커밋한다.

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

## 유통

바이너리의 유통 방식과 서명, 마켓플레이스 출처는 사람이 정한다(`docs/design.md` 17절 H3). 출처가 정해지면 `plugin/deps.toml`의 `[self].source`만 바꾼다. `dist/`는 `xtask dist`의 생성물이고 개발 브랜치에는 커밋하지 않는다.
