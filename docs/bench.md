# 비교와 성능 측정

묶음 G의 도구 셋을 쓰는 법과 첫 측정 결과다. 기준은 설계 2.2절(성능 목표), 12.1절(비교 방법), 12.4절(합격 기준)이다.

| 도구 | 하는 일 | 내는 것 |
|---|---|---|
| `scenarios/drivers/py_driver.py` | 시나리오의 동사를 1.24.0의 hook 명령과 스크립트 호출로 옮겨 돌린다(`py` 구동기). 동사 대응은 파일 첫머리에 있다 | stdout의 JSON |
| `cargo xtask compare` | 같은 시나리오를 `py`·`rs` 두 구동기로 돌려 판정 종류와 흐름을 견준다. 12.2절의 ID 가운데 시나리오 파일이 없는 것과 남은 `pending`도 센다 | `scenarios/report.md` |
| `cargo xtask bench` | hook 대표 입력 13개와 명령 2개의 p50·p95·최대, git·Orca 프로세스 수, 상태 파일 쓰기 수를 잰다 | stdout의 Markdown 표 |

## 1. 준비

- 기준선 1.24.0의 사본(py-baseline) 위치를 환경 변수 `FULLOPS_PY_BASELINE`에 준다. 사본의 루트(그 안에 `plugins/fullops-squad/`가 있는 폴더)다. 이 변수가 없으면 `compare`와 `bench`는 `py` 쪽을 건너뛰고 `rs` 쪽만 낸다.
- Python 3.11 이상이 PATH에 있어야 한다(Windows는 `python`, 그 밖은 `python3`). `compare`와 `bench`는 PATH에서 찾은 전체 경로로 띄운다. `py` 구동기는 표준 라이브러리만 쓴다.
- 기준선 사본은 읽고 실행만 한다. 두 구동기 모두 임시 레포·임시 홈에서 돌고 testkit의 `fake-orca`·`fake-curl`·`fake-host`만 부른다. 실제 Orca, 호스트 CLI, 네트워크, 사용자 홈은 건드리지 않는다. `py` 구동기가 넘기는 `TYPESAFE_API_KEY=fake-key`는 1.24.0의 분류 스크립트가 `fake-curl`까지 가게 하는 가짜 값이다.

```powershell
$env:FULLOPS_PY_BASELINE = "<py-baseline 사본의 루트>"
```

```sh
export FULLOPS_PY_BASELINE="<py-baseline 사본의 루트>"
```

## 2. 합친 뒤 다시 돌릴 명령

묶음 B·C·D·E·F·J가 통합 브랜치에 합쳐진 뒤 이 순서로 돌린다.

1. `scenarios/k4.toml`의 `pending` 줄을 모두 지운다(다른 묶음의 시나리오에 남은 `pending`은 그 주인이 지운다).
2. 비교: 시나리오 폴더 전체를 돌리고 `scenarios/report.md`를 다시 쓴다. 회귀, 시나리오의 기대와 다른 단계(`rs`의 `expect`, `py`의 `py`), `py` 구동기가 끝까지 돌지 못한 시나리오 가운데 하나라도 있으면 종료 코드가 0이 아니다.

   ```sh
   cargo xtask compare
   ```

   시나리오 하나만 볼 때는 파일을 준다. 보고서를 다른 곳에 쓰려면 `--out`을 준다.

   ```sh
   cargo xtask compare scenarios/k4.toml --out target/k4-report.md
   ```

3. 보고서의 "합격 점검" 표에서 다섯 줄을 본다: 회귀 0건, 기대와 다른 단계 0건, 남은 `pending` 0건, 시나리오 파일이 없는 12.2절 ID 0개, `py` 구동기가 끝까지 돌지 못한 시나리오 0개(그런 시나리오는 견준 것이 아니다. 12.2절의 1.24.0 칸이 `-`인 행에서도 따로 센다). "끝까지 간 정도" 표에서 K4의 병합된 과제 수가 `py` 이상이고 lead 차단 수가 `py` 이하인지 본다.
4. 성능: 릴리스 빌드로 잰다. 다른 빌드가 돌지 않는 PC에서 돌린다. 결과 표를 이 문서의 4절에 옮겨 적는다.

   ```sh
   cargo run --release -p xtask -- bench
   ```

   횟수는 `--runs <n>`(기본 300, 처음 20회를 버린다)과 `--py-runs <n>`(기본 30, 1.24.0은 한 번에 1초 안팎이 걸린다)으로 바꾼다. `--check`를 주면 p95가 2.2절 목표의 두 배를 넘는 입력이 있을 때 실패한다(CI의 Windows 러너용).

