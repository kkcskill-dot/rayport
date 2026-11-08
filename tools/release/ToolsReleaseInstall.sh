#!/usr/bin/env bash
set -euo pipefail

# Copies verified releases and switches current/previous. Never starts/stops a service.
if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
  printf '%s\n' 'Usage: ToolsReleaseInstall.sh PACKAGE_DIRECTORY_OR_TAR_GZ INSTALL_DIRECTORY [--service-stopped]' \
    'Existing installations require the operator to stop the service first and pass --service-stopped.' \
    'Data and configuration under instance/ are preserved. No service commands are run.'
  exit 0
fi
if [[ $# -lt 2 || $# -gt 3 || ( $# == 3 && "$3" != --service-stopped ) ]]; then
  printf '%s\n' 'Invalid arguments. Use --help.' >&2
  exit 2
fi
command -v python3 >/dev/null || { printf '%s\n' 'python3 is required.' >&2; exit 1; }
python3 - "$1" "$2" "${3:-}" <<'PY'
from pathlib import Path, PurePosixPath
import hashlib
import fcntl
import json
import os
import platform
import re
import shutil
import stat
import sys
import tarfile
import tempfile
import uuid

source = Path(sys.argv[1]).expanduser().resolve()
destination_arg = Path(sys.argv[2]).expanduser().absolute()
stopped = sys.argv[3] == '--service-stopped'
if destination_arg.is_symlink():
    raise SystemExit('Installation directory must not be a symlink.')
destination = destination_arg.resolve()
if source == destination or source == destination / 'instance':
    raise SystemExit('Package input and installation state must be separate.')

def safe_relative(name):
    path = PurePosixPath(name)
    if not name or path.is_absolute() or '..' in path.parts or '\\' in name or '\n' in name or '\r' in name:
        raise ValueError('Unsafe path in release package.')
    return path

def verify(package):
    if not package.is_dir() or package.is_symlink():
        raise ValueError('Release package must be a directory of regular files.')
    for path in package.rglob('*'):
        if path.is_symlink() or not (path.is_dir() or path.is_file()):
            raise ValueError('Symlinks and special files are not permitted inside a release.')
    sums = package / 'SHA256SUMS'
    if not sums.is_file():
        raise ValueError('Package SHA256SUMS is missing.')
    expected = {}
    for line in sums.read_text().splitlines():
        match = re.fullmatch(r'([a-f0-9]{64})  (.+)', line)
        if not match:
            raise ValueError('Malformed SHA256SUMS.')
        digest, name = match.groups()
        relative = safe_relative(name)
        if name in expected or name == 'SHA256SUMS':
            raise ValueError('Duplicate or invalid checksum entry.')
        path = package.joinpath(*relative.parts)
        if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise ValueError('Release checksum verification failed.')
        expected[name] = digest
    actual = {path.relative_to(package).as_posix() for path in package.rglob('*') if path.is_file() and path != sums}
    if actual != set(expected):
        raise ValueError('Release contains missing or unlisted files.')
    required = {'rayport-server', 'web/index.html', 'manifest.json', 'ToolsReleaseInstall.sh',
                'docs/delivery/DocsDeliveryRayportV1Operations.md'}
    if not required.issubset(actual):
        raise ValueError('Release is incomplete.')
    if not os.access(package / 'rayport-server', os.X_OK):
        raise ValueError('Release server binary is not executable.')
    metadata = json.loads((package / 'manifest.json').read_text())
    if metadata.get('product') != 'rayport' or not re.fullmatch(r'\d+\.\d+\.\d+', metadata.get('version', '')):
        raise ValueError('Invalid release manifest.')
    target = metadata.get('target', '')
    if not re.fullmatch(r'[a-z0-9_]+(?:-[a-z0-9_]+)+', target):
        raise ValueError('Invalid release target.')
    system = platform.system()
    machine = platform.machine().lower()
    expected_arch = 'aarch64' if machine in {'arm64', 'aarch64'} else 'x86_64' if machine in {'x86_64', 'amd64'} else None
    if not expected_arch or not target.startswith(expected_arch + '-'):
        raise ValueError('Release architecture does not match this machine.')
    if (system == 'Darwin' and 'apple-darwin' not in target) or (system == 'Linux' and 'linux' not in target) or system not in {'Darwin', 'Linux'}:
        raise ValueError('Release operating system does not match this machine.')
    release_id = 'rayport-' + metadata['version'] + '-' + target + '-' + hashlib.sha256(sums.read_bytes()).hexdigest()[:12]
    return release_id

def managed_link(path):
    if not path.exists() and not path.is_symlink():
        return None
    if not path.is_symlink():
        raise ValueError('Current and previous release pointers must be managed symlinks.')
    target = os.readlink(path)
    relative = safe_relative(target)
    if len(relative.parts) != 2 or relative.parts[0] != 'releases':
        raise ValueError('Release pointer escapes the managed releases directory.')
    if not (destination / target).is_dir():
        raise ValueError('Managed release pointer references a missing release.')
    return target

def replace_link(path, target):
    temporary = path.parent / ('.' + path.name + '-' + uuid.uuid4().hex)
    try:
        os.symlink(target, temporary)
        os.replace(temporary, path)
    finally:
        if temporary.is_symlink():
            temporary.unlink()

with tempfile.TemporaryDirectory(prefix='rayport-install-') as temporary_directory:
    extracted = Path(temporary_directory)
    if source.is_file():
        count = 0
        size = 0
        with tarfile.open(source, 'r:gz') as archive:
            for member in archive:
                relative = safe_relative(member.name)
                count += 1
                size += member.size
                if count > 50000 or size > 1024 * 1024 * 1024 or not (member.isdir() or member.isfile()):
                    raise ValueError('Release archive contains unsupported entries or exceeds limits.')
                path = extracted.joinpath(*relative.parts)
                if member.isdir():
                    path.mkdir(parents=True, exist_ok=True)
                else:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    with path.open('xb') as output, archive.extractfile(member) as content:
                        shutil.copyfileobj(content, output)
                    path.chmod(member.mode & 0o777)
        roots = list(extracted.iterdir())
        if len(roots) != 1 or not roots[0].is_dir():
            raise ValueError('Release archive must contain one top-level directory.')
        package = roots[0]
    else:
        package = source
    release_id = verify(package)
    destination.mkdir(parents=True, exist_ok=True)
    install_lock = os.open(destination / '.install.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    lock_metadata = os.fstat(install_lock)
    if not stat.S_ISREG(lock_metadata.st_mode) or lock_metadata.st_nlink != 1:
        raise ValueError('Installation lock must be a private regular file.')
    os.fchmod(install_lock, 0o600)
    try:
        fcntl.flock(install_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        raise SystemExit('Another installation operation is already running.')
    # The descriptor intentionally owns the lock until this script exits.
    if (destination / 'releases').is_symlink() or (destination / 'instance').is_symlink():
        raise ValueError('Release and instance directories must not be symlinks.')
    current = managed_link(destination / 'current')
    managed_link(destination / 'previous')
    if current and not stopped:
        raise SystemExit('Stop the running service first, then repeat with --service-stopped. No changes were activated.')
    instance = destination / 'instance'
    instance.mkdir(mode=0o700, exist_ok=True)
    web_link = instance / 'web'
    if web_link.exists() or web_link.is_symlink():
        if not web_link.is_symlink() or os.readlink(web_link) != '../current/web':
            raise ValueError('Existing instance/web is not the managed web link; migrate it explicitly before updating.')
    releases = destination / 'releases'
    releases.mkdir(exist_ok=True)
    release = releases / release_id
    if release.exists():
        if verify(release) != release_id:
            raise ValueError('Existing installed release differs from the package.')
    else:
        stage = releases / ('.incoming-' + uuid.uuid4().hex)
        try:
            shutil.copytree(package, stage)
            verify(stage)
            for path in stage.rglob('*'):
                if path.is_file():
                    with path.open('rb') as handle:
                        os.fsync(handle.fileno())
            os.replace(stage, release)
        finally:
            if stage.exists():
                shutil.rmtree(stage)
    new_target = 'releases/' + release_id
    if current and current != new_target:
        replace_link(destination / 'previous', current)
    replace_link(destination / 'current', new_target)
    if not web_link.is_symlink():
        os.symlink('../current/web', web_link)
    directory_handle = os.open(destination, os.O_RDONLY)
    try:
        os.fsync(directory_handle)
    finally:
        os.close(directory_handle)
    print('Installed release: ' + str(release))
    print('Current pointer:   ' + str(destination / 'current'))
    print('Instance preserved: ' + str(instance))
    print('No service was started, stopped, enabled, or registered.')
    print('Next: follow docs/delivery/DocsDeliveryRayportV1Operations.md for init/check and explicit startup.')
PY
