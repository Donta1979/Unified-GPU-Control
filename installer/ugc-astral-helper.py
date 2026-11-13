#!/usr/bin/env python3
"""Root-owned fixed-action helper; safe verification-only reinstall until isolated recovery tests."""
import os, sys, subprocess, pathlib, shutil, tempfile, datetime, stat
BASE=pathlib.Path('/var/usrlocal/lib/astral-guard-manager')
PAYLOAD=BASE/'payload'
MODULE=pathlib.Path('/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko')
GUARD=pathlib.Path('/var/usrlocal/bin/astral-guard')
UNITS=pathlib.Path('/etc/systemd/system')
NAMES=['astral-hwmon.service','astral-guard.service','astral-guard.timer','astral-guard-poweroff.service']
def cmd(*args,cwd=None,timeout=150):
 p=subprocess.run(args,cwd=cwd,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout)
 if p.returncode: raise RuntimeError('Command failed: '+ ' '.join(map(str,args))+'\n'+p.stdout[-1800:])
 return p.stdout

def validate_payload():
 # Root helpers must never compile user-writable code.
 for path in (BASE, PAYLOAD, *PAYLOAD.rglob('*')):
  info=path.lstat()
  if stat.S_ISLNK(info.st_mode):
   raise RuntimeError('Unsafe symbolic link in repair payload: '+str(path))
  if info.st_uid != 0 or (info.st_mode & 0o022):
   raise RuntimeError('Repair payload is not root-owned and read-only to non-root: '+str(path))
 for name in NAMES:
  if not (PAYLOAD/'units'/name).is_file():
   raise RuntimeError('Missing service unit: '+name)


def sensor_channels_present():
 """Read-only check of the 12 expected Astral hardware channels."""
 for hwmon in pathlib.Path('/sys/class/hwmon').glob('hwmon*'):
  try:
   if (hwmon/'name').read_text().strip() != 'astral12vhpwr':
    continue
   currents = sorted(hwmon.glob('curr*_input'))
   voltages = sorted(hwmon.glob('in*_input'))
   if len(currents) != 6 or len(voltages) != 6:
    return False
   values = [int(channel.read_text().strip()) for channel in currents + voltages]
   return all(0 <= v <= 30000 for v in values[:6]) and all(10500 <= v <= 13500 for v in values[6:])
  except (OSError, ValueError):
   return False
 return False

def service_property(unit, prop):
 return cmd('systemctl', 'show', unit, '-p', prop, '--value', '--no-pager').strip()

def verify_running_protection():
 """Read-only checks: NEVER invoke guard or start the systemd guard service."""
 expected = {
  'astral-guard.service': (
   'OnFailure=astral-guard-poweroff.service',
   'SuccessExitStatus=1 3',
   'ExecStart=/var/usrlocal/bin/astral-guard --quiet',
  ),
  'astral-guard-poweroff.service': (
   'ExecStartPre=/bin/sh -c \'test "$MONITOR_EXIT_STATUS" = 2\'',
   'ExecStart=/usr/bin/systemctl poweroff',
  ),
  'astral-guard.timer': (
   'OnBootSec=10s', 'OnUnitInactiveSec=5s', 'AccuracySec=1s',
  ),
 }
 for unit, directives in expected.items():
  content = (UNITS/unit).read_text()
  effective_lines = {line.strip() for line in content.splitlines()
                     if line.strip() and not line.lstrip().startswith(('#', ';'))}
  for directive in directives:
   if directive not in effective_lines:
    raise RuntimeError('Required directive missing in '+unit+': '+directive)
 # Verify the displayed warning and actual shutdown countdown agree.
 poweroff_lines = [
  line.strip()
  for line in (UNITS/'astral-guard-poweroff.service').read_text().splitlines()
  if line.strip() and not line.lstrip().startswith(('#', ';'))
 ]
 sleep_lines = [
  line for line in poweroff_lines
  if line.startswith('ExecStartPre=/usr/bin/sleep ')
 ]
 wall_lines = [
  line for line in poweroff_lines
  if line.startswith('ExecStartPre=/usr/bin/wall ')
 ]

 if len(sleep_lines) != 1 or len(wall_lines) != 1:
  raise RuntimeError('Shutdown countdown configuration is ambiguous')

 seconds = sleep_lines[0].split()[-1]
 if seconds not in ('5', '10', '15', '20', '30'):
  raise RuntimeError('Unsupported emergency shutdown countdown')

 if f'Powering off in {seconds} seconds.' not in wall_lines[0]:
  raise RuntimeError('Shutdown warning and actual countdown disagree')

 if service_property('astral-hwmon.service', 'ActiveState') != 'active':
  raise RuntimeError('Astral driver service not active')
 if service_property('astral-hwmon.service', 'UnitFileState') != 'enabled':
  raise RuntimeError('Astral driver service not enabled')
 if service_property('astral-guard.timer', 'ActiveState') != 'active':
  raise RuntimeError('Astral monitoring timer not active')
 if service_property('astral-guard.timer', 'UnitFileState') != 'enabled':
  raise RuntimeError('Astral monitoring timer not enabled')
 if service_property('astral-guard.service', 'Result') != 'success':
  raise RuntimeError('Last systemd guard execution was not successful')
 if service_property('astral-guard.service', 'ExecMainStatus') not in ('0', '1', '3'):
  raise RuntimeError('Last systemd guard exit status was not accepted')
 if service_property('astral-guard-poweroff.service', 'ActiveState') != 'inactive':
  raise RuntimeError('Emergency poweroff service is not inactive')
 if not pathlib.Path('/sys/module/astral_hwmon').exists():
  raise RuntimeError('Astral kernel module not loaded')
 if not sensor_channels_present():
  raise RuntimeError('Astral sensor channels missing or voltage outside validation range')

