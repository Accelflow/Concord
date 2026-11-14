"""Run unittest guards and publish the observed cases in Concord's report format."""
import json
import os
from pathlib import Path
import unittest


class CaseResult(unittest.TextTestResult):
    def __init__(self, *args):
        super().__init__(*args)
        self.passed_cases = []

    def addSuccess(self, test):
        super().addSuccess(test)
        self.passed_cases.append(test)


def run_guards(pattern, expected):
    suite = unittest.defaultTestLoader.discover(str(Path(__file__).resolve().parent), pattern=pattern)
    if suite.countTestCases() != expected:
        raise SystemExit(f'Expected {expected} cases for {pattern}')
    result = unittest.TextTestRunner(verbosity=2, resultclass=CaseResult).run(suite)
    if path := os.environ.get('CONCORD_COMMAND_REPORT'):
        cases = [{'id': test.id(), 'status': 'passed'} for test in result.passed_cases]
        for status, results in [('failed', result.failures + result.errors), ('skipped', result.skipped)]:
            cases.extend({'id': test.id(), 'status': status} for test, _ in results)
        Path(path).write_text(json.dumps({'schema': 'concord.command-report.v1', 'cases': cases}))
    summary = {'tests_run': result.testsRun, 'failed': len(result.failures) + len(result.errors),
               'skipped': len(result.skipped)}
    print(json.dumps(summary))
    return 0 if result.wasSuccessful() and result.testsRun == expected and not result.skipped else 1
