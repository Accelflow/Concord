#!/usr/bin/env python3
"""Concord's standard validation entry; also implements the Conflux result contract."""
import json
import os
import pathlib
import platform
import re
import subprocess
import sys
import time

from conflux_link import attach

root = pathlib.Path(__file__).resolve().parents[1]
context = json.loads(os.environ.get('CONFLUX_VERIFICATION_CONTEXT_JSON', '{}'))
output = pathlib.Path(context.get('output_dir', root / '.concord/verification' / str(time.time_ns())))
output.mkdir(parents=True, exist_ok=True)
started = time.monotonic()
steps = []
passed = 0

def run(label, argv, cwd=root):
    with (output / (label + '.log')).open('w') as log:
        result = subprocess.run(argv, cwd=cwd, stdout=log, stderr=subprocess.STDOUT)
    steps.append({'step': label, 'command': argv, 'exit_code': result.returncode})
    print(label, result.returncode, flush=True)
    if result.returncode:
        raise RuntimeError(label + ' failed; see ' + str(output / (label + '.log')))
    if run_id := context.get('run_id'):
        print(json.dumps({'conflux_progress': {
            'run_id': run_id,
            'phase': 'standard-verification',
            'sequence': len(steps),
            'completed': len(steps),
            'checkpoint': label,
        }}), flush=True)

try:
    run('format', ['cargo', 'fmt', '--check'])
    run('selection-assurance-guards', ['python3', 'scripts/verify-selection-guards.py'])
    run('fixture-install', ['corepack', 'pnpm', 'install', '--frozen-lockfile', '--ignore-scripts'], root / 'tests/fixtures/vitest')
    run('clippy', ['cargo', 'clippy', '--all-targets', '--locked', '--', '-D', 'warnings'])
    run('tests', ['cargo', 'test', '--all-targets', '--locked', '--', '--test-threads=1'])
    counts = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', (output / 'tests.log').read_text())
    assert counts and all(int(f) == int(s) == 0 for _, f, s in counts)
    passed = sum(int(p) for p, _, _ in counts)
    assert passed >= 60, 'required CLI E2E cases missing'
    run('definitions', ['cargo', 'run', '--locked', '--quiet', '--', 'check', '--json'])
    definitions = json.loads((output / 'definitions.log').read_text())
    assert definitions['valid'] and definitions['configuration_checked'], definitions
    run('self-verification', ['python3', 'scripts/verify-self.py', str(output / 'self-verification')])
    link = attach(root, output / 'self-verification', context) if context else None
    run('release-age', ['python3', 'scripts/check-release-age.py', str(output / 'dependency-release-age.json')])
    run('cargo-audit', ['python3', 'scripts/audit.py', str(output / 'cargo-audit.json')])
    run('npm-audit', ['corepack', 'pnpm', 'audit', '--json'], root / 'tests/fixtures/vitest')
    result = {'schema': 'conflux.verification-result.v1', 'passed': passed, 'failed': 0, 'skipped': 0, 'ui_mode': 'headless', 'os': {'Darwin':'macos'}.get(platform.system(),platform.system().lower()), 'steps': steps, 'elapsed_seconds': time.monotonic() - started}
    if link is not None:
        result['concord'] = link
except Exception as error:
    result = {'schema': 'conflux.verification-result.v1', 'passed': passed, 'failed': 1, 'skipped': 0, 'ui_mode': 'headless', 'os': {'Darwin':'macos'}.get(platform.system(),platform.system().lower()), 'steps': steps, 'error': str(error), 'elapsed_seconds': time.monotonic() - started}
(output / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result))
sys.exit(0 if not result['failed'] else 1)
