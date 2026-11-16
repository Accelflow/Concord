#!/usr/bin/env python3
"""Check locked Cargo dependencies against the 168-hour adoption rule."""
import concurrent.futures
import datetime
import json
import pathlib
import subprocess
import sys
import urllib.request

root = pathlib.Path(__file__).resolve().parents[1]
meta = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version=1'], cwd=root))
packages = sorted({(p['name'],p['version']) for p in meta['packages'] if (p.get('source') or '').startswith('registry+')})
cutoff = datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(hours=168)

def check(pair):
    name, version = pair
    try:
        req = urllib.request.Request(f'https://crates.io/api/v1/crates/{name}/{version}', headers={'User-Agent': 'Concord dependency validation'})
        release = json.load(urllib.request.urlopen(req, timeout=30))['version']
        published = datetime.datetime.fromisoformat(release['created_at'].replace('Z','+00:00'))
        return {'name': name, 'version': version, 'created_at': release['created_at'], 'mature': published <= cutoff, 'yanked': release['yanked']}
    except Exception as error:
        return {'name': name, 'version': version, 'mature': False, 'error': str(error)}
with concurrent.futures.ThreadPoolExecutor(8) as pool:
    releases = list(pool.map(check, packages))
result = {'passed': all(p['mature'] and not p.get('yanked') for p in releases), 'cutoff': cutoff.isoformat(), 'packages': releases}
path = pathlib.Path(sys.argv[1]) if len(sys.argv)>1 else root/'evidence/dependency-release-age.json'
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'passed':result['passed'], 'packages':len(releases),'failures':[p for p in releases if not p['mature'] or p.get('yanked')]}))
sys.exit(0 if result['passed'] else 1)