5. `py` 구동기만 따로 돌릴 때(시나리오의 1.24.0 쪽 결과를 JSON으로 본다). `FULLOPS_FAKES`는 가짜 실행 파일이 있는 폴더다.

   ```sh
   cargo build -p fullops-testkit
   FULLOPS_FAKES=target/debug python -X utf8 -B scenarios/drivers/py_driver.py scenarios/k4.toml
   ```

6. 측정값으로 2.2절의 목표를 확정한다(설계 16절 미정-M9).

### 시나리오에 적는 비교용 키

`compare`만 읽는 단계 키가 둘 있다. 두 구동기는 이 키를 보지 않는다.

| 키 | 뜻 | 없을 때 |
|---|---|---|
| `df = "DF-n"` | 이 단계에서 두 판의 판정이 다른 것은 설계 12.3절의 그 차이다. 12.3절에 없는 ID를 적으면 회귀로 센다 | 그 파일에 `df`를 적은 단계가 하나도 없으면 시나리오 ID의 12.2절 행에 있는 DF 전부로 가린다(결과 칸에 "12.2절 <ID> 행:"이라고 나온다). 행 단위라 같은 행의 다른 단계에서 난 진짜 회귀도 함께 가려지므로, 판정이 다른 단계에는 `df`를 적는다. 한 단계라도 `df`를 적은 파일은 단계 단위로만 가린다 |
| `py = "<판정>"` | py 구동기가 이 단계에서 내야 하는 판정이다. 다르면 결과 칸에 `py 기대와 다름`이 붙고 "기대와 다른 단계"로 센다 | py 판정을 단언하지 않는다 |

`py`는 N3이 쓴다. py 구동기는 `raw`인 `fullops migrate …` 단계를 새 판 실행본으로 실제로 돌리므로(판정은 `n/a`), 전환한 레포에서 1.24.0의 hook이 통과를 내고 되돌린 레포에서 다시 판정하는 것을 `py = "pass"`와 `py = "deny"`로 확인한다.

## 3. 재는 방법

- 입력: 12.4절이 적은 대표 입력이다. 12.4절은 "12개"라고 적었지만 나열한 것은 13개이고, 나열한 대로 13개를 잰다. 여기에 명령 둘(`fullops rules --json`, `fullops status --json`)을 더했다.

  | 이름 | 입력 | 상태 |
  |---|---|---|
  | `inactive` | 비활성 레포의 PreToolUse(Bash `ls -la`) | 표식 없는 레포 |
  | `pre_bash` | lead의 PreToolUse(Bash `ls -la`) | |
  | `pre_edit` | worker의 PreToolUse(Edit `src/app.py`) | |
  | `pre_inbox_edit` | lead의 PreToolUse(Edit `handovers/to_dev.md`) | |
  | `pre_worker_start` | lead의 PreToolUse(Bash 날것 `orca orchestration worker-start`) | |
  | `pre_git_commit` | worker의 PreToolUse(Bash `git commit -m x`) | |
  | `stop_lead_clean` | lead의 Stop | 상태 파일 없음 |
  | `stop_lead_pending10` | lead의 Stop | 병합하지 않은 완료 보고 10건(Run 하나, dispatch·메시지·completion 열 개씩) |
  | `stop_lead_ready` | lead의 Stop | `PLANS.md`에 준비된 행 하나 |
  | `stop_lead_open_run` | lead의 Stop | 열린 Run 하나와 미정산 dispatch 하나 |
  | `stop_worker` | worker의 Stop | preamble로 dispatch가 묶였고 코드 커밋이 있으며 lint 기록이 없다. Orca는 미정산이라고 답한다 |
  | `session_start_lead`, `session_start_worker` | SessionStart(`startup`) | |
  | `cli_rules`, `cli_status` | `fullops rules --json`, `fullops status --json` | |

