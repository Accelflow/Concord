#!/usr/bin/env python3
"""Exercise approved repository definitions and require successful plan/verify."""
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[1]
out = pathlib.Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
expected_selected_commands = {'repository-format', 'repository-definitions', 'selection-assurance-guards', 'selection-assurance'}
expected_full_commands = expected_selected_commands | {'conflux-evidence-guards'}
expected_selected_relations = {'REL-DEFINITION-READONLY', 'REL-DEFINITION-REFERENCE', 'REL-WORKFLOW-COMPOSITION', 'REL-WORKFLOW-COMPLETENESS'}
expected_scenarios = {'positive', 'direct', 'sibling', 'indirect', 'final', 'unrelated', 'unknown-unrelated'}

def invoke(label, args):
    command = ['cargo', 'run', '--locked', '--quiet', '--', *args, '--json']
    with (out / (label + '.stderr.log')).open('w') as stderr:
        result = subprocess.run(command, cwd=root, stdout=subprocess.PIPE, stderr=stderr)
    (out / (label + '.json')).write_bytes(result.stdout)
    return result.returncode, json.loads(result.stdout)

# Fixed-source checkouts may have no parent commit. Exercise unknown Rust impact
# with an owned, untracked input, then restore the checkout before verification.
probe = root / 'concord-plan-probe.rs'
with probe.open('x') as file:
    file.write('// Standard verification input; not part of a Cargo target.\n')
try:
    code, plan = invoke('plan', ['plan', '--base', 'HEAD'])
finally:
    probe.unlink()
assert 'concord-plan-probe.rs' in plan['changed_files'], plan
assert code == 0 and plan['selected_files'] and plan['tests'], plan
assert plan['mode'] == 'all' and not plan['excluded_files'], plan
# A real repository test-source change: compiler deps select its target; the
# binding regression also reads tests/ at runtime and must remain selected.
changed = root / 'tests/definition_check.rs'
original = changed.read_bytes()
modified = original + b'\n// Concord standard impact-selection probe.\n'
try:
    changed.write_bytes(modified)
    selected_code, selected_plan = invoke('selected-plan', ['plan', '--base', 'HEAD'])
    assert selected_code == 0 and selected_plan['mode'] == 'selected', selected_plan
    assert set(selected_plan['selected_files']) == {'tests/definition_check.rs', 'tests/split_definitions.rs', 'tests/commands.rs', 'tests/relations.rs'}, selected_plan
    assert set(selected_plan['affected_relations']) == expected_selected_relations, selected_plan
    assert {c['id'] for c in selected_plan['selected_commands']} == expected_selected_commands, selected_plan
    selected_code, selected_run = invoke('selected-verify', ['verify', '--base', 'HEAD', '--expect-plan', selected_plan['plan_id']])
    assert selected_code == selected_run['exit_code'] == 0 and selected_run['status'] == 'passed' and selected_run['passed'] == 31, selected_run
    assert {c['id'] for c in selected_run['command_results']} == expected_selected_commands and len(selected_run['command_results']) == 4 and all(c['status'] == 'passed' for c in selected_run['command_results']), selected_run
    assert {'P-VERIFICATION-WORKFLOW', 'P-SELECTION-COMPLETENESS'} <= set(selected_plan['affected_properties']), selected_plan
    assert selected_run['failed'] == selected_run['skipped'] == 0, selected_run
    assert selected_run['input_unchanged'] and selected_run['result_integrity'], selected_run
    assert selected_run['obligation_results'] and all(o['status'] == 'verified' for o in selected_run['obligation_results']), selected_run
    assert not selected_run['plan']['property_gaps'], selected_run
    assert selected_run['property_results'] and all(p['status'] == 'verified' for p in selected_run['property_results']), selected_run
finally:
    changed.write_bytes(original)

command_tests = root / 'tests/commands.rs'
command_tests_original = command_tests.read_bytes()
try:
    command_tests.write_bytes(command_tests_original + b'\n// Concord evidence impact-selection probe.\n')
    evidence_code, evidence_plan = invoke('evidence-plan', ['plan', '--base', 'HEAD'])
    assert evidence_code == 0 and evidence_plan['mode'] == 'selected', evidence_plan
    assert {'REL-COMMAND-CASE-EVIDENCE', 'REL-CONFLUX-EVIDENCE-INTEGRITY'} <= set(evidence_plan['affected_relations']), evidence_plan
    assert 'conflux-evidence-guards' in {c['id'] for c in evidence_plan['selected_commands']}, evidence_plan
    assert not evidence_plan['property_gaps'], evidence_plan
