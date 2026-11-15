#!/usr/bin/env node
// npx 실행기: 통합 패키지에서 이 PC의 바이너리를 골라 검증한 뒤 `fullops install`에 넘기기만 한다(docs/install-and-retire.md 1.3·1.4절).
// 설치 로직(호스트 CLI, 마켓플레이스, 스킬, 출처 충돌)은 모두 Rust `fullops install`에 있다. Node 내장 모듈만 쓴다.
'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const HOSTS = ['claude', 'codex', 'grok', 'agy'];
const PACKAGE = path.join(__dirname, '..', 'dist', 'native', 'fullops');
const USAGE = `fullops 설치 실행기

npx --yes github:tourbut/fullops <install|update|check|help> [--hosts claude,codex,grok,agy|all] [--dry-run] [--repo <경로>]

  install    fullops install을 실행한다(실행본, 호스트 플러그인, 의존 플러그인·스킬)
  update     install과 같다. 새 판의 갱신은 다시 설치하는 것이다
  check      fullops install --check. 설치하지 않고 빠진 것만 낸다(빠진 것이 있으면 종료 코드 2)
  --hosts    고를 호스트. claude-code는 claude로 받는다. 주지 않으면 PATH에 있는 호스트 모두
  --dry-run  부를 명령과 쓸 경로만 낸다. 아무것도 바꾸지 않는다
  --repo     설치 뒤 그 레포가 활성 레포면 fullops upgrade를 부른다. 아직 켜지 않은 레포와 Python판 레포는 안내만 한다

종료 코드는 fullops의 것을 그대로 돌려준다: 0 성공, 1 사용법 오류, 2 빠짐·거절, 4 바깥 명령 실패.`;
const PANEL =
  'Orca 패널(선택): orca plugin catalog add tourbut/fullops 뒤 orca plugin install tourbut.fullops-panel. 코드를 실행하는 플러그인이라 Orca가 보여 주는 --approve 명령으로 사람이 승인한다(INSTALL.md의 Orca 패널).';

/** 실행기가 멈추는 이유와 종료 코드(1 사용법, 2 거절, 4 실행 실패). */
class Stop extends Error {
  constructor(message, code) {
    super(message);
    this.code = code;
  }
}

function hostList(value) {
  const names = value.split(',').map(n => n.trim()).filter(Boolean).map(n => (n === 'claude-code' ? 'claude' : n));
  const unknown = names.find(n => n !== 'all' && !HOSTS.includes(n));
  if (unknown !== undefined || names.length === 0)
    throw new Stop(`모르는 호스트입니다: ${unknown ?? value}. ${HOSTS.join(', ')}, all 가운데 고릅니다`, 1);
  return names.includes('all') ? 'all' : [...new Set(names)].join(',');
}

function parseArgs(argv) {
  const args = [...argv];
  const first = args.shift();
  const options = { command: first ?? 'help', hosts: null, dryRun: false, repo: null };
  if (['--help', '-h'].includes(options.command)) options.command = 'help';
  if (!['help', 'install', 'update', 'check'].includes(options.command))
    throw new Stop(`모르는 명령입니다: ${options.command}. install, update, check, help 가운데 고릅니다`, 1);
  while (args.length) {
    const flag = args.shift();
    if (flag === '--dry-run') options.dryRun = true;
    else if (flag === '--help' || flag === '-h') options.command = 'help';
    else if (flag === '--hosts' || flag === '--repo') {
      const value = args.shift();
      if (!value || value.startsWith('--')) throw new Stop(`${flag}에 값이 없습니다`, 1);
      if (flag === '--hosts') options.hosts = hostList(value);
      else options.repo = path.resolve(value);
    } else if (flag === '--codex-home' || flag.startsWith('--codex-home='))
      throw new Stop('--codex-home은 새 판에는 없습니다. CODEX_HOME=<홈>을 주고 다시 실행합니다(INSTALL.md)', 1);
    else throw new Stop(`모르는 인자입니다: ${flag}`, 1);
  }
  return options;
}

/** 패키지 안의 바이너리 이름. 규칙은 `crates/fullops/src/cmd/install.rs`의 `platform_binary`와 같다. */
function binaryName(platform = process.platform, arch = process.arch) {
  if (platform === 'win32') return 'fullops.exe';
  if (platform === 'darwin' && arch === 'arm64') return 'fullops-darwin-arm64';
  if (platform === 'darwin' && arch === 'x64') return 'fullops-darwin-x64';
  if (platform === 'linux' && arch === 'x64') return 'fullops-linux-x64';
  return null;
}

