"""Attach Concord evidence to a registered Conflux standard-runner result."""

import hashlib
import json
import re
import shutil
import subprocess


def require(condition, message):
    if not condition:
        raise ValueError(message)


def attach(root, evidence, context):
    """Bind the planned and executed scope to the runner's fixed source and IDs."""
    def git(*args):
        return subprocess.check_output(['git', *args], cwd=root, text=True).strip()

    source = context['source']
    require(source['head'] == git('rev-parse', 'HEAD')
            and source['tree'] == git('rev-parse', 'HEAD^{tree}')
            and re.fullmatch('[0-9a-f]{64}', source['bundle_sha256'])
            and not git('status', '--porcelain=v1', '--untracked-files=all'),
            'Conflux fixed source does not match the checkout')
    require(context.get('schema') == 1 and context['run_id'] and context['assignment_id'],
            'Missing Conflux execution identity')

    plan_path = evidence / 'full-plan.json'
    run_path = evidence / 'verify.json'
    plan = json.loads(plan_path.read_bytes())
    run = json.loads(run_path.read_bytes())
    require(plan['head_sha'] == source['head'] and plan['plan_id']
            and run['plan'] == plan and run['run_id'],
            'Concord plan and execution do not match the fixed source')
    require(run['exit_code'] == 0 and run['status'] == 'passed'
            and run['input_unchanged'] is True and run['result_integrity'] is True
            and not plan['property_gaps'] and run['failed'] == run['skipped'] == 0,
            'Concord execution is incomplete or unsuccessful')

    selected_files = set(plan['selected_files'])
    expected_cases = {(case['file'], case['name']) for case in plan['tests']
                      if case['file'] in selected_files}
    cases = run['cases']
    actual_cases = {(case['file'], case['name']) for case in cases}
    require(expected_cases == actual_cases and len(cases) == len(actual_cases)
            and run['passed'] == len(cases)
            and all(case['status'] == 'passed' for case in cases),
            'Missing, duplicate or unsuccessful native cases')

    commands = {command['id']: command for command in plan['selected_commands']}
    results = {command['id']: command for command in run['command_results']}
    require(commands.keys() == results.keys() and len(results) == len(run['command_results']) == len(plan['selected_commands']),
            'Missing or duplicate command execution')
    expected_obligations = {
        (row['relation_id'], row['obligation']['id']):
            (row['property_id'], row['obligation']['commands'])
        for row in plan['required_obligations']
    }
    obligations = run['obligation_results']
    require(len(obligations) == len(expected_obligations)
            and {(row['relation_id'], row['obligation_id']):
                 (row['property_id'], row.get('command_ids', []))
                 for row in obligations} == expected_obligations
            and all(row['status'] == 'verified' for row in obligations),
            'Missing or unverified Concord obligations')
    relations = run['property_results']
    require(len(relations) == len(plan['relations'])
            and {(row['relation_id'], row['status']) for row in relations}
            == {(row['id'], 'verified') for row in plan['relations']},
            'Missing or unverified Concord relations')

    reports = {}
    for command_id, command in commands.items():
        result = results[command_id]
        require(result['status'] == 'passed' and result['exit_code'] == 0,
                f'Command did not pass: {command_id}')
        if 'report' not in command:
            continue
        report = result['report']
        passed_ids = {case['id'] for case in report['cases'] if case['status'] == 'passed'}
        require(report['failed'] == report['skipped'] == 0
                and report['passed'] == len(report['cases']) == len(passed_ids) > 0
                and set(command['report']['required_cases']) <= passed_ids,
                f'Incomplete command cases: {command_id}')
        command_directory = hashlib.sha256(command_id.encode()).hexdigest()
        raw = root / '.concord/runs' / run['run_id'] / 'commands' / command_directory / 'report.json'
        raw_bytes = raw.read_bytes()
        require(hashlib.sha256(raw_bytes).hexdigest() == report['sha256']
                and json.loads(raw_bytes) == {'schema': 'concord.command-report.v1', 'cases': report['cases']},
                f'Command report missing or changed: {command_id}')
        export = evidence / 'command-reports' / (command_directory + '.json')
        export.parent.mkdir(exist_ok=True)
        shutil.copyfile(raw, export)
        reports[command_id] = {
            'path': str(export.relative_to(evidence.parent)), 'sha256': report['sha256'],
            'passed': report['passed'], 'failed': 0, 'skipped': 0,
        }

    require(run['passed'] > 0 or reports, 'Concord did not produce test case evidence')
    return {
        'schema': 'concord.conflux-link.v1',
        'conflux_run_id': context['run_id'],
        'assignment_id': context['assignment_id'],
        'source': source,
        'base': plan['base_sha'],
        'plan_id': plan['plan_id'],
        'concord_run_id': run['run_id'],
        'native_cases_passed': run['passed'],
        'obligations': obligations,
        'command_reports': reports,
        'artifacts': {
            str(path.relative_to(evidence.parent)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in (plan_path, run_path)
        },
    }