def install():
 """Safe existing-install verification; destructive recovery requires separate testing."""
 if os.geteuid() != 0:
  raise RuntimeError('Root privileges required')
 validate_payload()
 existing = [MODULE, GUARD, *(UNITS/name for name in NAMES)]
 if not all(path.is_file() and not path.is_symlink() for path in existing):
  raise RuntimeError(
   'Astral Guard is missing files or partially installed. Automatic repair is '
   'disabled until fresh-install and rollback have been tested in isolation. '
   'No protection was changed.'
  )
 try:
  verify_running_protection()
  version_output = cmd('modinfo', '-F', 'vermagic', str(MODULE)).split()
  if not version_output or version_output[0] != os.uname().release:
   raise RuntimeError('Installed driver does not match running kernel')
 except (RuntimeError, OSError, subprocess.SubprocessError) as error:
  raise RuntimeError('Safety verification failed; no changes made: '+str(error)) from error
 print('Astral Guard already operational. No reinstall necessary; no changes made.')

def configure(seconds):
 if os.geteuid()!=0: raise RuntimeError('Root privileges required')
 if seconds not in ('5','10','15','20','30'): raise RuntimeError('Unsupported countdown')
 # Confirm existing critical-only guard logic before changing either service.
 guard=(UNITS/'astral-guard.service').read_text()
 old=(UNITS/'astral-guard-poweroff.service').read_text()
 if 'OnFailure=astral-guard-poweroff.service' not in guard or 'SuccessExitStatus=1 3' not in guard:
  raise RuntimeError('Existing guard interlock differs; refusing changes')
 if 'MONITOR_EXIT_STATUS' not in old or '/usr/bin/systemctl poweroff' not in old:
  raise RuntimeError('Existing shutdown interlock differs; refusing changes')
 # A short interval between guard completions; avoids overlapping long-running checks.
 timer=UNITS/'astral-guard.timer'
 if not timer.is_file(): raise RuntimeError('Guard timer missing')
 oldtimer=timer.read_text()
 if '[Timer]' not in oldtimer or 'OnBootSec=' not in oldtimer: raise RuntimeError('Unexpected timer unit')
 backup=BASE/'backups'/datetime.datetime.now().strftime('%Y%m%d-%H%M%S-%f')
 backup.mkdir(parents=True,exist_ok=False)
 shutil.copy2(UNITS/'astral-guard-poweroff.service',backup/'astral-guard-poweroff.service')
 shutil.copy2(timer,backup/'astral-guard.timer')
 unit='''[Unit]
Description=Power off after a CRITICAL 12VHPWR verdict

[Service]
Type=oneshot
ExecStartPre=/bin/sh -c 'test "$MONITOR_EXIT_STATUS" = 2'
ExecStartPre=/usr/bin/wall "ASTRAL GUARD CRITICAL: 12VHPWR safety limit exceeded. Powering off in {seconds} seconds."
ExecStartPre=/usr/bin/sleep {seconds}
ExecStart=/usr/bin/systemctl poweroff
'''.format(seconds=seconds)
 newtimer='''[Unit]
Description=Check the 12VHPWR connector periodically

[Timer]
OnBootSec=10s
OnUnitInactiveSec=5s
AccuracySec=1s

[Install]
WantedBy=timers.target
'''
 def atomic(dest,content,mode=0o644):
  if dest.is_symlink():
   raise RuntimeError('Refusing to replace symbolic link: '+str(dest))
  fd, staged_name = tempfile.mkstemp(
   prefix='.'+dest.name+'.', suffix='.tmp', dir=str(dest.parent)
  )
  staged = pathlib.Path(staged_name)
  try:
   with os.fdopen(fd,'w',encoding='utf-8') as stream:
    stream.write(content)
    stream.flush()
    os.fsync(stream.fileno())
   os.chmod(staged,mode)
   os.replace(staged,dest)
   dirfd=os.open(str(dest.parent),os.O_RDONLY|getattr(os,'O_DIRECTORY',0))
   try:
    os.fsync(dirfd)
   finally:
    os.close(dirfd)
  finally:
   staged.unlink(missing_ok=True)

 try:
  atomic(UNITS/'astral-guard-poweroff.service',unit)
  atomic(timer,newtimer)
  cmd('systemctl','daemon-reload')
  cmd('systemctl','restart','astral-guard.timer')
  cmd('systemctl','enable','astral-guard.timer')
 except Exception as original_error:
  rollback_errors=[]
  for dest,saved in (
   (UNITS/'astral-guard-poweroff.service',backup/'astral-guard-poweroff.service'),
   (timer,backup/'astral-guard.timer'),
  ):
   try:
    atomic(dest,saved.read_text(),stat.S_IMODE(saved.stat().st_mode))
   except Exception as rollback_error:
    rollback_errors.append(str(rollback_error))

  for args in (
   ['systemctl','daemon-reload'],
   ['systemctl','restart','astral-guard.timer'],
  ):
   result=subprocess.run(args,text=True,stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT)
   if result.returncode:
    rollback_errors.append(' '.join(args)+': '+result.stdout[-500:])

  if rollback_errors:
   raise RuntimeError(
    'Configuration failed and rollback reported problems: '
    + '; '.join(rollback_errors)
   ) from original_error
  raise
 print('Configured '+seconds+'-second CRITICAL countdown and 5-second post-check interval. Backup: '+str(backup))


