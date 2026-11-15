'use strict';
// npx 실행기 시험(node --test). 옛 실행기(fullops-squad.cjs)의 안전장치 가운데 실행기에 남은 것을 단언한다.
const { test, before, after } = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const launcher = require('./fullops.cjs');

const temps = [];
after(() => temps.forEach(dir => fs.rmSync(dir, { recursive: true, force: true })));

/** tarball 꼴의 폴더: `<루트>/npx/fullops.cjs`와 `<루트>/dist/native/fullops/`. `binary`가 바이너리 바이트다. */
function tree({ binary, sum } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'fullops-npx-'));
  temps.push(root);
  fs.mkdirSync(path.join(root, 'npx'));
  fs.copyFileSync(path.join(__dirname, 'fullops.cjs'), path.join(root, 'npx', 'fullops.cjs'));
  const log = path.join(root, 'calls.jsonl');
  // 가짜 바이너리: Windows는 이 node의 사본, 그 밖은 표식을 붙여 이 node를 띄우는 셸 스크립트다
  // (macOS의 node는 옆의 libnode를 @rpath로 불러 다른 폴더에 복사하면 뜨지 않는다).
  // 미리 읽히는 스크립트가 받은 인자를 적고 FAKE_EXIT로 끝낸다(실행기 자신의 node에서는 아무것도 안 한다)
  const preload = path.join(root, 'fake.cjs');
  fs.writeFileSync(
    preload,
    `const p = require('node:path');
if (process.env.FAKE_FULLOPS === '1' || p.basename(process.execPath).startsWith('fullops')) {
  require('node:fs').appendFileSync(${JSON.stringify(log)}, JSON.stringify([p.basename(process.argv[1]), ...process.argv.slice(2)]) + '\\n');
  process.exit(Number(process.env.FAKE_EXIT || 0));
}
`,
  );
  let bin = null;
  if (binary !== undefined) {
    const pkg = path.join(root, 'dist', 'native', 'fullops');
    const name = launcher.binaryName();
    bin = path.join(pkg, 'bin', name);
    fs.mkdirSync(path.dirname(bin), { recursive: true });
    if (binary === 'node' && process.platform === 'win32') fs.copyFileSync(process.execPath, bin);
    else if (binary === 'node')
      fs.writeFileSync(bin, `#!/bin/sh
FAKE_FULLOPS=1 exec ${JSON.stringify(process.execPath)} "$@"
`, { mode: 0o755 });
    else fs.writeFileSync(bin, binary);
    const hash = sum ?? crypto.createHash('sha256').update(fs.readFileSync(bin)).digest('hex');
    fs.writeFileSync(path.join(pkg, 'SHA256SUMS'), `${'0'.repeat(64)}  VERSION\n${hash}  bin/${name}\n`);
    fs.writeFileSync(path.join(pkg, 'VERSION'), '2.0.0\n');
  }
  const run = (args, env = {}) => {
    fs.rmSync(log, { force: true });
    const result = spawnSync(process.execPath, [path.join(root, 'npx', 'fullops.cjs'), ...args], {
      encoding: 'utf8',
      env: { ...process.env, NODE_OPTIONS: `--require "${preload.replace(/\\/g, '/')}"`, ...env },
    });
    const calls = fs.existsSync(log) ? fs.readFileSync(log, 'utf8').trim().split('\n').map(l => JSON.parse(l)) : [];
    return { status: result.status, out: result.stdout + result.stderr, calls };
  };
  return { root, bin, run };
}

let pkg;
before(() => {
  pkg = tree({ binary: 'node' });
  fs.writeFileSync(path.join(pkg.root, 'orca-catalog.json'), '{}');
});

test('인자: claude-code를 claude로 바꾸고 겹친 호스트를 합치며 모르는 것은 거절한다', () => {
  const options = launcher.parseArgs(['update', '--hosts', 'claude-code,grok,claude', '--repo', 'service repo', '--dry-run']);
  assert.equal(options.hosts, 'claude,grok');
  assert.equal(options.repo, path.resolve('service repo'));
  assert.equal(options.dryRun, true);
  assert.equal(launcher.parseArgs(['install', '--hosts', 'codex,all']).hosts, 'all');
  assert.equal(launcher.parseArgs([]).command, 'help');
  for (const bad of [
    ['install', '--hosts', 'unknown'],
    ['install', '--hosts', ','],
    ['update', '--repo'],
    ['install', '--hosts', '--dry-run'],
    ['remove'],
    ['install', '--trust'],
  ])
    assert.throws(() => launcher.parseArgs(bad), e => e instanceof launcher.Stop && e.code === 1, bad.join(' '));
  // 옛 실행기의 인자는 대신할 길을 알린다
  for (const old of [['install', '--codex-home', 'X'], ['install', '--codex-home=X']])
    assert.throws(() => launcher.parseArgs(old), e => e.code === 1 && /CODEX_HOME=/.test(e.message), old.join(' '));
});

test('package.json의 버전이 워크스페이스 버전과 같다(릴리스 1번)', () => {
  const cargo = fs.readFileSync(path.join(__dirname, '..', 'Cargo.toml'), 'utf8');
  const version = cargo.match(/\[workspace\.package\][^[]*?version = "([^"]+)"/)[1];
  assert.equal(require('../package.json').version, version);
});

