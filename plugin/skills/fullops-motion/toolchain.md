# 도구 준비와 로컬 실행

## 기본 경로: 단일 HTML 렌더

필요한 것은 Node 20 이상, 영상 폴더에 설치한 `playwright`(또는 시스템 Chrome과 `playwright-core`), FFmpeg와 FFprobe다. 이 스킬의 렌더만 Node를 쓴다. `fullops` 실행본과 hook은 Node가 필요 없다.

먼저 있는 것을 확인한다. `npx`의 자동 다운로드로 존재 검사를 대신하지 않는다.

```bash
node --version
ffmpeg -version
ls videos/<작업>/node_modules/playwright videos/<작업>/node_modules/playwright-core
```

없는 것이 있으면 설치 명령과 위치를 알리고 승인받은 뒤 준비한다. 영상 폴더의 `package.json`·lock 파일과 실제 설치 버전을 기록한다.

```bash
cd videos/<작업>
npm i -D playwright && npx playwright install chromium     # 브라우저까지 받는다
npm i -D playwright-core                                   # 시스템 Chrome을 쓸 때. 렌더에 --channel chrome
```

스틸과 렌더:

```bash
cd videos/<작업>
node <스킬 폴더>/render.mjs index.html --stills 0,0.5,1,1.5 --out qa/v01/stills --width 1920 --height 1080
node <스킬 폴더>/render.mjs index.html --out renders/v01/clip.mp4 --width 1920 --height 1080 --fps 60 --duration 15 --subframes 4 [--audio audio/mix.wav]
ffprobe -v error -count_frames -show_streams -show_format -of json renders/v01/clip.mp4
```

`<스킬 폴더>`는 이 문서가 있는 폴더다. `render.mjs`는 실행한 폴더(`videos/<작업>`)의 `node_modules`에서 Playwright를 찾는다.

- `render.mjs`는 페이지의 `window.seek(t)`를 프레임 시각마다 부르고 화면을 찍어 FFmpeg(H.264, yuv420p, BT.709/TV, 고정 프레임률)로 보낸다.
- `--stills`는 초 단위 시각 목록이고 `--out`은 폴더다. 파일 이름은 `t<초>.png`다.
- `--subframes N`은 한 프레임 안의 N개 시각을 평균해 모션 블러를 만든다. 픽셀 작품은 1로 둔다.
- 시스템 Chrome은 `--channel chrome`, PATH 밖의 FFmpeg는 `--ffmpeg <경로>`, 화질은 `--crf <값>`(기본 16)으로 지정한다.
- 이미 있는 MP4 경로는 거부한다(종료 코드 2). 수정 렌더는 새 버전 폴더에 낸다. 입력 오류는 2, 렌더 실패는 1이다.
- 페이지의 외부 요청을 막지 않는다. HTML이 로컬 자산만 참조하게 한다.

## HyperFrames·Remotion

사용자가 지정했거나 기존 프로젝트가 쓸 때만 준비한다(2026-10-07 검토 기준). 전 OS·CPU 지원을 보증하지 않는다. 실행한 환경과 한계를 검수 기록에 적는다.