def check_install_state():
    """Read-only classification. Exit 2 on unsafe/partial installations."""
    paths = [MODULE, GUARD, *(UNITS/name for name in NAMES)]
    present = [p for p in paths if p.exists() or p.is_symlink()]
    module_loaded = pathlib.Path('/sys/module/astral_hwmon').exists()

    if not present and not module_loaded:
        print("FRESH: No installed Astral files or loaded module detected.")
        return 0

    if len(present) != len(paths):
        print("BLOCKED: Partial installation or loaded module without complete files. No changes made.")
        return 2

    if any(p.is_symlink() or not p.is_file() for p in paths):
        print("BLOCKED: Unexpected installation files detected. No changes made.")
        return 2

    print("EXISTING: Astral files detected. No changes made.")
    return 0

def fresh_install_action():
    """Fresh hosts only. Import reviewed code from a root-owned, immutable deployment."""
    if os.geteuid() != 0:
        raise RuntimeError('Root privileges required')
    import importlib.util
    module_path = BASE / 'ugc-astral-fresh-install.py'
    # Do not import Python from a user-writable location as root.
    for path in (BASE, module_path):
        info = path.lstat()
        if info.st_uid != 0 or (info.st_mode & 0o022) or stat.S_ISLNK(info.st_mode):
            raise RuntimeError('Unsafe fresh installer source: '+str(path))
    if not stat.S_ISREG(module_path.stat().st_mode) or module_path.stat().st_nlink != 1:
        raise RuntimeError('Unsafe fresh installer file')
    spec = importlib.util.spec_from_file_location('ugc_astral_fresh_install', module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError('Unable to load trusted fresh installer')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    print(module.fresh_install())

def main():
 if len(sys.argv)==2 and sys.argv[1]=='check-install-state': sys.exit(check_install_state())
 elif len(sys.argv)==2 and sys.argv[1]=='reinstall': install()
 elif len(sys.argv)==2 and sys.argv[1]=='fresh-install': fresh_install_action()
 elif len(sys.argv)==3 and sys.argv[1]=='configure': configure(sys.argv[2])
 else: raise RuntimeError('Supported actions: check-install-state, reinstall, fresh-install, configure [5|10|15|20|30]')
if __name__=='__main__':
 try: main()
 except Exception as e: print('ERROR: '+str(e),file=sys.stderr); sys.exit(1)