- 레포: testkit의 임시 레포(원격, `dev`·`architecture` 워크트리) 하나를 끝까지 쓴다. Git 공용 디렉터리에 `archive.jsonl` 5,000줄과 `turns.jsonl` 50,000줄을 넣어 둔다. `state.json`은 `docs/formats.md` 4.1절의 타입(`State`)으로 읽히는지 확인한 뒤에 쓴다.
- 시간: `fullops` 프로세스의 생성부터 종료까지의 벽시계다. 셸 없이 직접 띄운다. 호스트가 끼우는 셸의 시간은 들어 있지 않다. Stop은 한 번 막기가 매번 처음처럼 걸리도록 재기 전에 UserPromptSubmit을 한 번 넣어 턴을 넘긴다(그 시간은 넣지 않는다).
- 프로세스와 쓰기: 시간을 재는 실행과 따로 한 번 실행해 센다. git 프로세스는 git의 추적(`GIT_TRACE2_EVENT`)에서 `fullops`가 직접 띄운 것만 세고, Orca 호출은 `fake-orca`의 `calls.jsonl`로 센다. "쓰기"는 `<git-common-dir>/fullops/`와 `<git-dir>/fullops/` 아래에서 새로 생기거나 바뀐 파일 수다.
- 1.24.0 쪽(`py` 열): `py` 구동기가 같은 입력으로 기준선의 `hooks/hooks.json`에 등록된 명령을 순서대로 돌린 시간의 합이다. 1.24.0은 도구 호출마다 PreToolUse와 PostToolUse가 모두 돌므로 `pre_*`의 `py` 값은 둘의 합이다(PreToolUse가 거부하면 PreToolUse만이다). "py 프로세스"는 그 이벤트에서 뜬 hook 프로세스 수다. 상태는 같은 뜻이 되게 1.24.0의 방식으로 만든다(통합 대기 10건은 `orca_wait.py`가 받은 완료 보고 열 건이다).
- "본 것" 열은 세는 실행에서 나온 판정(hook)이나 종료 코드(명령)다. 그 입력이 뜻한 경로를 탔는지 확인하는 용도다.

## 4. 측정 결과

### 4.1 합친 뒤의 측정(2026-11-15)

구현·통합·검증 수정을 모두 합친 뒤(`5a418f2`), 다른 빌드가 돌지 않는 상태에서 잰 값이다. 규칙을 실제로 평가하는 경로다.

- 명령: `FULLOPS_PY_BASELINE=<py-baseline> cargo run --release -p xtask -- bench`(기본 횟수).
- 같은 때 이 PC의 프로세스 바닥값(60회, p50/p95): `cmd /c exit` 22.5/25.9ms, `git --version` 38.2/43.0ms, `fullops --version` 20.1/22.5ms. 첫 측정 때보다 PC 전체의 바닥값이 올라 있었다.

| 입력 | 본 것 | p50 | p95 | 최대 | 목표 p95 | git | orca | 쓰기 | py 본 것 | py p50 | py p95 | py 프로세스 | py git | py orca |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| inactive | pass | 22.3ms | 25.5ms | 38.6ms | 15ms | 0 | 0 | 0 | pass | 430.9ms | 443.7ms | 2 | 2 | 0 |
| pre_bash | pass | 23.3ms | 25.8ms | 45.2ms | 30ms | 0 | 0 | 1 | pass | 727.9ms | 745.8ms | 2 | 8 | 0 |
| pre_edit | pass | 23.6ms | 26.7ms | 34.9ms | 30ms | 0 | 0 | 1 | pass | 735.3ms | 756.1ms | 2 | 8 | 0 |
| pre_inbox_edit | pass | 23.7ms | 26.4ms | 38.9ms | 80ms | 0 | 0 | 0 | deny | 360.9ms | 376.8ms | 1 | 4 | 0 |
| pre_worker_start | deny | 23.6ms | 26.8ms | 34.8ms | 80ms | 0 | 0 | 1 | deny | 359.9ms | 380.0ms | 1 | 4 | 0 |
| pre_git_commit | pass | 64.8ms | 69.4ms | 89.0ms | 80ms | 1 | 0 | 0 | pass | 823.3ms | 846.2ms | 2 | 10 | 0 |
| stop_lead_clean | pass | 68.5ms | 74.9ms | 120.8ms | 150ms | 1 | 0 | 1 | pass | 1022.5ms | 1069.3ms | 2 | 15 | 0 |
| stop_lead_pending10 | block | 731.9ms | 769.4ms | 804.9ms | - | 21 | 0 | 2 | block | 1956.0ms | 2007.6ms | 2 | 35 | 0 |
| stop_lead_ready | block | 72.9ms | 79.4ms | 101.4ms | - | 1 | 0 | 2 | block | 1030.7ms | 1076.9ms | 2 | 25 | 0 |
| stop_lead_open_run | block | 70.9ms | 76.7ms | 97.5ms | - | 1 | 0 | 2 | block | 1072.4ms | 1123.1ms | 2 | 15 | 2 |
| stop_worker | block | 197.2ms | 209.9ms | 245.3ms | - | 4 | 1 | 3 | block | 991.8ms | 1028.1ms | 2 | 14 | 1 |
| session_start_lead | context | 29.7ms | 33.7ms | 40.4ms | 250ms | 0 | 0 | 1 | warn | 882.2ms | 901.0ms | 3 | 11 | 0 |
| session_start_worker | context | 25.6ms | 28.6ms | 36.1ms | 250ms | 0 | 0 | 1 | warn | 932.1ms | 956.2ms | 3 | 11 | 0 |
| cli_rules | 종료 코드 0 | 21.5ms | 25.6ms | 33.2ms | - | 0 | 0 | 0 | - | - | - | - | - | - |
| cli_status | 종료 코드 0 | 65.6ms | 69.6ms | 85.3ms | - | 0 | 0 | 2 | - | - | - | - | - | - |

