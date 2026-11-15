#!/usr/bin/env python3
"""`py` 구동기(설계 12.1절). 시나리오의 의도 수준 동사를 Fullops-Squad-plugin 1.24.0의 hook 명령과 스크립트 호출로 옮긴다.

개발 시험 도구이고 배포본에 들어가지 않는다. 기준선 사본은 읽고 실행만 한다. 임시 레포·임시 홈·가짜 Orca에서만 돈다.

    python -X utf8 -B scenarios/drivers/py_driver.py <시나리오.toml>
    python -X utf8 -B scenarios/drivers/py_driver.py --bench <횟수>

환경 변수: FULLOPS_PY_BASELINE(1.24.0 사본의 루트), FULLOPS_FAKES(fake-orca·fake-curl·fake-host가 있는 폴더. 보통 target/debug).
FULLOPS_TEST_KEEP이 있으면 임시 폴더를 남긴다.

stdout에 JSON 하나를 낸다. 시나리오는 {"name", "steps": [{"act", "verdict", "flow", "exit", "ms", "output"}], "merged"},
`--bench`는 {"cases": {<이름>: {"p50", "p95", "max", "procs", "verdict", "git", "orca"}}}다(이름과 입력은 xtask/src/bench.rs와 같다).

동사 대응(docs/formats.md 9.4절의 동사 → 1.24.0에서 에이전트가 하던 일):
- hook 동사: 기준선의 hooks/hooks.json에 등록된 명령을 이벤트와 matcher대로 돌린다(SessionStart는 프로세스 셋, Stop은 둘).
  판정은 rs 구동기의 어휘(pass·context·warn·deny·block)이고 여러 프로세스의 출력 가운데 가장 센 것이다.
  편집 동사는 PreToolUse → 파일 반영 → PostToolUse 순서다. 단계의 판정은 PreToolUse의 것이다.
- lead_routes: `jev_route.py --key … --request …`. `class`대로 fake-curl이 Jev의 답을 낸다(direct = simple, 그 밖은 design·product).
- lead_plans: PLANS.md 과제 표에 행을 쓴다(1.24.0에는 명령이 없다. 손 편집이다).
- lead_writes_task: `work.py new`.
- lead_dispatches: (bare가 아니면 분류, 지시서, 준비 커밋과 push, 역할 워크트리 동기화) → `orca orchestration run-create` →
  `orca orchestration worker-start --run … --spec "Task key: …"`. 판정은 worker-start를 실은 PreToolUse의 것이다.
- worker_works: rs 구동기와 같다(세션 시작, preamble, 코드 파일 커밋).
- worker_lints: `lint.py --repo … --from <기본 브랜치>`.
- worker_finishes: 인박스에 완료 보고를 쓰고 `work.py finish`, 완료 기록 커밋.
- worker_reports: 위의 넷 뒤에 worker가 `orca orchestration send --type worker_done`을 보내고 lead가 받는다.
- lead_waits: `orca_wait.py --run …`(가짜 Orca는 기다리지 않으므로 확인 주기를 0.001분으로 준다).
- lead_merges: `integration.py status`로 SHA를 찾고 `review.py need`, 기본 브랜치 병합, push, 역할 워크트리 동기화, PLANS.md 기록.
- runs_fullops: 1.24.0의 스킬이 같은 일에 시키던 스크립트로 옮긴다. `lint` → `lint.py`, `context` → `jev_context.py`(오프라인), `packet build` → `jev_packet.py`,
  `hold` → `integration.py hold`(통합 대기 기록의 메시지를 찾아), 사용자가 답한 뒤의 `review waive` → `review.py waive --decided-by user`.
  `decision add`·`answer`는 1.24.0에서 대화로 묻던 일이라 "n/a"다(답했다는 사실만 기억한다). 그 밖은 1.24.0에 대응이 없다. 판정 "n/a"로 적는다.
  단 `raw`인 `fullops migrate …`는 FULLOPS_FAKES 폴더의 새 판 실행본으로 실제로 돌린다(판정은 그대로 "n/a").
  전환·되돌린 레포에서 1.24.0의 hook이 무엇을 내는지 뒤 단계가 보게 하려는 것이다(N3).
- runs_shell은 `fullops <하위 명령>`도 1.24.0의 hook에 그대로 넣는다(그 판정이 1.24.0이 실제로 낸 것이다).
- 설정: 1.24.0은 `.fullops-squad/fullops.json`을 읽는다. `fullops.toml`을 쓰는 준비 동사(`file`)는 fullops.json의 mode·primary_role·
  primary_branch·test_level을 바꾸고(TOML로 읽히지 않으면 깨진 JSON을 쓴다), 에이전트가 `fullops.toml`을 쓰는 단계는 1.24.0의 복구 명령
  `setup.py`로 옮긴다.
- agy: 1.24.0에는 agy hook이 없다. 준비 동사가 아닌 단계는 모두 "n/a"다.
- 단계의 `df`(그 단계의 차이를 설명하는 12.3절 ID)와 `py`(이 구동기가 내야 하는 판정)는 `xtask compare`가 읽는다. 이 구동기는 보지 않는다.
- 준비 동사(file, git, commits, orca_replies, orca_sends, host_replies, curl_replies, fakes): rs 구동기와 같다.
에이전트가 실행하는 명령은 실제 흐름처럼 셸 도구의 PreToolUse hook과 PostToolUse hook 사이에서 실행한다.
가짜 Orca는 사실대로 답해야 하므로, 배정한 worker의 `worker-list`와 `task-list` 응답은 구동기가 각본 규칙으로 유지한다.
"""
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib

EXE = '.exe' if os.name == 'nt' else ''
# testkit의 Sandbox::command가 지우는 것과 같다. 사용자의 Orca·호스트 설정·API 키가 새어 들지 않게 한다
SCRUB = ('ORCA_TERMINAL_HANDLE', 'ORCA_AGENT_SESSION_ID', 'ORCA_DEV_REPO_ROOT', 'GROK_HOOK_EVENT', 'GROK_HOME', 'CODEX_HOME',
         'CLAUDE_CONFIG_DIR', 'TYPESAFE_API_KEY', 'OPENROUTER_API_KEY', 'FULLOPS_JEV_ENABLED', 'FULLOPS_JEV_CACHE_BYPASS',
         'FULLOPS_JEV_CACHE', 'FULLOPS_ORCA_CLI', 'FULLOPS_TRACE')
LEVEL = {'off': 'off', 'read': 'lite', 'on': 'standard', 'ultra': 'full'}  # 병렬 방침 → 1.24.0의 subagent_level(설계 12.2절 D7)
STRENGTH = ('deny', 'block', 'warn', 'context', 'pass')  # testkit의 verdict_of와 같은 순서
DEFAULTS = {'mode': 'coor', 'roles': {'dev': 'worker'}, 'remote': True, 'primary_role': None, 'routing': 'legacy',
            'tech_plan_role': None, 'test_level': 'lite', 'parallel': 'on', 'host': 'claude'}
PREP = ('file', 'git', 'commits', 'orca_replies', 'orca_sends', 'host_replies', 'curl_replies', 'fakes')
SHELL = {'claude': 'Bash', 'codex': 'Bash', 'grok': 'run_terminal_command'}
SUBAGENT = {'claude': ('Agent', 'prompt'), 'codex': ('spawn_agent', 'message'), 'grok': ('spawn_subagent', 'prompt')}
CONFIG, CONFIG_JSON = '.fullops-squad/fullops.toml', '.fullops-squad/fullops.json'
OPERATING = ('mode', 'primary_role', 'primary_branch', 'test_level')  # 1.24.0의 fullops.json에도 있는 운영 값
PACKET_FLAGS = {'--role': '--role', '--seed': '--seeds', '--update-doc': '--update-docs', '--required': '--required',
                '--decisions': '--decisions'}  # fullops packet build → jev_packet.py