finally:
    command_tests.write_bytes(command_tests_original)

plan_code, full_plan = invoke('full-plan', ['plan', '--all', '--base', 'HEAD'])
assert plan_code == 0 and full_plan['mode'] == 'all', full_plan
code, run = invoke('verify', ['verify', '--all', '--base', 'HEAD', '--expect-plan', full_plan['plan_id']])
assert run['plan'] == full_plan, run
# Definitions were explicitly reviewed and approved; no missing approval or evidence is allowed.
assert code == run['exit_code'] == 0 and run['status'] == 'passed', run
assert run['passed'] >= 60 and run['failed'] == run['skipped'] == 0, run
assert {c['id'] for c in run['command_results']} == expected_full_commands and len(run['command_results']) == 5 and all(c['status'] == 'passed' for c in run['command_results']), run
assert run['result_integrity'] and run['input_unchanged'], run
assert run['obligation_results'] and all(o['status'] == 'verified' for o in run['obligation_results']), run
assert len(run['property_results']) == len(run['plan']['relations']) == 11, run
assert all(p['status'] == 'verified' and p['approval_status'] == 'approved' for p in run['property_results']), run
assert not run['plan']['property_gaps'], run
assert all(c['approval_status'] == 'approved' for c in run['plan']['contracts']), run
assurance = {}
for label, result in [('selected', selected_run), ('full', run)]:
    guard = next(c for c in result['command_results'] if c['id'] == 'selection-assurance-guards')
    assert guard['report']['passed'] == 11 and guard['report']['failed'] == guard['report']['skipped'] == 0, guard
    assert len({case['id'] for case in guard['report']['cases']}) == 11, guard
    guard_report = json.loads(pathlib.Path(guard['stdout']).read_text())
    assert guard_report == {'tests_run': 11, 'failed': 0, 'skipped': 0}, guard_report
    command = next(c for c in result['command_results'] if c['id'] == 'selection-assurance')
    proof = json.loads(pathlib.Path(command['stdout']).read_text())
    assert proof['passed'] and proof['scenario_count'] == 7, proof
    assert {s['scenario'] for s in proof['scenarios']} == expected_scenarios, proof
    assert all(s['passed'] for s in proof['scenarios']), proof
    shutil.copytree(proof['output'], out / (label + '-selection-assurance'))
    assurance[label] = proof
selected_commands = {(c['id'], c['status'], c['exit_code']) for c in selected_run['command_results']}
full_commands = {(c['id'], c['status'], c['exit_code']) for c in run['command_results']}
assert selected_commands <= full_commands, (selected_commands, full_commands)
evidence_guard = next(c for c in run['command_results'] if c['id'] == 'conflux-evidence-guards')
assert evidence_guard['report']['passed'] == 6, evidence_guard
assert evidence_guard['report']['failed'] == evidence_guard['report']['skipped'] == 0, evidence_guard
assert any(o['obligation_id'] == 'O-CONFLUX-EVIDENCE-INTEGRITY'
           and o['command_ids'] == ['conflux-evidence-guards']
           for o in run['obligation_results']), run
selected_cases = {(c['file'],c['name'],c['status']) for c in selected_run['cases']}
full_cases = {(c['file'],c['name'],c['status']) for c in run['cases']}
assert selected_cases <= full_cases, (selected_cases, full_cases)
summary = {'selection_assurance_scenarios': 7, 'expected_selected_relations': sorted(expected_selected_relations), 'selection_assurance': assurance, 'selected_verify_exit_code': selected_code, 'selected_tests_passed': selected_run['passed'], 'selected_files': selected_plan['selected_files'], 'excluded_files': selected_plan['excluded_files'], 'selected_commands_passed': len(selected_run['command_results']), 'commands_passed': len(run['command_results']), 'selected_run_id': selected_run['run_id'], 'probe_original_sha256': hashlib.sha256(original).hexdigest(), 'probe_modified_sha256': hashlib.sha256(modified).hexdigest(), 'selected_cases_match_full': True, 'passed': True, 'source_revision': run['plan']['head_sha'], 'run_id': run['run_id'], 'tests_passed': run['passed'], 'verify_exit_code': code, 'obligations': len(run['obligation_results']), 'relations': len(run['property_results']), 'definitions_approved': True, 'plan_id': full_plan['plan_id'], 'unknown_input_plan_id': plan['plan_id'], 'evidence_plan_id': evidence_plan['plan_id']}
(out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary))