읽을 것:
- 목표가 있는 입력 가운데 넘은 것은 `inactive`(p95 25.5ms, 목표 15ms) 하나다. 이 값은 아무것도 하지 않는 `cli_rules`(25.6ms)와 같고 `cmd /c exit`보다 낮다. 곧 이 PC에서 프로세스 하나를 띄우는 바닥값이다. 목표를 바닥값 기준(예: 바닥값 + 5ms)으로 다시 정해야 한다(미정-M9).
- 도구 호출 hook(`pre_*`)은 p95 26~27ms(목표 30ms), `git commit` 판정은 69ms(목표 80ms), lead Stop은 75~79ms(목표 150ms), SessionStart는 29~34ms(목표 250ms)다. 1.24.0 대비 도구 호출은 약 28배, Stop은 약 14배, SessionStart는 약 30배 빠르고, 띄우는 프로세스는 호출당 2~3개에서 1개다.
- 목표가 없는 입력 가운데 `stop_lead_pending10`(통합 대기 10건)이 732ms(git 21회)로 가장 느리다. 대기 건마다 통합 여부를 git으로 확인해서 건수에 비례한다. 1.24.0(1,956ms, git 35회)보다는 2.7배 빠르지만, 대기 건이 많은 레포에서는 lead Stop이 1초에 가까워질 수 있다. 확인을 한 번의 git 호출로 묶는 것이 다음 개선 거리다.
- `stop_worker`는 210ms(git 4회, Orca 1회)다. Orca의 worker-show 조회(X-03)가 대부분이다.

### 4.2 첫 측정(2026-11-14, 뼈대)

첫 측정(2026-11-14)이다. **참고값이다.** 이 수치로 2.2절의 목표를 확정하지 않는다. 이유는 둘이다.

- 측정하는 동안 같은 PC에서 다른 묶음 일곱 개가 빌드와 시험을 돌리고 있었다. 최댓값이 40ms 안팎으로 튀는 것은 그 부하다.
- 이 시점의 `fullops`는 뼈대다. 규칙 표가 비어 있어 모든 hook이 통과 경로이고(`본 것` 열이 전부 `pass`), 흐름 명령은 "구현 안 됨"이다(`cli_status`의 종료 코드 1). 그래서 `rs` 쪽 수치는 프로세스가 뜨고 입력을 읽고 통과를 내는 바닥값이고, git·Orca·쓰기가 0인 것도 규칙이 아무것도 평가하지 않아서다. 규칙을 평가하는 경로의 시간과 프로세스 수는 합친 뒤 2절의 4번으로 다시 잰다.

지금 읽을 수 있는 것은 두 가지다. 첫째, 이 PC에서 `fullops` 프로세스 하나의 바닥값이 p50 12~14ms다. 비활성 레포의 목표 15ms는 바닥값과 거의 같으므로, 합친 뒤의 측정에서 여유가 없으면 목표를 다시 정해야 한다(미정-M9). 둘째, 1.24.0은 같은 입력에 도구 호출 하나당 0.7~0.8초(PreToolUse와 PostToolUse, 프로세스 하나에 약 0.35초), Stop 하나당 1~2초, SessionStart 하나당 1초 남짓을 쓴다. 설계 2.2절에 적힌 기준선(1.17.0 실측: PreToolUse와 PostToolUse 각 약 500ms, Stop 약 730ms)과 같은 자릿수다. 이 값도 부하 속에서 잰 것이라 같은 조건으로 다시 잰다.

