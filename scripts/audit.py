#!/usr/bin/env python3
"""Audit the exact Cargo.lock packages against OSV. No source or credentials sent."""
import json
import pathlib
import subprocess
import sys
import urllib.request

root = pathlib.Path(__file__).resolve().parents[1]
output = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else root / 'evidence/cargo-audit.json'
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version=1'], cwd=root))
packages = sorted({(p['name'], p['version']) for p in metadata['packages'] if (p.get('source') or '').startswith('registry+')})
queries = [{'package': {'ecosystem': 'crates.io', 'name': n}, 'version': v} for n, v in packages]
request = urllib.request.Request('https://api.osv.dev/v1/querybatch', data=json.dumps({'queries': queries}).encode(), headers={'Content-Type': 'application/json'})
try:
    report = json.load(urllib.request.urlopen(request, timeout=60))
    results = report['results']
    assert len(results) == len(packages)
    findings = [{'name': name, 'version': version, 'vulns': result.get('vulns', [])} for (name, version), result in zip(packages, results) if result.get('vulns')]
    # Incomplete batch responses cannot be accepted as an empty audit.
    assert not any(r.get('next_page_token') for r in results)
    result = {'source': 'https://api.osv.dev/v1/querybatch', 'packages': len(packages), 'findings': findings, 'passed': not findings}
except Exception as error:
    result = {'passed': False, 'status': 'audit_unavailable', 'error': str(error)}
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result))
sys.exit(0 if result['passed'] else 1)
