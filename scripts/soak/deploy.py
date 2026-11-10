"""Stage and start an archived, reproducible native soak on a disposable host."""
import argparse
from datetime import datetime, timezone
import hashlib
import io
import importlib.metadata
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import tarfile
import tempfile

REMOTE = r'''
from pathlib import Path
from datetime import datetime,timezone
import fcntl,hashlib,json,os,shutil,socket,subprocess,sys,tarfile,tempfile
user_dir=Path.home();root=user_dir/'soak';root.mkdir(exist_ok=True)
lock=(root/'.deploy.lock').open('a');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
status=subprocess.run(['systemctl','show','conduit-soak','--property=ActiveState','--value'],capture_output=True,text=True)
if status.returncode:raise SystemExit('Cannot establish current unit state: '+status.stderr)
state=status.stdout.strip()
if state in ('active','activating','deactivating','reloading'):
 raise SystemExit('Refusing to replace an active soak. Wait for completion.')
if (root/'out').is_dir() and any((root/'out').iterdir()):
 ev=root/'out/events.log';report=root/'out/report.txt'
 if not report.is_file() or not ev.is_file() or not any(l.endswith(' end') for l in ev.read_text().splitlines()):
  raise SystemExit('Refusing to overwrite incomplete results; inspect/archive them explicitly.')
for port in (8080,8443,9180,9001,9002,9003,9443,9100,9101,9102):
 with socket.socket() as s:
  s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
  try:s.bind(('127.0.0.1',port))
  except OSError:raise SystemExit(f'Fixture port {port} is already in use; no process was stopped.')
if shutil.disk_usage(user_dir).free < 2*1024**3:raise SystemExit('Need at least 2 GiB free disk for rotated logs and archived results.')
for command in ('nginx','nft','sudo'):
 if not shutil.which(command):raise SystemExit(f'Required test-host command missing: {command}')
with tempfile.TemporaryDirectory(prefix='.soak-stage-',dir=user_dir) as temp:
 stage=Path(temp)
 with tarfile.open(fileobj=sys.stdin.buffer,mode='r|gz') as tar:tar.extractall(stage,filter='data')
 manifest=json.loads((stage/'run-manifest.json').read_text())
 for name,expected in manifest['files'].items():
  p=(stage/name).resolve()
  if not p.is_relative_to(stage) or not p.is_file() or hashlib.sha256(p.read_bytes()).hexdigest()!=expected:
   raise SystemExit(f'Staged artifact hash mismatch: {name}')
 for name in ('conduit','oha'): (stage/name).chmod(0o755)
 subprocess.run([str(stage/'conduit'),'--version'],check=True)
 subprocess.run([str(stage/'oha'),'--version'],check=True)
 archive=root/'archive'/('run-'+datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ'))
 archive.mkdir(parents=True)
 for name in ('out','run'):
  if (root/name).exists():shutil.move(str(root/name),archive/name)
 previous=archive/'inputs';previous.mkdir()
 for p in root.iterdir():
  if p.is_file() and p.name!='.deploy.lock':shutil.copy2(p,previous/p.name)
 if (root/'python').exists():shutil.move(str(root/'python'),previous/'python')
 for p in stage.iterdir():shutil.move(str(p),root/p.name)
 # Start only after archive, validation and installation are complete.
 settings=manifest['settings']
 command=['sudo','-n','systemd-run','--quiet','--unit','conduit-soak',
          '--uid',str(os.getuid()),'--gid',str(os.getgid()),
          '-p',f'WorkingDirectory={root}','-p','LimitNOFILE=65536']
 for key,value in settings.items():command+=['--setenv',f'{key}={value}']
 command+=['/bin/bash',str(root/'soak.sh')]
 subprocess.run(['sudo','-n','systemctl','reset-failed','conduit-soak'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 subprocess.run(command,check=True)
 print(json.dumps(dict(archive=str(archive),root=str(root),binary_sha256=manifest['files']['conduit'],
                      git_head=manifest['git_head'],settings=settings),sort_keys=True))
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('host')
    parser.add_argument('hours', nargs='?', type=int, default=24)
    args = parser.parse_args()
    if args.hours <= 0:
        parser.error('hours must be positive')
    root = Path(__file__).resolve().parents[2]
    binary = Path(os.environ.get('SOAK_BIN', str(root/'dist/conduit'))).resolve()
    oha = Path(os.environ.get('OHA_BIN', shutil.which('oha') or '')).resolve()
    deps = Path(os.environ.get('SOAK_PYTHON_LIB', str(root/'local/soak-python'))).resolve()
    if not binary.is_file() or not oha.is_file():
        parser.error('Set SOAK_BIN and OHA_BIN to release binaries')
    if not (deps/'h2/__init__.py').is_file():
        parser.error('Install pinned requirements into SOAK_PYTHON_LIB before deploying')
    settings = dict(SOAK_HOURS=str(args.hours), SOAK_SCALE=os.environ.get('SOAK_SCALE','1'),
                    SOAK_UPGRADE_AT=os.environ.get('SOAK_UPGRADE_AT','2 6'))
    if not settings['SOAK_SCALE'].isdigit() or int(settings['SOAK_SCALE']) <= 0:
        parser.error('SOAK_SCALE must be a positive integer')
    if not re.fullmatch(r'\d+( \d+)*', settings['SOAK_UPGRADE_AT']):
        parser.error('SOAK_UPGRADE_AT must be space-separated nonnegative hours')
    if 'SOAK_SECONDS' in os.environ:
        if not os.environ['SOAK_SECONDS'].isdigit() or int(os.environ['SOAK_SECONDS']) <= 0:
            parser.error('SOAK_SECONDS must be positive')
        settings['SOAK_SECONDS'] = os.environ['SOAK_SECONDS']
    inputs = dict(conduit=binary, oha=oha)
    for name in ('soak.sh','ws.py','report.py','grpc.py','probe.py','requirements.txt'):
        inputs[name] = Path(__file__).parent/name
    for p in deps.rglob('*'):
        if p.is_file() and '__pycache__' not in p.parts:
            inputs['python/'+str(p.relative_to(deps))] = p
    versions = {d.metadata['Name'].lower(): d.version for d in importlib.metadata.distributions(path=[str(deps)])}
    for line in (Path(__file__).parent/'requirements.txt').read_text().splitlines():
        if line and not line.startswith('#'):
            name, version = line.split('==')
            if versions.get(name.lower()) != version:
                parser.error('Fixture dependency does not match pin: '+line)
    manifest = dict(fixture_dependency_versions=versions, created_utc=datetime.now(timezone.utc).isoformat(),settings=settings,
                    git_head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),
                    files={name:hashlib.sha256(p.read_bytes()).hexdigest() for name,p in inputs.items()})
    dirty = subprocess.run(['git','diff','--quiet','--','scripts/soak'],cwd=root,capture_output=True).returncode
    untracked = subprocess.check_output(['git','ls-files','--others','--exclude-standard','scripts/soak'],cwd=root,text=True)
    manifest['harness_working_tree_changes'] = bool(dirty or untracked.strip())
    provenance = os.environ.get('SOAK_RELEASE_PROVENANCE')
    if provenance:
        info = json.loads(Path(provenance).read_text())
        if info['release_binary_sha256'] != manifest['files']['conduit']:
            parser.error('Binary differs from supplied release provenance')
        for name,expected in info['source_sha256'].items():
            if hashlib.sha256((root/name).read_bytes()).hexdigest() != expected:
                parser.error('Current source differs from release provenance: '+name)
        manifest['release_provenance'] = info
    with tempfile.TemporaryDirectory(prefix='conduit-soak-bundle-') as temp:
        bundle = Path(temp)/'bundle.tar.gz'
        with tarfile.open(bundle,'w:gz') as tar:
            for name,p in inputs.items():tar.add(p,arcname=name,recursive=False)
            data=json.dumps(manifest,indent=2).encode();item=tarfile.TarInfo('run-manifest.json');item.size=len(data);item.mode=0o600;tar.addfile(item,io.BytesIO(data))
        command=['ssh','-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o','UpdateHostKeys=no',
                 '-o','ConnectTimeout=12',args.host,shlex.join(['python3','-I','-c',REMOTE])]
        with bundle.open('rb') as source:
            subprocess.run(command,stdin=source,check=True)


if __name__ == '__main__':
    main()