test('하위 명령을 fullops install의 인자로 바꿔 넘기고 패널 안내를 낸다', () => {
  if (process.platform !== 'win32') fs.chmodSync(pkg.bin, 0o644);
  const install = pkg.run(['install', '--hosts', 'claude-code,grok', '--dry-run']);
  assert.equal(install.status, 0, install.out);
  assert.deepEqual(install.calls, [['install', '--hosts', 'claude,grok', '--dry-run']]);
  assert.match(install.out, /orca plugin install tourbut\.fullops-panel/);
  if (process.platform !== 'win32') assert.ok(fs.statSync(pkg.bin).mode & 0o100, '실행 비트');

  assert.deepEqual(pkg.run(['update']).calls, [['install']]);
  const check = pkg.run(['check', '--hosts', 'all']);
  assert.deepEqual(check.calls, [['install', '--check', '--hosts', 'all']]);
  assert.doesNotMatch(check.out, /orca plugin/);
});

test('바이너리의 종료 코드를 그대로 돌려주고 실패하면 레포 단계로 가지 않는다', () => {
  for (const code of ['2', '4']) {
    const result = pkg.run(['install', '--repo', pkg.root], { FAKE_EXIT: code });
    assert.equal(result.status, Number(code));
    assert.deepEqual(result.calls, [['install']]);
  }
});

test('--repo: 활성 레포에만 셸 없이 경로를 그대로 넘기고, 켜지 않은 레포·Python판 레포·--dry-run에서는 부르지 않는다', () => {
  const repo = path.join(pkg.root, 'svc repo & $(x) `y` %PATH%');
  fs.mkdirSync(path.join(repo, '.fullops-squad'), { recursive: true });
  const fixed = path.join(os.homedir(), '.fullops', 'bin');

  // 켜지 않은 레포: upgrade는 2를 내므로 부르지 않고, 설치가 성공했으니 0과 패널 안내로 끝난다
  const inactive = pkg.run(['update', '--repo', repo], { FAKE_EXIT: '0' });
  assert.equal(inactive.status, 0, inactive.out);
  assert.deepEqual(inactive.calls, [['install']]);
  assert.match(inactive.out, /setup-fullops/);
  const exe = path.join(fixed, process.platform === 'win32' ? 'fullops.exe' : 'fullops');
  assert.ok(inactive.out.includes(`"${exe}" init --repo`), inactive.out);
  assert.match(inactive.out, /orca plugin install/);

  fs.writeFileSync(path.join(repo, '.fullops-squad', 'fullops.json'), '{}');
  const python = pkg.run(['update', '--repo', repo]);
  assert.equal(python.status, 0, python.out);
  assert.deepEqual(python.calls, [['install']]);
  assert.ok(python.out.includes(`${fixed}`), python.out);
  assert.match(python.out, /" migrate --from-python --dry-run/);

  fs.writeFileSync(path.join(repo, '.fullops-squad', 'fullops.toml'), '');
  const update = pkg.run(['update', '--repo', repo]);
  assert.equal(update.status, 0, update.out);
  assert.deepEqual(update.calls, [['install'], ['upgrade', '--repo', repo]]);

  const dry = pkg.run(['update', '--repo', repo, '--dry-run']);
  assert.deepEqual(dry.calls, [['install', '--dry-run']]);
  assert.match(dry.out, /fullops upgrade --repo/);
});

test('패널 안내는 패키지에 orca-catalog.json이 있을 때만 낸다(첫 패널 릴리스 전에는 orca가 거절한다)', () => {
  const bare = tree({ binary: 'node' });
  const result = bare.run(['install']);
  assert.equal(result.status, 0, result.out);
  assert.doesNotMatch(result.out, /orca plugin/);
});

test('sha256이 SHA256SUMS와 다르면 실행하지 않고 거절한다', () => {
  const broken = tree({ binary: 'not a binary', sum: 'a'.repeat(64) });
  const result = broken.run(['install']);
  assert.equal(result.status, 2);
  assert.deepEqual(result.calls, []);
  assert.match(result.out, /SHA256SUMS와 다릅니다/);
});

test('배포본이 없으면(릴리스 커밋 전의 main) 아무것도 실행하지 않고 만드는 길을 알린다', () => {
  const empty = tree();
  const result = empty.run(['install', '--dry-run']);
  assert.equal(result.status, 2);
  assert.deepEqual(result.calls, []);
  assert.match(result.out, /배포본\(dist\/native\/fullops\)이 없습니다/);
  assert.match(result.out, /cargo xtask dist --root/);
});

test('모르는 호스트·명령은 바이너리를 부르지 않고 종료 코드 1이다', () => {
  for (const args of [['install', '--hosts', 'cursor'], ['uninstall']]) {
    const result = pkg.run(args);
    assert.equal(result.status, 1, args.join(' '));
    assert.deepEqual(result.calls, []);
  }
  const help = pkg.run(['help']);
  assert.equal(help.status, 0);
  assert.deepEqual(help.calls, []);
  assert.match(help.out, /npx --yes github:tourbut\/fullops/);
});