| 구성 | 설치 정책 | 공식 원천 |
|---|---|---|
| HyperFrames CLI | `hyperframes@latest`, 설치된 패키지의 Node 요구사항 확인 | [공식 저장소](https://github.com/heygen-com/hyperframes) |
| HyperFrames 스킬 | 공식 저장소의 최신 `hyperframes` 스킬 | [공식 스킬 원천](https://github.com/heygen-com/hyperframes) |
| Remotion CLI·패키지 | `remotion@latest`·`@remotion/*@latest`, 한 프로젝트 안의 패키지 버전 일치 | [CLI](https://www.remotion.dev/docs/cli), [공식 스킬](https://www.remotion.dev/docs/ai/skills) |
| Remotion 스킬 | 공식 저장소의 최신 `remotion-best-practices` 스킬 | [공식 스킬 원천](https://github.com/remotion-dev/skills) |

| 작업 | 읽을 공식 스킬 |
|---|---|
| 짧은 무내레이션 HTML 모션 | HyperFrames `hyperframes` → `motion-graphics`와 core·animation·creative·cli |
| 내레이션·긴 영상·여러 장면 | `hyperframes` → `general-video`(제품 URL 기반 소개는 `product-launch-video`) |
| React 소스·프레임 단위 애니메이션·클립 합성 | Remotion `remotion-best-practices` → create·markup·studio·render |
| 기존 Remotion 소스를 HTML로 옮기는 명시적 요청 | HyperFrames `remotion-to-hyperframes`. 번역 한계와 전후 프레임 비교를 기록한다 |

### 기존 로컬 환경 확인

1. 영상 폴더의 `package.json`·lock 파일과 로컬 실행 파일을 먼저 확인한다. Node·브라우저·FFmpeg·FFprobe 버전과 OS·CPU를 기록한다. HyperFrames는 `--version`, Remotion은 `versions`로 실제 설치를 확인한다.
2. 설치·업데이트를 요청받았으면 별도 작업 폴더에서 latest를 설치하고 호환을 확인한다. 기존 작업의 패키지와 lock은 보존한다.
3. 누락된 도구·스킬과 실행할 수 없는 검사를 보고한다. 도구나 전역 스킬을 자동으로 설치·갱신하지 않는다.

설치된 공식 스킬에서 필요한 진입 문서와 상대 참조를 읽고, 그 출처와 실제 설치 ref를 기록한다. 공식 스킬 전체를 복사하거나 독자 포크를 유지하지 않는다.

### 로컬 명령

해당 버전 CLI의 `--help`와 스킬 참조를 읽고 실행한다. `HYPERFRAMES_SKIP_SKILLS=1`로 init의 자동 스킬 조회를 막는다. 기본 blank 예제의 외부 GSAP·글꼴 URL은 로컬 자산으로 바꾼다. registry·add·capture는 별도의 네트워크 작업이다.

```bash
HYPERFRAMES_SKIP_SKILLS=1 DO_NOT_TRACK=1 ./node_modules/.bin/hyperframes init clip --non-interactive --example blank --skill motion-graphics
DO_NOT_TRACK=1 ./node_modules/.bin/hyperframes preview clip --background
DO_NOT_TRACK=1 ./node_modules/.bin/hyperframes check clip --snapshots --json
DO_NOT_TRACK=1 ./node_modules/.bin/hyperframes render clip --strict --fps 30 --output renders/v01/clip.mp4
./node_modules/.bin/remotion studio src/index.jsx
./node_modules/.bin/remotion render src/index.jsx Intro renders/v01/intro.mp4 --color-space bt709
```

브라우저와 FFmpeg가 PATH에 없으면 실행 프로세스에만 경로를 준다. HyperFrames는 `HYPERFRAMES_BROWSER_PATH`·`HYPERFRAMES_FFMPEG_PATH`·`HYPERFRAMES_FFPROBE_PATH`, Remotion은 `--browser-executable`이다. 시스템 PATH는 바꾸지 않는다.

HyperFrames와 Remotion은 브라우저를 기본 `--no-sandbox`로 띄운다. 신뢰하는 로컬 소스만 열고 그 실행 경계를 기록한다. 프로젝트가 sandbox 유지를 요구하면 실행을 보류한다. 보안 해제 옵션이나 권한 상승을 스스로 더하지 않는다.

## 비용과 라이선스

- Playwright는 [Apache-2.0](https://github.com/microsoft/playwright), HyperFrames는 [Apache-2.0](https://github.com/heygen-com/hyperframes)이다. 외부 스톡·글꼴·음성의 권리는 별도로 확인한다.
- Remotion은 [현재 라이선스](https://www.remotion.dev/docs/license)의 사용 주체 조건을 확인한다. 개인, 직원 3명 이하 영리 조직, 비영리 조직, 아직 상업적으로 쓰지 않는 적합성 평가가 무료 대상이고 그 밖의 조직은 Company License가 필요하다. 자격이 불명확하면 실행을 보류한다. 로컬 렌더라는 이유로 무료라고 단정하지 않는다.
- 실행은 로컬 자산과 로컬 렌더로 제한한다. 클라우드 렌더(Lambda, Cloud Run), 외부 TTS·생성 이미지 API, usage·feedback·publish 업로드를 하지 않는다. HyperFrames CLI에는 `DO_NOT_TRACK=1`을 준다.
- 기록과 스크린샷에 키나 민감 정보를 넣지 않는다.
