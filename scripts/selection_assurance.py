"""Independent expected selections and selective/full black-box CLI verification."""

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time

EXPECTED_COMMANDS = {'direct', 'sibling', 'indirect', 'final'}
EXPECTED_RELATIONS = {'R-DIRECT', 'R-SIBLING', 'R-INDIRECT', 'R-FINAL'}
EXPECTED_PROPERTIES = {'P-DIRECT', 'P-SIBLING', 'P-INDIRECT', 'P-FINAL'}
EXPECTED_CONTRACTS = {'C-BASE', 'C-UPPER', 'C-FINAL'}
RELATION_COMMANDS = {
    'R-DIRECT': 'direct', 'R-SIBLING': 'sibling', 'R-INDIRECT': 'indirect',
    'R-FINAL': 'final', 'R-UNRELATED': 'unrelated', 'R-UNUSED': 'unrelated',
}


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def validate_scenario(plan, selected, full, failure, unknown=False):
    """Check expected coverage, execution and injected outcomes independently."""
    wanted = EXPECTED_COMMANDS | ({'unrelated'} if unknown else set())
    relations = set(RELATION_COMMANDS) if unknown else EXPECTED_RELATIONS
    properties = EXPECTED_PROPERTIES | ({'P-UNUSED'} if unknown else set())
    contracts = EXPECTED_CONTRACTS | ({'C-UNRELATED'} if unknown else set())
    require(plan['mode'] == ('all' if unknown else 'selected'), 'incorrect selection mode')
    ids = [c['id'] for c in plan['selected_commands']]
    require(set(ids) == wanted and len(ids) == len(wanted), 'missing/extra command selection')
    require(plan['selected_files'] == [], 'unexpected native test selection')
    for field, expected in [('affected_relations', relations),
                            ('affected_properties', properties), ('affected_contracts', contracts)]:
        require(set(plan[field]) == expected, 'incorrect ' + field)
    for run, commands, edges in [(selected, wanted, relations),
                                  (full, EXPECTED_COMMANDS | {'unrelated'}, set(RELATION_COMMANDS))]:
        require(run['input_unchanged'] and run['result_integrity'], 'untrusted execution result')
        actual = [c['id'] for c in run['command_results']]
        require(set(actual) == commands and len(actual) == len(commands), 'missing/extra command execution')
        failed = failure in commands
        require(run['exit_code'] == (1 if failed else 0), 'CLI exit code masks failure')
        require(run['status'] == ('failed' if failed else 'passed'), 'CLI status masks failure')
        for c in run['command_results']:
            require(c['status'] == ('failed' if c['id'] == failure else 'passed'), 'command status masks failure')
            require(c['exit_code'] == (7 if c['id'] == failure else 0), 'command exit code mismatch')
        for field in ['obligation_results', 'property_results']:
            rows = run[field]
            require({o['relation_id'] for o in rows} == edges and len(rows) == len(edges), 'missing/extra ' + field)
            for row in rows:
                expected = 'failed' if RELATION_COMMANDS[row['relation_id']] == failure else 'verified'
                require(row['status'] == expected, field + ' masks failure')
        if not failed:
            require(not run['plan']['property_gaps'], 'unexpected verification gap')
    selected_results = {(c['id'], c['status'], c['exit_code']) for c in selected['command_results']}
    full_results = {(c['id'], c['status'], c['exit_code']) for c in full['command_results']}
    require(selected_results <= full_results, 'selected execution differs from full execution')


def fixture(root):
    """A repository with no import edges; only declared semantic relationships."""
    (root / '.gitignore').write_text('.concord/\n')
    (root / 'implementation.txt').write_text('before\n')
    # Command-mode selection must also work in a repository with unchanged
    # frontend configuration; the fixture does not invoke these tools.
    (root / 'tsconfig.json').write_text('{"compilerOptions": {}}\n')
    (root / 'vite.config.ts').write_text('export default {};\n')
    (root / '.env.example').write_text('# Unchanged public example configuration\n')
    (root / 'check.py').write_text('''import json, os, pathlib, sys
name = sys.argv[1]
failed = os.environ.get('CONCORD_ASSURANCE_FAILURE') == name
directory = pathlib.Path(os.environ['CONCORD_ASSURANCE_MARKERS'])
directory.mkdir(parents=True, exist_ok=True)
(directory / (name + '.json')).write_text(json.dumps({'id': name, 'failed': failed}))
print(name, 'failed' if failed else 'passed')
sys.exit(7 if failed else 0)
''')
    config = 'schema_version=1\n[runner]\nkind="commands"\n'
    for name in ['direct', 'sibling', 'indirect', 'final', 'unrelated']:
        config += '\n[[commands]]\nid=' + json.dumps(name) + '\nprogram=' + json.dumps(sys.executable)
        config += '\nargs=' + json.dumps(['check.py', name]) + '\nfiles=["check.py"]\n'
    (root / 'concord.toml').write_text(config)
    def contract(name):
        return dict(id=name, version='1', description=name, scope=name, responsibility=name,
                    assumptions=[], preconditions=[], approval_status='approved', requirement_ids=['REQ'])
    def prop(name, dependencies):
        return dict(id=name, version='1', description=name, depends_on=dependencies,
                    implementations=[], obligations=[], approval_status='approved')
    def relation(name, contract_id, property_id, files=None):
        return dict(id=name, contract_id=contract_id, property_id=property_id, relation='requires',
                    implementations=files or [], obligations=[dict(id='O-' + name, method='integration',
                    commands=[RELATION_COMMANDS[name]])])
    doc = dict(schema_version=2, requirements=[dict(id='REQ', description='Select all necessary verification')],
               contracts=[contract(c) for c in sorted(EXPECTED_CONTRACTS | {'C-UNRELATED'})],
               properties=[prop('P-DIRECT', []), prop('P-SIBLING', []),
                           prop('P-INDIRECT', ['P-SIBLING']), prop('P-FINAL', ['P-INDIRECT']), prop('P-UNUSED', [])],
               relations=[relation('R-DIRECT', 'C-BASE', 'P-DIRECT', ['implementation.txt']),
                          relation('R-SIBLING', 'C-BASE', 'P-SIBLING'),
                          relation('R-INDIRECT', 'C-UPPER', 'P-INDIRECT'),
                          relation('R-FINAL', 'C-FINAL', 'P-FINAL'),
                          relation('R-UNRELATED', 'C-UNRELATED', 'P-DIRECT'),
                          relation('R-UNUSED', 'C-UNRELATED', 'P-UNUSED')])
    (root / 'concord').mkdir()
    (root / 'concord/contracts.yaml').write_text(json.dumps(doc))
    for args in [['init', '-b', 'main'], ['config', 'user.name', 'Concord verification'],
                 ['config', 'user.email', 'verification@example.invalid'], ['add', '.'], ['commit', '-m', 'fixture']]:
        subprocess.run(['git', *args], cwd=root, check=True, capture_output=True)


