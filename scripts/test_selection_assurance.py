import unittest

from selection_assurance import (
    EXPECTED_COMMANDS, EXPECTED_RELATIONS, EXPECTED_PROPERTIES,
    EXPECTED_CONTRACTS, RELATION_COMMANDS, validate_scenario,
)


def evidence(failure=None):
    def run(commands, relations):
        failed = failure in commands
        return {
            'exit_code': 1 if failed else 0, 'status': 'failed' if failed else 'passed',
            'input_unchanged': True, 'result_integrity': True,
            'command_results': [{'id': c, 'status': 'failed' if c == failure else 'passed',
                                 'exit_code': 7 if c == failure else 0} for c in sorted(commands)],
            'obligation_results': [{'relation_id': r, 'status': 'failed' if RELATION_COMMANDS[r] == failure else 'verified'} for r in sorted(relations)],
            'property_results': [{'relation_id': r, 'status': 'failed' if RELATION_COMMANDS[r] == failure else 'verified'} for r in sorted(relations)],
            'plan': {'property_gaps': []},
        }
    plan = {'mode': 'selected', 'selected_files': [],
            'selected_commands': [{'id': c} for c in sorted(EXPECTED_COMMANDS)],
            'affected_relations': sorted(EXPECTED_RELATIONS),
            'affected_properties': sorted(EXPECTED_PROPERTIES),
            'affected_contracts': sorted(EXPECTED_CONTRACTS)}
    selected = run(EXPECTED_COMMANDS, EXPECTED_RELATIONS)
    full = run(EXPECTED_COMMANDS | {'unrelated'}, EXPECTED_RELATIONS | {'R-UNRELATED', 'R-UNUSED'})
    return plan, selected, full


class SelectionAssurance(unittest.TestCase):
    def test_valid_positive_and_indirect_failure_are_accepted(self):
        for failure in [None, 'indirect']:
            validate_scenario(*evidence(failure), failure)

    def test_missing_indirect_selection_is_rejected_even_if_both_runs_pass(self):
        plan, selected, full = evidence()
        plan['selected_commands'] = [c for c in plan['selected_commands'] if c['id'] != 'indirect']
        selected['command_results'] = [c for c in selected['command_results'] if c['id'] != 'indirect']
        with self.assertRaises(AssertionError):
            validate_scenario(plan, selected, full, None)

    def test_missing_command_execution_is_rejected(self):
        plan, selected, full = evidence()
        selected['command_results'].pop()
        with self.assertRaises(AssertionError):
            validate_scenario(plan, selected, full, None)

    def test_missing_relation_property_or_contract_is_rejected(self):
        for field in ['affected_relations', 'affected_properties', 'affected_contracts']:
            plan, selected, full = evidence()
            plan[field].pop()
            with self.assertRaises(AssertionError):
                validate_scenario(plan, selected, full, None)

    def test_unrelated_selection_is_rejected(self):
        plan, selected, full = evidence()
        plan['selected_commands'].append({'id': 'unrelated'})
        with self.assertRaises(AssertionError):
            validate_scenario(plan, selected, full, None)

    def test_missing_obligation_result_is_rejected(self):
        plan, selected, full = evidence()
        selected['obligation_results'].pop()
        with self.assertRaises(AssertionError):
            validate_scenario(plan, selected, full, None)

    def test_missing_property_result_is_rejected(self):
        plan, selected, full = evidence()
        selected['property_results'].pop()
        with self.assertRaises(AssertionError):
            validate_scenario(plan, selected, full, None)

    def test_masked_failure_is_rejected_even_when_both_reports_claim_success(self):
        with self.assertRaises(AssertionError):
            validate_scenario(*evidence(), 'indirect')

    def test_nonzero_command_exit_or_cli_success_mismatch_is_rejected(self):
        for mutate in ['command', 'cli']:
            plan, selected, full = evidence('indirect')
            if mutate == 'command':
                for run in [selected, full]:
                    next(c for c in run['command_results'] if c['id'] == 'indirect')['exit_code'] = 0
            else:
                selected['exit_code'] = 0
            with self.assertRaises(AssertionError):
                validate_scenario(plan, selected, full, 'indirect')

    def test_incomplete_or_changed_input_is_rejected(self):
        for field in ['input_unchanged', 'result_integrity']:
            plan, selected, full = evidence()
            selected[field] = False
            with self.assertRaises(AssertionError):
                validate_scenario(plan, selected, full, None)

    def test_unrelated_failure_does_not_require_unrelated_selection(self):
        plan, selected, full = evidence('unrelated')
        validate_scenario(plan, selected, full, 'unrelated')


if __name__ == '__main__':
    unittest.main()