/** 고른 바이너리를 `SHA256SUMS`로 검증한다. 전송 손상만 잡고 출처 위조는 막지 못한다(서명 없음, 설계 17절 H3). */
function verifiedBinary(root = PACKAGE) {
  const sumsFile = path.join(root, 'SHA256SUMS');
  if (!fs.existsSync(sumsFile))
    throw new Stop(
      `배포본(dist/native/fullops)이 없습니다: ${root}\n` +
        '받은 판에 아직 릴리스 커밋이 없거나 저장소 체크아웃에서 실행했습니다. 릴리스된 판에서 다시 실행합니다.\n' +
        '개발 중이면 저장소에서 `cargo xtask dist --root <폴더> --target <타깃>`으로 배포본을 만들고 ' +
        '그 안의 bin/fullops install(Windows는 bin\\fullops.exe install)을 직접 실행합니다(docs/build.md).',
      2,
    );
  const name = binaryName();
  if (!name) throw new Stop(`지원하지 않는 OS·CPU입니다: ${process.platform}-${process.arch}`, 2);
  const file = path.join(root, 'bin', name);
  const line = fs.readFileSync(sumsFile, 'utf8').split(/\r?\n/).find(l => l.endsWith(`  bin/${name}`));
  if (!line) throw new Stop(`SHA256SUMS에 bin/${name}이 없습니다`, 2);
  if (!fs.existsSync(file)) throw new Stop(`바이너리가 없습니다: ${file}`, 2);
  const actual = crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  if (actual !== line.slice(0, 64))
    throw new Stop(`bin/${name}의 sha256이 SHA256SUMS와 다릅니다. 받는 중에 손상됐을 수 있습니다. npx 캐시를 지우고 다시 받습니다`, 2);
  // npm 꾸러미는 bin 밖 파일의 실행 비트를 지키지 않을 수 있다
  if (process.platform !== 'win32') {
    try {
      fs.accessSync(file, fs.constants.X_OK);
    } catch {
      fs.chmodSync(file, 0o755);
    }
  }
  return file;
}

/** 셸 없이 인자 배열로 실행하고 종료 코드를 돌려준다. */
function run(file, args) {
  const result = spawnSync(file, args, { stdio: 'inherit', windowsHide: true });
  if (result.error) throw new Stop(`${file}을 실행하지 못했습니다: ${result.error.message}`, 4);
  return result.status ?? 1;
}

/** 설치가 만든 고정 위치 실행본. fullops는 PATH를 바꾸지 않아 안내 명령은 이 전체 경로로 적는다(INSTALL.md). */
function fixedBinary() {
  return path.join(os.homedir(), '.fullops', 'bin', process.platform === 'win32' ? 'fullops.exe' : 'fullops');
}

/** `--repo`: 설치 뒤 활성 레포면 fullops upgrade를 부른다. 아직 켜지 않은 레포와 Python판 레포는 고치지 않고 안내만 한다. */
function repoStep(file, options) {
  const marker = path.join(options.repo, '.fullops-squad');
  if (!fs.existsSync(path.join(marker, 'fullops.toml'))) {
    const python = fs.existsSync(path.join(marker, 'fullops.json'));
    // 설치는 이미 끝났으므로 안내만 하고 성공으로 끝낸다(upgrade는 활성 레포가 아니면 2를 낸다)
    console.log(
      python
        ? `${options.repo}는 아직 Python판(fullops-squad 1.24.0) 레포입니다. 레포는 건드리지 않았습니다.\n` +
            `전환은 미리 보기부터 합니다: "${fixedBinary()}" migrate --from-python --dry-run --repo "${options.repo}"`
        : `${options.repo}는 아직 fullops를 켜지 않은 레포입니다. 레포는 건드리지 않았습니다.\n` +
            `켜려면 그 레포의 Claude Code 세션에서 setup-fullops 스킬을 쓰거나 "${fixedBinary()}" init --repo "${options.repo}"를 실행합니다.`,
    );
    return 0;
  }
  if (options.dryRun) {
    console.log(`설치 뒤 부를 것: fullops upgrade --repo "${options.repo}"`);
    return 0;
  }
  return run(file, ['upgrade', '--repo', options.repo]);
}

function main(argv) {
  const options = parseArgs(argv);
  if (options.command === 'help') {
    console.log(USAGE);
    return 0;
  }
  const file = verifiedBinary();
  const args = ['install'];
  if (options.command === 'check') args.push('--check');
  if (options.hosts) args.push('--hosts', options.hosts);
  if (options.dryRun) args.push('--dry-run');
  let code = run(file, args);
  if (code === 0 && options.repo) code = repoStep(file, options);
  // 첫 패널 릴리스 전에는 카탈로그가 없어 orca가 거절하므로 안내하지 않는다
  if (code === 0 && options.command !== 'check' && fs.existsSync(path.join(__dirname, '..', 'orca-catalog.json')))
    console.log(PANEL);
  return code;
}

module.exports = { Stop, parseArgs, binaryName, main };

if (require.main === module) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    console.error(`fullops: ${error.message}`);
    if (error instanceof Stop && error.code === 1) console.error('사용법: npx --yes github:tourbut/fullops help');
    process.exitCode = error instanceof Stop ? error.code : 1;
  }
}