def quote(arg):
    arg = str(arg)
    return arg if re.fullmatch(r'[A-Za-z0-9_./=:-]+', arg) else "'" + arg.replace("'", "'\\''") + "'"


def verdict_of(out):
    specific = out.get('hookSpecificOutput') or {}
    if specific.get('permissionDecision') == 'deny':
        return 'deny'
    if out.get('decision') == 'block':
        return 'block'
    if isinstance(out.get('systemMessage'), str):
        return 'warn'
    return 'context' if isinstance(specific.get('additionalContext'), str) else 'pass'


def flow_of(verdict):
    return 'continues' if verdict in ('pass', 'context', 'warn', 'n/a') else 'stopped'


class Unsupported(Exception):
    """1.24.0에 대응이 없는 단계."""


class Driver:
    def __init__(self, setup):
        unknown = set(setup) - set(DEFAULTS)
        if unknown:
            raise ValueError(f'모르는 setup 키: {sorted(unknown)}')
        self.setup = {**DEFAULTS, **setup}
        self.host = self.setup['host']
        self.plugin = Path(os.environ['FULLOPS_PY_BASELINE']).resolve() / 'plugins/fullops-squad'
        self.fakes = Path(os.environ['FULLOPS_FAKES']).resolve()
        self.root = Path(tempfile.mkdtemp(prefix='fullops-py-')).resolve()
        self.home, self.bin = self.root / 'home', self.root / 'bin'
        for name in ('home', 'orca', 'host', 'curl', 'bin'):
            (self.root / name).mkdir()
        (self.home / '.gitconfig').write_text(
            '[user]\n\tname = fullops-test\n\temail = test@example.invalid\n[init]\n\tdefaultBranch = main\n'
            '[core]\n\tautocrlf = false\n[commit]\n\tgpgsign = false\n[protocol "file"]\n\tallow = always\n', encoding='utf-8')
        self.install_fakes(['curl'])  # 1.24.0의 분류(jev_route.py)는 curl로 Jev를 부른다
        # 등록된 hook 명령: 이벤트 → [(matcher, [스크립트, 인자…])]. 런타임 사본 대신 기준선의 스크립트를 바로 돌린다
        registered = json.loads((self.plugin / 'hooks/hooks.json').read_text(encoding='utf-8'))['hooks']
        self.hooks = {event: [(group.get('matcher'), re.search(r'"\s+([\w -]+)$', hook['command']).group(1).split())
                              for group in groups for hook in group['hooks']] for event, groups in registered.items()}
        self.routed, self.tasked, self.dispatched, self.runs = set(), set(), [], {}
        self.seq = self.tool_seq = 0
        self.ms = 0.0
        self.continuing = set()  # Stop이 막혀 이어 가는 세션. 호스트가 다음 Stop에 stop_hook_active를 싣는다
        self.workers_rule = self.tasks_rule = None
        self.decided = False  # 사용자가 결정에 답했다(1.24.0은 대화로 물었다). 그 뒤의 review waive만 1.24.0의 waive로 옮긴다
        self.build()

    # --- 프로세스 ---

    def dir(self, who):
        return self.root / who

    def env(self, who=None, extra=None):
        env = {k: v for k, v in os.environ.items() if k.upper() not in SCRUB}
        env.update(HOME=str(self.home), USERPROFILE=str(self.home), GIT_CONFIG_GLOBAL=str(self.home / '.gitconfig'),
                   GIT_CONFIG_NOSYSTEM='1', GIT_TERMINAL_PROMPT='0', ORCA_CLI_COMMAND=str(self.fakes / f'fake-orca{EXE}'),
                   FAKE_ORCA_DIR=str(self.root / 'orca'), FAKE_HOST_DIR=str(self.root / 'host'),
                   FAKE_CURL_DIR=str(self.root / 'curl'), FULLOPS_OFFLINE='1',
                   PATH=str(self.bin) + os.pathsep + os.environ.get('PATH', ''))
        terminal = next((d['terminal'] for d in reversed(self.dispatched) if d['role'] == who and not d['sub']), None)
        if terminal:
            env['ORCA_TERMINAL_HANDLE'] = terminal
        env.update(extra or {})
        return env

    def run(self, argv, cwd, env=None, stdin=''):
        started = time.perf_counter()
        done = subprocess.run([str(a) for a in argv], cwd=str(cwd), env=env or self.env(), input=stdin,
                              capture_output=True, text=True, encoding='utf-8', errors='replace')
        self.ms += (time.perf_counter() - started) * 1000
        return done

    def git(self, cwd, *args):
        done = self.run(['git', *args], cwd)
        if done.returncode:
            raise RuntimeError(f'git {args} 실패: {done.stderr}')
        return done.stdout.strip()

    def script(self, name):
        return (self.plugin / 'scripts' / name).as_posix()

    def real(self, argv):
        """에이전트가 치는 명령(python3, orca)을 이 PC에서 실제로 띄울 argv로 바꾼다."""
        if argv[0] == 'python3':
            return [sys.executable, '-X', 'utf8', '-B', *argv[1:]]
        return [self.fakes / f'fake-orca{EXE}', *argv[1:]] if argv[0] == 'orca' else argv

    # --- 임시 레포 ---

    def build(self):
        setup, lead = self.setup, self.dir('lead')
        lead.mkdir()
        self.git(lead, 'init', '-q', '-b', 'main')
        (lead / 'README.md').write_text('# 시험 레포\n', encoding='utf-8')
        self.commit(lead, '처음')
        if setup['remote']:
            self.git(self.root, 'init', '-q', '--bare', '-b', 'main', 'origin.git')
            self.git(lead, 'remote', 'add', 'origin', str(self.root / 'origin.git'))
            self.git(lead, 'push', '-q', '-u', 'origin', 'main')
        roles = dict(setup['roles'])
        self.designer = next((r for r, kind in roles.items() if kind == 'design'), None)
        if not self.designer:  # 1.24.0의 분류는 설계 역할이 있어야 돈다. 워크트리 없는 역할을 하나 등록한다
            self.designer = 'architecture'
            roles[self.designer] = 'design'
        args = ['--repo', lead, '--roles', *roles, '--mode', setup['mode'], '--test-level', setup['test_level'],
                '--subagent-level', LEVEL[setup['parallel']]]
        if setup['primary_role']:
            args += ['--primary-role', setup['primary_role']]
        if not setup['remote']:
            args.append('--local-only')
        done = self.run(self.real(['python3', self.script('setup.py'), *args]), lead)
        if done.returncode:
            raise RuntimeError(f'setup.py 실패: {done.stderr}')
        self.config_base = json.loads((lead / CONFIG_JSON).read_text(encoding='utf-8'))
        marks = [f'- 설계 역할: `{self.designer}`']
        marks += [f'- {label} 역할: `{role}`' for role, kind in roles.items()
                  for label in [{'coordinator': 'coordinator', 'tester': 'tester'}.get(kind)] if label]
        if setup['routing'] == 'split':
            marks += [f'- 제품 기획 역할: `{self.designer}`', f"- 기술 계획 역할: `{setup['tech_plan_role']}`"]
        agents = lead / '.fullops-squad/orca-agents.md'
        text = re.sub(r'^- (?:설계|coordinator|tester) 역할: `[^`]*`\n', '', agents.read_text(encoding='utf-8'), flags=re.M)
        agents.write_text(text.replace('- `dev`:', '\n'.join(marks) + '\n- `dev`:', 1), encoding='utf-8', newline='\n')
        self.commit(lead, 'fullops 설정')
        if setup['remote']:
            self.git(lead, 'push', '-q', 'origin', 'main')
        for role in setup['roles']:
            branch = f'fullops/{role}'
            if setup['remote']:  # setup.py가 원격에 만든 역할 브랜치를 받아 설정 커밋까지 맞춘다
                self.git(lead, 'fetch', '-q', 'origin')
                self.git(lead, 'worktree', 'add', '-q', '-b', branch, str(self.dir(role)), f'origin/{branch}')
                self.git(self.dir(role), 'merge', '-q', '--ff-only', 'main')
                self.git(self.dir(role), 'push', '-q', '-u', 'origin', branch)
            else:
                self.git(lead, 'worktree', 'add', '-q', '-b', branch, str(self.dir(role)))
        # 1.24.0의 분류가 고르는 역할: 설계 역할과 coordinator 역할을 뺀 것(dev 모드의 주 담당은 남는다)
        primary, dev = setup['primary_role'], setup['mode'] == 'dev'
        self.workers = [r for r, kind in roles.items() if (dev and r == primary) or
                        (r != self.designer and kind != 'coordinator' and r != primary)]

    def commit(self, cwd, message):
        self.git(cwd, 'add', '-A')
        self.git(cwd, 'commit', '-q', '--allow-empty', '-m', message)

    def base(self):
        return 'origin/main' if self.setup['remote'] else 'main'

    def merged(self):
        """통합 기준 ref에 들어간 과제 수(worker_works의 커밋 제목 `<KEY> 구현`으로 센다)."""
        where = self.root / 'origin.git' if self.setup['remote'] else self.dir('lead')
        subjects = self.git(where, 'log', '--format=%s', 'main').splitlines()
        return len({s[:-3] for s in subjects if s.endswith(' 구현')})

    # --- 가짜 실행 파일 ---

    def install_fakes(self, names):
        for name in names:
            shutil.copy(self.fakes / f"{'fake-curl' if name == 'curl' else 'fake-host'}{EXE}", self.bin / f'{name}{EXE}')

    def rule(self, kind, rule, at=None):
        """각본에 규칙을 덧붙인다. `at`을 주면 그 번호의 규칙을 바꾼다(번호가 바뀌지 않아 `times` 셈이 유지된다)."""
        path = self.root / kind / 'script.json'
        script = json.loads(path.read_text(encoding='utf-8')) if path.is_file() else {'rules': []}
        if at is None:
            script['rules'].append(rule)
        else:
            script['rules'][at] = rule
        path.write_text(json.dumps(script, ensure_ascii=False), encoding='utf-8')
        return len(script['rules']) - 1 if at is None else at

    def orca_calls(self):
        path = self.root / 'orca/calls.jsonl'
        return [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()] if path.is_file() else []

    def mail(self, run, message):
        with open(self.root / 'orca/mail.jsonl', 'a', encoding='utf-8', newline='\n') as box:
            box.write(json.dumps({'run': run, 'message': message}, ensure_ascii=False) + '\n')
        if message['type'] == 'worker_done':
            for d in self.dispatched:
                d['settled'] = d['settled'] or d['dispatch'] == message['payload'].get('dispatchId')
            self.sync_workers()

    def sync_workers(self):
        """1.24.0의 Stop이 묻는 `worker-list`와, 키 없는 완료 보고의 키를 찾는 `task-list`에 배정한 것을 사실대로 답하게 한다."""
        workers = [{'dispatchId': d['dispatch'], 'taskId': d['task'], 'runId': d['run'], 'terminal': d['terminal'],
                    'state': 'succeeded' if d['settled'] else 'ready', 'stage': 'settled' if d['settled'] else 'input_accepted',
                    'liveness': 'live'} for d in self.dispatched]
        rule = {'when': ['orchestration', 'worker-list'], 'stdout': {'ok': True, 'result': {'workers': workers}}}
        self.workers_rule = self.rule('orca', rule, self.workers_rule)
        tasks = [{'id': d['task'], 'status': 'ready', 'title': d['key'], 'spec': d['spec']} for d in self.dispatched]
        rule = {'when': ['orchestration', 'task-list'], 'stdout': {'ok': True, 'result': {'tasks': tasks}}}
        self.tasks_rule = self.rule('orca', rule, self.tasks_rule)

    def config_json(self, text):
        """`fullops.toml` 글을 1.24.0이 읽는 fullops.json 글로 옮긴다. TOML로 읽히지 않으면 깨진 JSON이다."""
        try:
            new = tomllib.loads(text)
        except tomllib.TOMLDecodeError:
            return '{ "schema_version": = 1\n'
        config = {k: v for k, v in self.config_base.items() if k not in OPERATING}
        config.update({k: new[k] for k in OPERATING if k in new})
        return json.dumps(config, ensure_ascii=False, indent=2) + '\n'

    def route_file(self, key):
        return self.dir('lead') / f'.fullops-squad/docs/evaluations/jev/{key}-route.json'

    # --- hook ---

    def event(self, name, who, session, fields):
        sid = f'sid-{who}' + (f'-{session}' if session else '')
        data = {'hook_event_name': name, 'session_id': sid, 'cwd': str(self.dir(who)), **fields}
        if self.host == 'grok':  # grok은 camelCase와 snake_case 별칭을 함께 준다
            data.update({re.sub(r'_(\w)', lambda m: m.group(1).upper(), k): v for k, v in list(data.items())})
            data['hookEventName'] = re.sub(r'(?<!^)(?=[A-Z])', '_', name).lower()
        return sid, data

    def hook(self, name, who, session=None, subject='', env=None, **fields):
        """등록된 hook 명령을 hooks.json의 순서와 matcher대로 돌린다. (판정, 출력 글)을 돌려준다."""
        if self.host == 'agy':
            raise Unsupported('1.24.0은 agy에서 hook이 돌지 않는다')
        sid, data = self.event(name, who, session, fields)
        verdict, text = 'pass', ''
        for matcher, words in self.hooks.get(name, []):
            if matcher and not re.fullmatch(matcher, subject):
                continue
            done = self.run(self.real(['python3', self.script(words[0] + '.py'), *words[1:]]), self.dir(who),
                            self.env(who, env), json.dumps(data, ensure_ascii=False))
            try:
                out = json.loads(done.stdout)
                found = verdict_of(out) if done.returncode == 0 and isinstance(out, dict) else 'error'
            except ValueError:
                found = 'error'
            text += done.stdout + done.stderr
            if found == 'error':
                return 'error', text
            verdict = min(verdict, found, key=STRENGTH.index)
        if name == 'Stop':
            (self.continuing.add if verdict == 'block' else self.continuing.discard)(sid)
        return verdict, text

    def tool_hook(self, who, session, tool_name, tool_input, apply=None, env=None):
        """도구 호출 하나: PreToolUse, 막히지 않으면 `apply`, PostToolUse. (PreToolUse 판정, 출력 글, `apply`의 값)."""
        self.tool_seq += 1
        fields = {'tool_name': tool_name, 'tool_input': tool_input, 'tool_use_id': f'toolu_{self.tool_seq}'}
        verdict, text = self.hook('PreToolUse', who, session, tool_name, env, **fields)
        if flow_of(verdict) == 'stopped' or apply is None:
            return verdict, text, None
        value = apply()
        response = {'stdout': value.stdout, 'exit_code': value.returncode} if hasattr(value, 'returncode') else {}
        _, after = self.hook('PostToolUse', who, session, tool_name, env, tool_response=response, **fields)
        return verdict, text + after, value

    def shell(self, who, *commands, session=None, env=None):
        """에이전트의 셸 도구 호출 하나. `commands`는 `&&`로 이은 argv들이고 앞의 것이 실패하면 멈춘다."""
        if self.host not in SHELL:
            raise Unsupported(f'{self.host}의 셸 도구')
        text = ' && '.join(' '.join(quote(a) for a in argv) for argv in commands)

        def apply():
            for argv in commands:
                done = self.run(self.real(list(argv)), self.dir(who), self.env(who, env))
                if done.returncode:
                    break
            return done
        return self.tool_hook(who, session, SHELL[self.host], {'command': text}, apply, env)

    def cli(self, act, who, *commands, env=None, fail='deny'):
        """셸 도구로 명령을 실행한 단계의 결과. hook이 막으면 그 판정, 아니면 종료 코드로 정한다."""
        verdict, text, done = self.shell(who, *commands, env=env)
        if done is None:
            return result(act, verdict, text), None
        return result(act, 'pass' if done.returncode == 0 else fail, text + done.stdout + done.stderr, done.returncode), done

    def write_tool(self, who, session, path, content, env=None, old=None):
        """쓰기(old가 있으면 편집, content가 None이면 삭제) 도구의 hook을 부르고 막히지 않으면 파일에 반영한다."""
        file = self.dir(who) / path
        before = file.read_text(encoding='utf-8') if file.is_file() else ''
        after = None if content is None else before.replace(old, content, 1) if old is not None else content
        if self.host == 'codex':
            body = (f'*** Delete File: {path}\n' if content is None else
                    f'*** Update File: {path}\n@@\n' + ''.join(f'-{line}\n' for line in old.splitlines())
                    + ''.join(f'+{line}\n' for line in content.splitlines()) if old is not None else
                    f'*** Add File: {path}\n' + ''.join(f'+{line}\n' for line in content.splitlines()))
            name, tool = 'apply_patch', {'command': f'*** Begin Patch\n{body}*** End Patch\n'}
        elif old is not None:
            name = {'claude': 'Edit', 'grok': 'search_replace'}[self.host]
            tool = {'file_path': str(file), 'old_string': old, 'new_string': content}
        else:
            name = {'claude': 'Write', 'grok': 'write'}[self.host]
            tool = {'file_path': str(file), 'content': content or ''}

        def apply():
            if after is None:
                file.unlink(missing_ok=True)
            else:
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_text(after, encoding='utf-8', newline='\n')
        return self.tool_hook(who, session, name, tool, apply, env)[:2]

    def prompt(self, who, session, text, env=None):
        self.continuing.discard(f'sid-{who}' + (f'-{session}' if session else ''))
        return self.hook('UserPromptSubmit', who, session, env=env, prompt=text)

    def preamble(self, d):
        # testkit의 orca_shapes::preamble과 같은 글이다
        return ('You are working inside Orca, a multi-agent IDE. You are a dispatched worker.\n'
                f"Your task ID is: {d['task']}\n\n=== CLI COMMANDS ===\n\n  orca orchestration send --from term_worker "
                '--type worker_done --subject "<short status>" --body "<summary>" '
                f"--task-id {d['task']} --dispatch-id {d['dispatch']} --outcome succeeded\n\n=== TASK ===\n{d['spec']}\n")

    # --- 흐름 동사 ---

    def lead_routes(self, act, key, role, kind):
        # Jev의 답. 폴백(unresolved)은 답을 걸지 않는다. 결과 파일이 이미 있으면 jev_route.py가 Jev를 부르지 않으므로
        # 답을 걸지 않는다(쓰이지 않은 답이 남아 다음 분류가 먹으면 안 된다)
        if kind != 'unresolved' and not self.route_file(key).is_file():
            split = self.setup['routing'] == 'split'
            scope = {'direct': 'implementation' if split else 'simple', 'product': 'product'}.get(kind, 'design')
            labels = ('implementation', 'product') if split else ('simple', 'design')
            if scope not in labels:
                raise Unsupported(f'routing {self.setup["routing"]}에는 class {kind}가 없다')
            chosen = role if role in self.workers else self.workers[0]
            choice = lambda pick, names: {'type': 'choice', 'choice': pick, 'confidence': 0.95,
                                          'probabilities': {n: 0.95 if n == pick else 0.05 / max(1, len(names) - 1) for n in names}}
            answers = {'scope': choice(scope, labels), 'role': choice(chosen, self.workers)}
            if len(self.workers) == 1:
                answers['role']['probabilities'] = {chosen: 1.0}
            self.rule('curl', {'when': ['--data-binary'], 'times': 1, 'stdout': json.dumps({
                'model': 'jev-fake', 'usage': {'input_tokens': 1, 'output_tokens': 1, 'cost': 0}, 'answers': answers})})
        command = ['python3', self.script('jev_route.py'), '--repo', self.dir('lead').as_posix(), '--key', key,
                   '--request', f'{key} 요청', '--agent', 'claude', '--model', 'test-model', '--effort', 'medium']
        online = {'FULLOPS_OFFLINE': '0', 'TYPESAFE_API_KEY': 'fake-key'}  # 가짜 키다. 요청은 fake-curl이 받는다
        step, _ = self.cli(act, 'lead', command, env=online)
        if step['flow'] == 'continues':
            self.routed.add(key)
        return step

    def lead_plans(self, act, key, role, goal, source, status, after):
        status = status or '대기'
        if after:
            status += f', 선행: {after}'
        plans = '.fullops-squad/PLANS.md'
        lines = (self.dir('lead') / plans).read_text(encoding='utf-8').split('\n')
        last = max(i for i, line in enumerate(lines) if line.startswith('|'))
        lines.insert(last + 1, f'| {key} | {goal or key + " 구현"} | {role} | {status} | {source or "시나리오 지시"} |')
        return result(act, *self.write_tool('lead', None, plans, '\n'.join(lines)))

    def lead_writes_task(self, act, key, role, goal):
        step, _ = self.cli(act, 'lead', ['python3', self.script('work.py'), 'new', '--repo', self.dir('lead').as_posix(),
                                         '--role', role, '--key', key, '--goal', goal or f'{key} 구현'])
        if step['flow'] == 'continues':
            self.tasked.add(f'{key}/{role}')
        return step

    def dispatch(self, act, who, key, role, spec, options, run=None, chat=False):
        """`who`가 worker-start로 배정한다. fake-orca가 실제로 낸 ID를 기억한다(번호는 rs 구동기와 같게 센다)."""
        self.seq += 1
        n = self.seq
        known = run or self.runs.get(who)
        if not known:
            self.rule('orca', {'when': ['orchestration', 'run-create'], 'times': 1,
                               'stdout': {'ok': True, 'result': {'runId': f'run_{n}', 'objective': 'fullops'}}})
            step, done = self.cli(act, who, ['orca', 'orchestration', 'run-create', '--objective', 'fullops', '--json'],
                                  fail='external')
            if step['verdict'] != 'pass':
                return step
            known = self.runs[who] = json.loads(done.stdout)['result']['runId']
        started = {'dispatchId': f'ctx_{n}', 'taskId': f'task_{n}', 'runId': known, 'state': 'ready', 'agent': 'claude'}
        if not chat:
            started['terminal'] = f'term_{n}'
        self.rule('orca', {'when': ['orchestration', 'worker-start'], 'times': 1, 'stdout': {'ok': True, 'result': started}})
        before = len(self.orca_calls())
        command = ['orca', 'orchestration', 'worker-start', '--run', known, '--spec', spec,
                   '--worktree', self.dir(role).as_posix(), *options, '--json']
        step, done = self.cli(act, who, command, fail='external')
        reply = next((c for c in self.orca_calls()[before:] if 'worker-start' in c['argv']), None)
        if reply and step['verdict'] == 'pass':
            got = reply['stdout']['result']
            self.dispatched.append({'key': key, 'role': role, 'run': got['runId'], 'dispatch': got['dispatchId'],
                                    'task': got['taskId'], 'terminal': got.get('terminal'), 'spec': spec,
                                    'started': False, 'sub': who != 'lead', 'settled': False})
            self.sync_workers()
        return step

    def lead_dispatches(self, step, key, role):
        act, purpose = step['act'], step.get('purpose', 'build')
        lead, work = self.dir('lead'), purpose in ('build', 'fix')
        if purpose == 'fix':
            reopened = self.reopen(act, key, role)
            if reopened and reopened['flow'] == 'stopped':
                return reopened
        if not step.get('bare'):
            # 1.24.0의 lead는 이미 분류한 키(결과 파일이 있다)를 다시 분류하지 않는다
            if key not in self.routed and not self.route_file(key).is_file():
                routed = self.lead_routes(act, key, role, 'direct')
                if routed['flow'] == 'stopped':
                    return routed
            if work and f'{key}/{role}' not in self.tasked:
                tasked = self.lead_writes_task(act, key, role, None)
                if tasked['flow'] == 'stopped':
                    return tasked
            # 준비 커밋과 동기화: 1.24.0에서는 lead가 손으로 했다(fullops-orca의 dispatch 절)
            ready = [['git', 'add', '-A'], ['git', 'commit', '-q', '--allow-empty', '-m', f'docs: {key} 배정 준비']]
            if self.setup['remote']:
                ready.append(['git', 'push', '-q', 'origin', 'main'])
            prepared, _ = self.cli(act, 'lead', *ready, fail='external')
            if prepared['flow'] == 'stopped':
                return prepared
            # 동기화가 안 되면(갈라졌거나 더럽다) 그대로 배정을 시도한다. 판정은 worker-start의 hook이 한다
            self.shell('lead', ['git', '-C', self.dir(role).as_posix(), 'merge', '-q', '--no-edit', 'main'])
        declared = {'review': f"review; Review SHA: {step.get('sha', '')}; Implementer session: sid-{role}; "
                              'Reviewer session: sid-reviewer', 'research': 'research; read only'}.get(purpose, 'implementation')
        spec = f'Task key: {key}; Purpose: {declared}'
        if work:
            spec += f'; 지시서 .fullops-squad/handovers/to_{role}.md'
        options = [word for name in ('terminal', 'agent', 'model', 'effort') if step.get(name)
                   for word in (f'--{name}', step[name])]
        return self.dispatch(act, 'lead', key, role, spec, options, step.get('run'), bool(step.get('chat')))

    def reopen(self, act, key, role):
        """완료한 과제의 수정 배정: 1.24.0은 `work.py reopen`으로 새 시도를 연다. 병합 전에는 완료 기록이 역할 브랜치에만
        있으므로 비워진 인박스와 완료 기록이 있는 체크아웃(역할 워크트리, 없으면 lead)에서 연다. 열 것이 없으면 None."""
        inbox = '.fullops-squad/handovers/to_' + role + '.md'
        logs = lambda where: (where / '.fullops-squad/handovers/logs').glob(f'*_to_{role}.md')
        where = next((w for w in (self.dir(role), self.dir('lead')) if (w / inbox).is_file() and not (w / inbox).read_bytes()
                      and any(f'## {key} — ' in p.read_text(encoding='utf-8') for p in logs(w))), None)
        if not where:
            return None
        step, _ = self.cli(act, 'lead', ['python3', self.script('work.py'), 'reopen', '--repo', where.as_posix(), '--role', role,
                                         '--key', key, '--goal', f'{key} 수정'],
                           ['git', '-C', where.as_posix(), 'add', '-A'],
                           ['git', '-C', where.as_posix(), 'commit', '-q', '-m', f'docs: {key} 수정 지시'])
        return step

    def start_worker(self, role):
        """배정받은 worker 세션이 처음 일을 시작한다: SessionStart와 preamble이 든 첫 프롬프트."""
        fresh = next((d for d in reversed(self.dispatched) if d['role'] == role and not d['sub'] and not d['started']), None)
        if fresh:
            fresh['started'] = True
            self.hook('SessionStart', role, subject='startup', source='startup')
            self.prompt(role, None, self.preamble(fresh))

    def worker_works(self, role, key, path, lines):
        self.start_worker(role)
        file = self.dir(role) / (path or f'src/{key.lower()}.py')
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(f'"""{key} 구현."""\n' + ''.join(f'print({i})\n' for i in range(lines)), encoding='utf-8', newline='\n')
        self.commit(self.dir(role), f'{key} 구현')
        return result('worker_works', 'pass')

    def worker_lints(self, act, role, env=None):
        return self.cli(act, role, ['python3', self.script('lint.py'), '--repo', self.dir(role).as_posix(),
                                    '--from', self.base()], env=env)[0]

    def current(self, role):
        return next((d for d in reversed(self.dispatched) if d['role'] == role and not d['sub']), None)

    def worker_finishes(self, act, role, followup=None, env=None):
        inbox = f'.fullops-squad/handovers/to_{role}.md'
        text = (self.dir(role) / inbox).read_text(encoding='utf-8') if (self.dir(role) / inbox).is_file() else ''
        # 1.24.0의 `work.py finish`는 --key가 있어야 한다. 배정이 막혔으면(예: DF-16) 인박스 제목 `# <KEY> — `의 키로 끝낸다
        d = self.current(role)
        title = re.search(r'^# (\S+) — ', text, re.M)
        key = d['key'] if d else title and title.group(1)
        if not key:
            raise Unsupported(f'{role}에 배정과 인박스 과제가 없다(1.24.0의 work.py finish는 --key와 인박스 과제가 있어야 한다)')
        head = self.git(self.dir(role), 'rev-parse', 'HEAD')
        report = (f"브랜치 fullops/{role} / SHA {head} / 변경 이유: {key} 구현 / 검증: lint 통과 / "
                  f"남은 일: {followup or '없음'}\n")
        verdict, out = self.write_tool(role, None, inbox, text.partition('## 완료 보고\n')[0] + '## 완료 보고\n\n' + report, env)
        if flow_of(verdict) == 'stopped':
            return result(act, verdict, out)
        return self.cli(act, role, ['python3', self.script('work.py'), 'finish', '--repo', self.dir(role).as_posix(),
                                    '--role', role, '--key', key],
                        ['git', 'add', '-A'], ['git', 'commit', '-q', '-m', f"docs({role}): {key} 완료 기록"], env=env)[0]

    def worker_reports(self, step, role, key):
        act = step['act']
        if not any(d['key'] == key and d['role'] == role and not d['sub'] for d in self.dispatched):
            dispatched = self.lead_dispatches(step, key, role)
            if dispatched['flow'] == 'stopped':
                return dispatched
        self.worker_works(role, key, None, 3)
        for done in (self.worker_lints(act, role), self.worker_finishes(act, role)):
            if done['flow'] == 'stopped':
                return done
        self.seq += 1
        d, message = self.current(role), f'msg_{self.seq}'
        body = (f"[완료] {key} | 브랜치 fullops/{role} | SHA {self.git(self.dir(role), 'rev-parse', 'HEAD')} | "
                f'변경: {key} 구현 | 검증(lint 포함): 통과')
        self.rule('orca', {'when': ['orchestration', 'send'], 'times': 1,
                           'stdout': {'ok': True, 'result': {'message': {'id': message}}}})
        sent, _ = self.cli(act, role, ['orca', 'orchestration', 'send', '--type', 'worker_done', '--subject', f'{key} 완료',
                                       '--body', body, '--task-id', d['task'], '--dispatch-id', d['dispatch'],
                                       '--outcome', 'succeeded', '--json'], fail='external')
        if sent['flow'] == 'stopped':
            return sent
        self.mail(d['run'], {'id': message, 'type': 'worker_done', 'from': d['dispatch'], 'subject': f'{key} 완료', 'body': body,
                             'payload': {'taskId': d['task'], 'dispatchId': d['dispatch'], 'outcome': 'succeeded'}})
        return self.lead_waits(act, None, [])

    def lead_waits(self, act, run, acks):
        command = ['python3', self.script('orca_wait.py'), '--orca', (self.fakes / f'fake-orca{EXE}').as_posix(),
                   '--repo', self.dir('lead').as_posix(), '--wait-ms', '1000', '--max-minutes', '0.001']
        run = run or self.runs.get('lead')
        if run:
            command += ['--run', run]
        for ack in acks[:1]:  # 1.24.0의 --ack는 전달 ID 하나다
            command += ['--ack', ack]
        return self.cli(act, 'lead', command, fail='external')[0]

    def lead_merges(self, act, key):
        lead, remote = self.dir('lead'), self.setup['remote']
        repo = lead.as_posix()
        step, done = self.cli(act, 'lead', ['python3', self.script('integration.py'), '--repo', repo, 'status'])
        if step['verdict'] != 'pass':
            return step
        item = next((i for i in json.loads(done.stdout)['pending'] if i.get('key') == key and i.get('sha')), None)
        if not item:
            return result(act, 'deny', f'{key}의 통합 대기 완료 보고가 없다', 2)
        sha = self.git(lead, 'rev-parse', item['sha'])
        step, done = self.cli(act, 'lead', ['python3', self.script('review.py'), 'need', '--repo', repo,
                                            '--from', self.base(), '--to', sha])
        if step['verdict'] != 'pass':
            return step
        waiver = lead / f'.fullops-squad/docs/evaluations/qa-reports/{key}-review/waiver.json'
        # 1.24.0은 여기서 사용자에게 진행·생략을 묻는다. 생략으로 답해 `review.py waive`를 남겼으면 병합한다
        if json.loads(done.stdout)['required'] and not waiver.is_file():
            return result(act, 'human', done.stdout, 3)
        before = self.git(lead, 'rev-parse', 'HEAD')
        update = [['git', 'fetch', '-q', 'origin'], ['git', 'merge', '-q', '--ff-only', 'origin/main']] if remote else []
        step, done = self.cli(act, 'lead', *update, ['git', 'merge', '-q', '--no-edit', sha], fail='human')
        if step['verdict'] != 'pass':
            self.run(['git', 'merge', '--abort'], lead)
            return step
        if self.git(lead, 'rev-parse', 'HEAD') != sha:  # fast-forward가 아니면 병합 결과에서 lint를 한 번 돌린다
            step, _ = self.cli(act, 'lead', ['python3', self.script('lint.py'), '--repo', repo, '--from', before])
            if step['verdict'] != 'pass':
                self.git(lead, 'reset', '-q', '--keep', before)
                return step
        if remote:
            step, _ = self.cli(act, 'lead', ['git', 'push', '-q', 'origin', 'main'], fail='external')
            if step['verdict'] != 'pass':
                return step
        plans = '.fullops-squad/PLANS.md'
        text = (lead / plans).read_text(encoding='utf-8')
        row = re.search(rf'^\| {re.escape(key)} \|.*$', text, re.M)
        if row:  # 병합 기록: 상태 칸을 고친다
            cells = row.group(0).split(' | ')
            cells[3] = f'완료(병합 {sha[:12]})'
            self.write_tool('lead', None, plans, text.replace(row.group(0), ' | '.join(cells), 1))
        record = [['git', 'add', '-A'], ['git', 'commit', '-q', '--allow-empty', '-m', f'docs: {key} 병합 기록']]
        step, _ = self.cli(act, 'lead', *record, *([['git', 'push', '-q', 'origin', 'main']] if remote else []), fail='external')
        clean = [r for r in self.setup['roles'] if not self.git(self.dir(r), 'status', '--porcelain')]
        if clean:  # 쉬는 역할 워크트리에 기본 브랜치를 반영한다. 실패는 다음 배정의 동기화가 다시 본다
            self.shell('lead', *[['git', '-C', self.dir(r).as_posix(), 'merge', '-q', '--ff-only', 'main'] for r in clean])
        return step

    # --- 단계 ---

    def runs_fullops(self, act, who, args, env):
        """`fullops` 명령을 1.24.0의 스킬이 같은 일에 시키던 스크립트로 옮긴다. 대응이 없으면 Unsupported."""
        repo = self.dir(who).as_posix()
        status = lambda: json.loads(self.run(self.real(['python3', self.script('integration.py'), '--repo', repo, 'status']),
                                             self.dir(who)).stdout or '{}').get('pending', [])
        option = lambda name: args[args.index(name) + 1] if name in args else None
        if args[:1] == ['lint']:
            level = ['--level', option('--level')] if '--level' in args else []
            return self.cli(act, who, ['python3', self.script('lint.py'), '--repo', repo, '--from', option('--from') or self.base(),
                                       *level], env=env)[0]
        if args[:2] == ['packet', 'build'] and len(args) > 2:
            grouped, rest = {}, []
            words = iter(args[3:])
            for word in words:
                if word in PACKET_FLAGS:
                    grouped.setdefault(PACKET_FLAGS[word], []).append(next(words))
                else:
                    rest.append(word)
            converted = [w for flag, values in grouped.items() for w in (flag, *values)]
            return self.cli(act, who, ['python3', self.script('jev_packet.py'), '--repo', repo, '--key', args[2],
                                       *converted, *rest], env=env)[0]
        if args[:1] == ['context'] and len(args) > 1:  # 인자 모양이 같다(--role, --paths, --required, --force)
            return self.cli(act, who, ['python3', self.script('jev_context.py'), '--repo', repo, '--key', args[1], *args[2:]],
                            env=env)[0]
        if args[:1] == ['hold'] and len(args) > 1:
            item = next((i for i in status() if i.get('key') == args[1]), None)
            if not item:
                return result(act, 'deny', f'{args[1]}의 통합 대기 기록이 없다', 2)
            return self.cli(act, who, ['python3', self.script('integration.py'), '--repo', repo, 'hold', '--message',
                                       item['message'], '--reason', option('--reason'), '--owner', option('--owner'),
                                       '--resume', option('--resume')], env=env)[0]
        if args[:2] == ['decision', 'answer']:
            self.decided = True
        if args[:2] == ['review', 'waive'] and len(args) > 2 and self.decided:
            item = next((i for i in status() if i.get('key') == args[2] and i.get('sha')), None)
            head = item['sha'] if item else self.git(self.dir(who), 'rev-parse', 'HEAD')
            base = self.git(self.dir(who), 'merge-base', head, self.base())
            return self.cli(act, who, ['python3', self.script('review.py'), 'waive', '--repo', repo, '--key', args[2],
                                       '--from', base, '--to', head, '--reason', option('--reason'),
                                       '--decided-by', 'user'], env=env)[0]
        raise Unsupported('1.24.0에는 `fullops` 명령이 없다')

    def step(self, step):
        act, who, session, env = step['act'], step.get('who', 'lead'), step.get('session'), step.get('env')
        if self.host == 'agy' and act not in PREP:
            raise Unsupported('1.24.0에는 agy hook이 없다')

        def req(name):
            if name not in step:
                raise ValueError(f'동사 {act}에 `{name}` 인자가 필요합니다')
            return step[name]
        fixture = lambda: result(act, 'pass')
        script = lambda kind: (self.rule(kind, {k: v for k, v in step.items() if k not in ('act', 'expect', 'pending')}), fixture())[1]
        if act == 'session_starts':
            source = step.get('source', 'startup')
            self.continuing.discard(f'sid-{who}' + (f'-{session}' if session else ''))
            kind = {'startup': 'new', 'resume': 'load'}.get(source, source) if self.host == 'grok' else source
            return result(act, *self.hook('SessionStart', who, session, kind, env, source=kind))
        if act == 'user_prompts':
            return result(act, *self.prompt(who, session, req('text'), env))
        if act == 'preamble_arrives':
            last = next((d for d in reversed(self.dispatched) if d['role'] == who), None)
            dispatch = step.get('dispatch') or (last or {}).get('dispatch') or req('dispatch')
            task = step.get('task') or (last or {}).get('task') or 'task_0'
            return result(act, *self.prompt(who, session, self.preamble(
                {'dispatch': dispatch, 'task': task, 'spec': (last or {}).get('spec', '')}), env))
        if act == 'runs_shell':
            name = 'PowerShell' if step.get('dialect') == 'powershell' and self.host == 'claude' else SHELL.get(self.host)
            if not name:
                raise Unsupported(f'{self.host}의 셸 도구')
            command = req('command')
            # 소비형 check는 실제로 실행해야 메일이 읽힌다(1.24.0의 Stop이 Orca의 미처리 메일을 본다). 다른 명령은 hook만 본다
            if command.startswith('orca orchestration check') and name != 'PowerShell':
                return result(act, *self.shell(who, shlex.split(command), session=session, env=env)[:2])
            return result(act, *self.tool_hook(who, session, name, {'command': command}, env=env)[:2])
        if act == 'writes_file' and req('path') == CONFIG:
            # 1.24.0에는 에이전트가 고칠 설정 편집 통로가 없다. 오류 문구가 시키는 복구 명령 setup.py로 옮긴다
            try:
                new = tomllib.loads(req('content'))
            except tomllib.TOMLDecodeError:
                new = {}
            flags = [w for k in ('mode', 'primary_role', 'test_level') if new.get(k)
                     for w in ('--' + k.replace('_', '-'), new[k])]
            return self.cli(act, who, ['python3', self.script('setup.py'), '--repo', self.dir(who).as_posix(),
                                       *(flags or ['--mode', 'coor'])], env=env)[0]
        if act in ('writes_file', 'edits_file', 'deletes_file'):
            content = None if act == 'deletes_file' else req('new' if act == 'edits_file' else 'content')
            return result(act, *self.write_tool(who, session, req('path'), content, env, step.get('old') if act == 'edits_file' else None))
        if act == 'spawns_subagent':
            name, field = SUBAGENT[self.host]
            return result(act, *self.tool_hook(who, session, name, {field: req('prompt')}, env=env)[:2])
        if act == 'runs_workflow':  # 1.24.0의 matcher에 Workflow가 없어 hook이 돌지 않는다
            return fixture()
        if act == 'stops':
            fields = {'stop_hook_active': f'sid-{who}' + (f'-{session}' if session else '') in self.continuing}
            if self.host == 'claude':
                fields['background_tasks'] = []
            if self.host == 'grok' or step.get('reason'):
                fields['reason'] = step.get('reason', 'end_turn')
            return result(act, *self.hook('Stop', who, session, env=env, **fields))
        if act == 'turn_fails':
            if self.host == 'codex':
                return fixture()
            return result(act, *self.hook('StopFailure', who, session, env=env, error='rate_limit',
                                          error_details=step.get('error', 'API Error: usage limit reached')))
        if act == 'file':
            path, content = req('path'), req('content')
            if path == CONFIG:  # 1.24.0이 읽는 설정은 fullops.json이다
                path, content = CONFIG_JSON, self.config_json(content)
            path = self.dir(who) / path
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding='utf-8', newline='\n')
            return fixture()
        if act == 'git':
            self.git(self.dir(who), *req('args'))
            return fixture()
        if act == 'commits':
            self.commit(self.dir(who), step.get('message', '시나리오 커밋'))
            return fixture()
        if act in ('orca_replies', 'host_replies', 'curl_replies'):
            return script(act.split('_')[0])
        if act == 'orca_sends':
            self.seq += 1
            last = self.dispatched[-1] if self.dispatched else {}
            kind = step.get('type', 'worker_done')
            payload = {'taskId': step.get('task', last.get('task', 'task_0')),
                       'dispatchId': step.get('dispatch', last.get('dispatch', 'ctx_0'))}
            outcome = step.get('outcome') or ('succeeded' if kind == 'worker_done' else None)
            if outcome:
                payload['outcome'] = outcome
            self.mail(step.get('run') or self.runs.get('lead') or req('run'),
                      {'id': step.get('id', f'msg_{self.seq}'), 'type': kind, 'from': payload['dispatchId'],
                       'subject': step.get('subject', '시나리오 메시지'), 'body': step.get('body', ''), 'payload': payload})
            return fixture()
        if act == 'fakes':
            self.install_fakes(req('names'))
            return fixture()
        if act == 'lead_routes':
            return self.lead_routes(act, req('key'), step.get('role'), step.get('class', 'direct'))
        if act == 'lead_plans':
            return self.lead_plans(act, req('key'), req('role'), step.get('goal'), step.get('source'), step.get('status'), step.get('after'))
        if act == 'lead_writes_task':
            return self.lead_writes_task(act, req('key'), req('role'), step.get('goal'))
        if act == 'lead_dispatches':
            return self.lead_dispatches(step, req('key'), req('role'))
        if act == 'worker_subdispatches':
            role, key = req('role'), req('key')
            self.start_worker(role)  # 하위 배정은 preamble을 받은 worker 세션이 한다
            options = [word for name in ('agent', 'model', 'effort') if step.get(name) for word in (f'--{name}', step[name])]
            return self.dispatch(act, role, key, role, f"Task key: {key}; Purpose: implementation; {req('brief')}", options)
        if act == 'worker_works':
            return self.worker_works(req('role'), req('key'), step.get('path'), step.get('lines', 3))
        if act == 'worker_lints':
            return self.worker_lints(act, req('role'), env)
        if act == 'worker_finishes':
            return self.worker_finishes(act, req('role'), step.get('followup'), env)
        if act == 'worker_reports':
            return self.worker_reports(step, req('role'), req('key'))
        if act == 'lead_waits':
            return self.lead_waits(act, step.get('run'), step.get('ack', []))
        if act == 'lead_merges':
            return self.lead_merges(act, req('key'))
        if act == 'runs_fullops':
            # migrate는 레포의 판을 바꾸는 일이다. 새 판 실행본으로 실제로 돌려, 뒤 단계가 전환·되돌린 레포에서 1.24.0의 판정을 받게 한다(N3)
            if step.get('raw') and req('args')[:1] == ['migrate']:
                self.run([self.fakes / f'fullops{EXE}', *step['args']], self.dir(who), self.env(who, env))
                raise Unsupported('1.24.0에는 `fullops` 명령이 없다')
            return self.runs_fullops(act, who, req('args'), env)
        raise ValueError(f'모르는 동사입니다: {act}')

    # --- 성능 측정(설계 12.4절) ---

    def bench(self, runs):
        """xtask bench와 같은 대표 입력에 등록된 hook 명령을 돌려 이벤트 하나의 시간(프로세스들의 합)을 잰다."""
        lead, dev = self.dir('lead'), self.dir('dev')
        plain = self.root / 'plain'
        plain.mkdir()
        self.git(plain, 'init', '-q', '-b', 'main')
        self.commit(plain, '처음')
        self.hook('SessionStart', 'lead', subject='startup', source='startup')
        self.hook('SessionStart', 'dev', subject='startup', source='startup')
        inbox = str(lead / '.fullops-squad/handovers/to_dev.md')
        edit = lambda path: {'tool_name': 'Edit', 'tool_input': {'file_path': path, 'old_string': 'a', 'new_string': 'b'}}
        bash = lambda command: {'tool_name': 'Bash', 'tool_input': {'command': command}}
        stop = ('Stop', 'lead', {'stop_hook_active': False, 'background_tasks': []})

        def pending():  # 통합 대기 10건: 병합하지 않은 커밋 열 개의 완료 보고를 lead가 받는다
            for n in range(10):
                self.commit(dev, f'B-{n} 구현')
                sha = self.git(dev, 'rev-parse', 'HEAD')
                self.mail('run_bench', {'id': f'msg_bench_{n}', 'type': 'worker_done', 'from': 'ctx_bench', 'subject': '완료',
                                        'body': f'[완료] B-{n} | 브랜치 fullops/dev | SHA {sha}', 'payload': {}})
            self.lead_waits('bench', 'run_bench', [])

        def settle():  # 통합 대기를 치운다(보고된 커밋을 기본 브랜치에 넣는다)
            self.git(lead, 'merge', '-q', '--ff-only', 'fullops/dev')
            self.git(lead, 'push', '-q', 'origin', 'main')

        def ready():
            self.lead_plans('bench', 'R-1', 'dev', None, None, None, None)

        def held():
            plans = lead / '.fullops-squad/PLANS.md'
            plans.write_text(plans.read_text(encoding='utf-8').replace('| 대기 |', '| 보류(측정) |'), encoding='utf-8', newline='\n')

        def opened():  # 열린 Run: lead가 worker를 띄웠고 그 worker가 아직 돌고 있다
            self.dispatched.append({'key': 'O-1', 'role': 'dev', 'run': 'run_open', 'dispatch': 'ctx_open', 'task': 'task_open',
                                    'terminal': 'term_open', 'spec': '', 'started': True, 'sub': False, 'settled': False})
            self.sync_workers()
            self.hook('PreToolUse', 'lead', subject='Bash', **bash('orca orchestration worker-start --run run_open --spec x'))

        def worker():  # dispatch가 묶이고 코드 커밋이 있는 worker 세션
            self.prompt('dev', None, self.preamble({'task': 'task_open', 'dispatch': 'ctx_open', 'spec': ''}))
            self.worker_works('dev', 'W-1', None, 3)

        cases = [
            ('inactive', None, ('PreToolUse', 'plain', bash('ls -la'))),
            ('pre_bash', None, ('PreToolUse', 'lead', bash('ls -la'))),
            ('pre_edit', None, ('PreToolUse', 'dev', edit(str(dev / 'src/app.py')))),
            ('pre_inbox_edit', None, ('PreToolUse', 'lead', edit(inbox))),
            ('pre_worker_start', None, ('PreToolUse', 'lead', bash('orca orchestration worker-start --spec x'))),
            ('pre_git_commit', None, ('PreToolUse', 'dev', bash('git commit -m x'))),
            ('stop_lead_clean', None, stop),
            ('stop_lead_pending10', pending, stop),
            ('stop_lead_ready', lambda: (settle(), ready()), stop),
            ('stop_lead_open_run', lambda: (held(), opened()), stop),
            ('stop_worker', worker, ('Stop', 'dev', {'stop_hook_active': False, 'background_tasks': []})),
            ('session_start_lead', None, ('SessionStart', 'lead', {'source': 'startup'})),
            ('session_start_worker', None, ('SessionStart', 'dev', {'source': 'startup'})),
        ]
        out = {}
        for name, prepare, (event, who, fields) in cases:
            if prepare:
                prepare()
            subject = fields.get('tool_name') or fields.get('source') or ''
            count = lambda name: sum(1 for matcher, _ in self.hooks[name] if not matcher or re.fullmatch(matcher, subject))

            def once(env=None):
                if event == 'Stop':  # 한 번 막기가 매번 처음처럼 걸리게 턴을 넘긴다(시간에 넣지 않는다)
                    self.prompt(who, None, '측정')
                self.ms, procs = 0.0, count(event)
                verdict, _ = self.hook(event, who, subject=subject, env=env, **fields)
                if event == 'PreToolUse' and flow_of(verdict) == 'continues':  # 1.24.0은 도구 호출마다 PostToolUse도 돈다
                    self.hook('PostToolUse', who, subject=subject, env=env, tool_response={}, **fields)
                    procs += count('PostToolUse')
                return self.ms, procs, verdict
            # 세는 실행: git 프로세스(git의 추적), Orca 호출(fake-orca의 기록)
            trace = self.root / f'trace-{name}.jsonl'
            calls = len(self.orca_calls())
            _, procs, verdict = once({'GIT_TRACE2_EVENT': str(trace)})
            orca = len(self.orca_calls()) - calls
            starts = [json.loads(line) for line in trace.read_text(encoding='utf-8').splitlines()] if trace.is_file() else []
            times = sorted([once()[0] for _ in range(runs)][min(runs // 15, runs - 1):])
            at = lambda q: round(times[min(len(times) - 1, int(len(times) * q))], 1)
            out[name] = {'p50': at(0.5), 'p95': at(0.95), 'max': round(times[-1], 1), 'procs': procs, 'verdict': verdict,
                         'git': sum(1 for e in starts if e.get('event') == 'start' and '/' not in e.get('sid', '')), 'orca': orca}
        return {'cases': out}


def result(act, verdict, output='', code=None):
    return {'act': act, 'verdict': verdict, 'flow': flow_of(verdict), 'exit': code, 'ms': 0, 'output': output[-4000:]}


def remove(root):
    """임시 폴더를 지운다. Windows에서는 git 객체가 읽기 전용이라 권한을 풀고 다시 지운다."""
    def retry(function, path, _):
        os.chmod(path, 0o700)
        function(path)
    handler = {'onexc': retry} if sys.version_info >= (3, 12) else {'onerror': retry}
    try:
        shutil.rmtree(root, **handler)
    except OSError:
        pass  # 남은 폴더는 임시 폴더 정리에 맡긴다


def main():
    if len(sys.argv) != 2 and sys.argv[1:2] != ['--bench']:
        sys.exit(__doc__)
    bench = sys.argv[1] == '--bench'
    scenario = {} if bench else tomllib.loads(Path(sys.argv[1]).read_text(encoding='utf-8'))
    setup = {'roles': {'dev': 'worker', 'architecture': 'design'}} if bench else scenario.get('setup', {})
    driver = Driver(setup)
    try:
        if bench:
            out = driver.bench(int(sys.argv[2]))
        else:
            steps = []
            for step in scenario.get('step', []):
                driver.ms = 0.0
                try:
                    done = driver.step(step)
                except Unsupported as why:
                    done = result(step['act'], 'n/a', str(why))
                # 준비 동사는 에이전트가 한 일이 아니라 시간을 적지 않는다(rs 구동기와 같다)
                done['ms'] = 0 if step['act'] in PREP else round(driver.ms)
                steps.append(done)
            out = {'name': scenario['name'], 'steps': steps, 'merged': driver.merged()}
    finally:
        if 'FULLOPS_TEST_KEEP' not in os.environ:
            remove(driver.root)
    print(json.dumps(out, ensure_ascii=False))


if __name__ == '__main__':
    main()