- PC: AMD Ryzen 5 5600(6코어 12스레드), RAM 32GB, Windows 11 Pro 10.0.26300.
- 도구: git 2.52.0.windows.1, Python 3.13.12, rustc 1.99.1.
- 명령: `cargo run --release -p xtask -- bench`(기본 횟수).
- 측정 환경: windows x86_64, 논리 코어 12개, 빌드 release, fullops 300회씩(처음 20회 버림), py 30회씩.

| 입력 | 본 것 | p50 | p95 | 최대 | 목표 p95 | git | orca | 쓰기 | py 본 것 | py p50 | py p95 | py 프로세스 | py git | py orca |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| inactive | pass | 12.9ms | 14.7ms | 40.0ms | 15ms | 0 | 0 | 0 | pass | 413.2ms | 453.2ms | 2 | 2 | 0 |
| pre_bash | pass | 12.2ms | 14.3ms | 37.7ms | 30ms | 0 | 0 | 0 | pass | 743.2ms | 1392.9ms | 2 | 8 | 0 |
| pre_edit | pass | 12.6ms | 14.5ms | 41.5ms | 30ms | 0 | 0 | 0 | pass | 720.5ms | 768.2ms | 2 | 8 | 0 |
| pre_inbox_edit | pass | 16.1ms | 20.7ms | 43.2ms | 80ms | 0 | 0 | 0 | deny | 341.2ms | 364.3ms | 1 | 4 | 0 |
| pre_worker_start | pass | 13.8ms | 16.2ms | 39.0ms | 80ms | 0 | 0 | 0 | deny | 345.9ms | 367.8ms | 1 | 4 | 0 |
| pre_git_commit | pass | 13.5ms | 16.7ms | 42.2ms | 80ms | 0 | 0 | 0 | pass | 790.1ms | 831.1ms | 2 | 10 | 0 |
| stop_lead_clean | pass | 13.8ms | 16.6ms | 19.0ms | 150ms | 0 | 0 | 0 | pass | 951.3ms | 1008.0ms | 2 | 15 | 0 |
| stop_lead_pending10 | pass | 13.4ms | 17.8ms | 37.8ms | - | 0 | 0 | 0 | block | 1857.3ms | 3677.2ms | 2 | 35 | 0 |
| stop_lead_ready | pass | 14.8ms | 18.4ms | 44.8ms | - | 0 | 0 | 0 | block | 1140.9ms | 1632.7ms | 2 | 25 | 0 |
| stop_lead_open_run | pass | 14.0ms | 18.4ms | 43.2ms | - | 0 | 0 | 0 | block | 2265.4ms | 2945.4ms | 2 | 15 | 2 |
| stop_worker | pass | 12.8ms | 14.9ms | 40.5ms | - | 0 | 0 | 0 | block | 1490.8ms | 2466.8ms | 2 | 14 | 1 |
| session_start_lead | pass | 13.2ms | 15.7ms | 40.4ms | 250ms | 0 | 0 | 0 | warn | 1231.1ms | 2322.6ms | 3 | 11 | 0 |
| session_start_worker | pass | 13.1ms | 15.4ms | 39.3ms | 250ms | 0 | 0 | 0 | warn | 1042.5ms | 1324.9ms | 3 | 11 | 0 |
| cli_rules | 종료 코드 0 | 14.3ms | 16.9ms | 40.8ms | - | 0 | 0 | 0 | - | - | - | - | - | - |
| cli_status | 종료 코드 1 | 14.7ms | 17.1ms | 41.7ms | - | 0 | 0 | 0 | - | - | - | - | - | - |

표를 읽는 법: `p50`·`p95`·`최대`는 `rs`(새 판)의 값이다. `목표 p95`는 2.2절의 값이고 `-`는 목표가 없는 입력이다. `git`·`orca`·`쓰기`는 그 호출 한 번에 `fullops`가 띄운 git 프로세스 수, Orca 호출 수, 바뀐 상태 파일 수다. `py`로 시작하는 열은 1.24.0이다. `py 본 것`이 `deny`인 두 입력은 PreToolUse가 거부해 PostToolUse가 돌지 않았다(프로세스 1개).

### 4.3 macOS 측정(2026-11-16)

`tourbut/dev`의 `ac1fe92`(2.0.1)를 실제 Mac에서, 다른 빌드가 돌지 않는 상태에서 잰 값이다. 1.24.0 기준선은 이 Mac에 없어 `py` 열은 비어 있다.

