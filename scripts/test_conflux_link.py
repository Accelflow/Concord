"""Exercise the public Conflux evidence link with independently constructed input."""
import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from conflux_link import attach


class ConfluxLink(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.evidence = self.root / 'output' / 'self-verification'
        self.evidence.mkdir(parents=True)
        self.context = {
            'schema': 1, 'run_id': 'conflux-run', 'assignment_id': 'assignment',
            'source': {'head': 'a' * 40, 'tree': 'b' * 40, 'bundle_sha256': 'c' * 64},
        }
        self.case = {'file': 'tests/save.rs', 'name': 'saves'}
        self.plan = {
            'head_sha': 'a' * 40, 'base_sha': 'a' * 40, 'plan_id': 'plan',
            'property_gaps': [], 'relations': [{'id': 'REL'}],
            'selected_files': ['tests/save.rs'], 'tests': [self.case],
            'selected_commands': [{'id': 'save', 'report': {'required_cases': ['save-case']}}],
            'required_obligations': [
                {'relation_id': 'REL', 'property_id': 'P-SAVE',
                 'obligation': {'id': 'O-SAVE', 'commands': ['save'], 'tests': [self.case]}}
            ],
        }
        cases = [{'id': 'save-case', 'status': 'passed'}]
        raw = json.dumps({'schema': 'concord.command-report.v1', 'cases': cases}).encode()
        directory = hashlib.sha256(b'save').hexdigest()
        self.raw_path = self.root / '.concord/runs/concord-run/commands' / directory / 'report.json'
        self.raw_path.parent.mkdir(parents=True)
        self.raw_path.write_bytes(raw)
        self.run = {
            'plan': copy.deepcopy(self.plan), 'run_id': 'concord-run', 'exit_code': 0,
            'status': 'passed', 'input_unchanged': True, 'result_integrity': True,
            'passed': 1, 'failed': 0, 'skipped': 0, 'cases': [dict(self.case, status='passed')],
            'command_results': [
                {'id': 'save', 'status': 'passed', 'exit_code': 0,
                 'report': {'passed': 1, 'failed': 0, 'skipped': 0,
                            'cases': cases, 'sha256': hashlib.sha256(raw).hexdigest()}}
            ],
            'obligation_results': [
                {'relation_id': 'REL', 'property_id': 'P-SAVE', 'obligation_id': 'O-SAVE',
                 'command_ids': ['save'], 'status': 'verified'}
            ],
            'property_results': [{'relation_id': 'REL', 'status': 'verified'}],
        }

    def link(self):
        (self.evidence / 'full-plan.json').write_text(json.dumps(self.plan))
        (self.evidence / 'verify.json').write_text(json.dumps(self.run))
        with patch('conflux_link.subprocess.check_output', side_effect=['a' * 40, 'b' * 40, '']):
            return attach(self.root, self.evidence, self.context)

    def test_links_fixed_source_obligation_and_original_report(self):
        link = self.link()
        self.assertEqual(link['plan_id'], self.plan['plan_id'])
        self.assertEqual(link['conflux_run_id'], self.context['run_id'])
        self.assertEqual(link['obligations'], self.run['obligation_results'])
        report = link['command_reports']['save']
        self.assertEqual((self.evidence.parent / report['path']).read_bytes(), self.raw_path.read_bytes())

    def test_rejects_a_different_source_or_plan(self):
        self.context['source']['tree'] = 'd' * 40
        with self.assertRaisesRegex(ValueError, 'fixed source'):
            self.link()
        self.context['source']['tree'] = 'b' * 40
        self.run['plan']['plan_id'] = 'different-plan'
        with self.assertRaisesRegex(ValueError, 'plan and execution'):
            self.link()

    def test_rejects_duplicate_relation_evidence(self):
        self.run['property_results'] *= 2
        with self.assertRaisesRegex(ValueError, 'relations'):
            self.link()

    def test_rejects_missing_or_skipped_native_cases_despite_passed_count(self):
        for cases in ([], [dict(self.case, status='skipped')]):
            with self.subTest(cases=cases):
                self.run['cases'] = cases
                with self.assertRaisesRegex(ValueError, 'native'):
                    self.link()

    def test_rejects_tampered_command_report(self):
        self.raw_path.write_text('{}')
        with self.assertRaisesRegex(ValueError, 'Command report missing or changed'):
            self.link()

    def test_rejects_wrong_property_or_missing_command_for_an_obligation(self):
        row = self.run['obligation_results'][0]
        row['command_ids'] = []
        with self.assertRaisesRegex(ValueError, 'obligations'):
            self.link()
        row['command_ids'] = ['save']
        row['property_id'] = 'P-OTHER'
        with self.assertRaisesRegex(ValueError, 'obligations'):
            self.link()