def exercise(binary, output):
    results = []
    with tempfile.TemporaryDirectory(prefix='concord-selection-assurance-') as tmp:
        root = pathlib.Path(tmp)
        fixture(root)
        marker_root = root / '.concord/markers'
        def invoke(label, args, failure=None):
            shutil.rmtree(marker_root, ignore_errors=True)
            env = {**os.environ, 'CONCORD_ASSURANCE_MARKERS': str(marker_root),
                   'CONCORD_ASSURANCE_FAILURE': failure or ''}
            p = subprocess.run([str(binary), *args, '--json'], cwd=root, env=env,
                               capture_output=True, timeout=60)
            (output / (label + '.stdout.json')).write_bytes(p.stdout)
            (output / (label + '.stderr.log')).write_bytes(p.stderr)
            value = json.loads(p.stdout)
            if args[0] == 'verify':
                require(p.returncode == value['exit_code'], 'process/report exit mismatch')
            else:
                require(p.returncode == 0, 'CLI authoring/planning failed')
            markers = [json.loads(p.read_text()) for p in sorted(marker_root.glob('*.json'))]
            (output / (label + '.markers.json')).write_text(json.dumps(markers, indent=2))
            return value, markers
        checked, _ = invoke('check', ['check'])
        require(checked['valid'], 'fixture definitions rejected')
        scan, _ = invoke('scan', ['scan'])
        require(not scan['tests'], 'command-only fixture collected native tests')
        noop, _ = invoke('noop-plan', ['plan', '--base', 'HEAD'])
        require(noop['mode'] == 'noop' and not noop['selected_commands'], 'clean fixture must be noop')
        (root / 'implementation.txt').write_text('after\n')
        cases = [(None, False), ('direct', False), ('sibling', False), ('indirect', False),
                 ('final', False), ('unrelated', False), ('unrelated', True)]
        for failure, unknown in cases:
            label = ('unknown-' if unknown else '') + (failure or 'positive')
            if unknown:
                (root / 'unknown-input.bin').write_bytes(b'unknown')
            plan, _ = invoke(label + '-plan', ['plan', '--base', 'HEAD'], failure)
            explanation, _ = invoke(label + '-explain', ['explain', 'final'], failure)
            require(explanation['command']['selected'], 'indirect explanation missing')
            selected, markers = invoke(label + '-selected', ['verify', '--base', 'HEAD'], failure)
            expected = EXPECTED_COMMANDS | ({'unrelated'} if unknown else set())
            require({m['id'] for m in markers} == expected, 'selected commands were not actually executed')
            require(all(m['failed'] == (m['id'] == failure) for m in markers), 'fault injection not observed')
            full, markers = invoke(label + '-full', ['verify', '--all', '--base', 'HEAD'], failure)
            require({m['id'] for m in markers} == EXPECTED_COMMANDS | {'unrelated'}, 'full commands were not actually executed')
            require(all(m['failed'] == (m['id'] == failure) for m in markers), 'full fault injection not observed')
            validate_scenario(plan, selected, full, failure, unknown)
            results.append(dict(scenario=label, selected_exit=selected['exit_code'], full_exit=full['exit_code'],
                                selected_commands=len(selected['command_results']), full_commands=len(full['command_results']),
                                passed=True))
    return results


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--concord', default='target/debug/concord')
    parser.add_argument('--output')
    args = parser.parse_args()
    output = pathlib.Path(args.output or ('.concord/selection-assurance/' + str(time.time_ns()))).resolve()
    output.mkdir(parents=True, exist_ok=False)
    binary = pathlib.Path(args.concord).resolve()
    require(binary.is_file(), 'build the Concord CLI before running selection assurance')
    results = exercise(binary, output)
    summary = dict(passed=True, scenarios=results, scenario_count=len(results), output=str(output),
                   cli_sha256=hashlib.sha256(binary.read_bytes()).hexdigest())
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary))