- 명령: `cargo run --release -p xtask -- bench`(기본 횟수, `FULLOPS_PY_BASELINE` 없음). `--py-runs 0`은 `--py-runs에 횟수가 필요합니다`로 거절된다. 기준선 변수가 없으면 횟수와 상관없이 py 측정을 건너뛰므로 인자를 빼면 된다.
- PC: Apple M2(논리 코어 8개), RAM 8GB, macOS 27.2.1(26B131). 도구: git 2.54.0(Apple Git-157), rustc 1.100.0.
- 같은 때 프로세스 바닥값(60회, p50/p95): `/usr/bin/true` 2.0/2.4ms, `git --version` 12.1/12.8ms, `fullops --version` 4.4/4.8ms.

| 입력 | 본 것 | p50 | p95 | 최대 | 목표 p95 | git | orca | 쓰기 |
|---|---|---|---|---|---|---|---|---|
| inactive | pass | 2.1ms | 2.2ms | 2.6ms | 15ms | 0 | 0 | 0 |
| pre_bash | pass | 2.2ms | 2.6ms | 3.3ms | 30ms | 0 | 0 | 1 |
| pre_edit | pass | 2.3ms | 2.5ms | 3.1ms | 30ms | 0 | 0 | 1 |
| pre_inbox_edit | pass | 2.3ms | 2.5ms | 2.6ms | 80ms | 0 | 0 | 0 |
| pre_worker_start | deny | 2.3ms | 2.5ms | 3.6ms | 80ms | 0 | 0 | 1 |
| pre_git_commit | pass | 11.6ms | 12.8ms | 14.7ms | 80ms | 1 | 0 | 0 |
| stop_lead_clean | pass | 14.3ms | 15.4ms | 16.4ms | 150ms | 1 | 0 | 1 |
| stop_lead_pending10 | block | 190.4ms | 209.7ms | 257.7ms | - | 21 | 0 | 2 |
| stop_lead_ready | block | 18.4ms | 20.3ms | 23.3ms | - | 1 | 0 | 2 |
| stop_lead_open_run | block | 15.4ms | 17.4ms | 26.0ms | - | 1 | 0 | 2 |
| stop_worker | block | 46.1ms | 56.4ms | 64.5ms | - | 4 | 1 | 3 |
| session_start_lead | context | 10.2ms | 16.6ms | 20.1ms | 250ms | 0 | 0 | 1 |
| session_start_worker | context | 8.9ms | 11.8ms | 15.6ms | 250ms | 0 | 0 | 1 |
| cli_rules | 종료 코드 0 | 9.1ms | 12.3ms | 23.8ms | - | 0 | 0 | 0 |
| cli_status | 종료 코드 0 | 116.4ms | 150.2ms | 288.1ms | - | 0 | 0 | 2 |

읽을 것:
- 목표가 있는 입력은 모두 목표 안이다. `inactive`도 p95 2.2ms로 목표 15ms 안이다. Windows에서 목표를 넘은 것은 그 PC의 프로세스 바닥값(20~22ms) 때문이었다는 4.1절의 판단과 맞다.
- 판정, git 수, Orca 수, 쓰기 수는 4.1절(Windows)과 모두 같다. 경로가 같고 시간만 다르다.
- hook 내부 기한(SessionStart·PreToolUse 4,000ms, UserPromptSubmit 1,500ms, Stop 12,000ms)과 견주면 가장 느린 Stop(`stop_lead_pending10`, 최대 258ms)도 기한의 2% 남짓이다.
- 기한 때문에 흔들리는지: `hook` 시험 묶음을 5회, `bundle_f`의 `n2_`를 3회 돌렸다. 한가할 때와 `cargo build --release`를 다른 target 폴더에서 처음부터 반복해 돌리는 부하 속에서 모두 통과했다(부하 때 `hook` 묶음 24~33초, 한가할 때 16~19초). GitHub Windows 러너에서 번갈아 실패한 `turns_are_counted_without_bodies`와 `owned_scenarios_pass`도 이 Mac에서는 실패하지 않았다.
- 같은 때 이 Mac의 툴체인은 release 빌드의 디버그 정보 제거(`strip`)에서 `rust-objcopy`가 `libLLVM.dylib`를 찾지 못한다는 경고를 냈다. 빌드는 끝나고 실행본은 그대로 돈다. 측정값에는 영향이 없다고 본다. 오래된 rustup(1.27.0)으로 툴체인을 올린 이 Mac의 사정이다.
