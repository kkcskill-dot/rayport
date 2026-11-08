#!/usr/bin/env bash
set -euo pipefail

# Builds and packages the current native host only. Never starts a service or DB.
RAYPORT_REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
RAYPORT_SKIP_BUILD=0
case "${1:-}" in
  "") ;;
  --no-build) RAYPORT_SKIP_BUILD=1 ;;
  --help|-h)
    printf '%s\n' 'Usage: ToolsReleasePackage.sh [--no-build]' \
      'Default: locked Rust release build + npm ci/build, then native release archive.' \
      '--no-build: package existing target/release and apps/web/dist after validation.'
    exit 0 ;;
  *) printf '%s\n' 'Unknown option. Use --help.' >&2; exit 2 ;;
esac
if [[ $# -gt 1 ]]; then printf '%s\n' 'Too many arguments.' >&2; exit 2; fi

if [[ "$(uname -s)" == Darwin ]]; then
  RAYPORT_TOOLCHAIN_ENV="${RAYPORT_TOOLCHAIN_ENV:-${HOME}/.local/share/rayport/toolchains/environment.sh}"
  if [[ -f "$RAYPORT_TOOLCHAIN_ENV" ]]; then
    # This task-local file sets Node/Rust and a compatible SDK, not global profiles.
    source "$RAYPORT_TOOLCHAIN_ENV"
  fi
fi
for RAYPORT_COMMAND in cargo rustc node npm python3 file; do
  command -v "$RAYPORT_COMMAND" >/dev/null || {
    printf 'Required build tool is unavailable: %s\n' "$RAYPORT_COMMAND" >&2
    exit 1
  }
done
cd "$RAYPORT_REPOSITORY_ROOT"
RAYPORT_NATIVE_TARGET="$(rustc -vV | awk '$1 == "host:" { print $2 }')"
if [[ -z "$RAYPORT_NATIVE_TARGET" ]]; then printf '%s\n' 'Cannot identify the Rust host target.' >&2; exit 1; fi
if [[ -n "${CARGO_BUILD_TARGET:-}" && "$CARGO_BUILD_TARGET" != "$RAYPORT_NATIVE_TARGET" ]]; then
  printf '%s\n' 'This script packages native builds only. Use a runner for the required target.' >&2
  exit 1
fi
if [[ ! -f Cargo.lock || ! -f apps/web/package-lock.json ]]; then
  printf '%s\n' 'Both Cargo.lock and apps/web/package-lock.json are required.' >&2
  exit 1
fi
python3 tools/validation/ToolsValidationValidateRepository.py
if [[ "$RAYPORT_SKIP_BUILD" == 0 ]]; then
  cargo build --release -p rayport-server --locked
  npm --prefix apps/web ci
  npm --prefix apps/web run build
fi
python3 - "$RAYPORT_REPOSITORY_ROOT" "$RAYPORT_NATIVE_TARGET" "$RAYPORT_SKIP_BUILD" <<'PY'
from pathlib import Path
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time

root = Path(sys.argv[1]).resolve()
target = sys.argv[2]
prebuilt = sys.argv[3] == '1'
manifest = (root / 'Cargo.toml').read_text()
workspace_package = manifest.split('[workspace.package]', 1)[1].split('\n[', 1)[0]
match = re.search(r'^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"', workspace_package, re.M)
if not match:
    raise SystemExit('A stable numeric workspace version is required for packaging.')
version = match.group(1)
web_package = json.loads((root / 'apps/web/package.json').read_text())
if web_package['version'] != version:
    raise SystemExit('Rust and web package versions do not match.')
binary = root / 'target/release/rayport-server'
web = root / 'apps/web/dist'
if not binary.is_file() or binary.is_symlink() or not os.access(binary, os.X_OK):
    raise SystemExit('Native release binary is missing. Run the release build first.')
if not (web / 'index.html').is_file():
    raise SystemExit('Web production build is missing.')
binary_format = subprocess.check_output(['file', '-b', str(binary)], text=True).strip()
if ('apple-darwin' in target and 'Mach-O' not in binary_format) or ('linux' in target and 'ELF' not in binary_format):
    raise SystemExit('The release binary format does not match the claimed native target.')
if target.startswith('aarch64') and not any(word in binary_format for word in ('arm64', 'aarch64', 'ARM aarch64')):
    raise SystemExit('The release binary architecture does not match aarch64.')
if target.startswith('x86_64') and not any(word in binary_format for word in ('x86_64', 'x86-64')):
    raise SystemExit('The release binary architecture does not match x86_64.')
if not any(platform in target for platform in ('apple-darwin', 'linux')):
    raise SystemExit('Only tested Unix packaging layouts are supported.')
epoch = int(os.environ.get('SOURCE_DATE_EPOCH', str(int(time.time()))))
if epoch < 0:
    raise SystemExit('SOURCE_DATE_EPOCH must be nonnegative.')
name = 'rayport-' + version + '-' + target
distribution = root / 'dist'
distribution.mkdir(exist_ok=True)
stage = distribution / name
if stage.is_symlink():
    raise SystemExit('Refusing to replace a symlink at the output directory.')
temporary = Path(tempfile.mkdtemp(prefix='.rayport-package-', dir=distribution))

def copy_file(source, destination):
    if source.is_symlink() or not source.is_file():
        raise RuntimeError('Package inputs must be regular files, not symlinks.')
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    destination.chmod(0o755 if source == binary or source.name == 'ToolsReleaseInstall.sh' else 0o644)

def copy_tree(source, destination, extensions=None):
    if source.is_symlink():
        raise RuntimeError('Package input directories must not be symlinks.')
    for path in sorted(source.rglob('*')):
        if path.is_symlink():
            raise RuntimeError('Symlink detected in package input.')
        if path.is_file() and (extensions is None or path.suffix in extensions):
            copy_file(path, destination / path.relative_to(source))

try:
    copy_file(binary, temporary / 'rayport-server')
    copy_tree(web, temporary / 'web')
    copy_tree(root / 'docs', temporary / 'docs', {'.md', '.html'})
    copy_tree(root / 'contracts', temporary / 'contracts', {'.json', '.yaml', '.yml'})
    copy_file(root / 'README.md', temporary / 'README.md')
    copy_file(root / 'tools/release/ToolsReleaseInstall.sh', temporary / 'ToolsReleaseInstall.sh')
    revision = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=root, text=True, capture_output=True)
    metadata = {
        'product': 'rayport', 'version': version, 'target': target,
        'packaged_at': datetime.fromtimestamp(epoch, timezone.utc).isoformat(),
        'source_revision': revision.stdout.strip() if revision.returncode == 0 else 'uncommitted',
        'prebuilt_artifacts': prebuilt, 'binary_format': binary_format,
        'packaging_toolchain': {tool: subprocess.check_output([tool, '--version'], text=True).strip()
                                for tool in ('rustc', 'cargo', 'node', 'npm')},
        'storage_schema': 1,
        'included_runtime_data': False,
    }
    (temporary / 'manifest.json').write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + '\n')
    files = sorted(path for path in temporary.rglob('*') if path.is_file())
    checksums = []
    for path in files:
        relative = path.relative_to(temporary).as_posix()
        if any(part.startswith('.') for part in path.relative_to(temporary).parts):
            raise RuntimeError('Hidden files are not permitted in release packages.')
        if path.suffix in {'.pem', '.key', '.p12', '.pfx'} or path.name in {'config.json', 'initial-admin-token.txt'}:
            raise RuntimeError('Runtime configuration or credential artifact found in package input.')
        checksums.append(hashlib.sha256(path.read_bytes()).hexdigest() + '  ' + relative)
    (temporary / 'SHA256SUMS').write_text('\n'.join(checksums) + '\n')
    archive_temporary = distribution / ('.' + name + '.tar.gz.tmp')
    with archive_temporary.open('wb') as output:
        with gzip.GzipFile(filename='', mode='wb', fileobj=output, mtime=epoch) as compressed:
            with tarfile.open(fileobj=compressed, mode='w') as archive:
                for path in [temporary] + sorted(temporary.rglob('*')):
                    relative = path.relative_to(temporary)
                    archive_name = name if str(relative) == '.' else name + '/' + relative.as_posix()
                    info = archive.gettarinfo(str(path), arcname=archive_name)
                    info.uid = info.gid = 0
                    info.uname = info.gname = 'root'
                    info.mtime = epoch
                    if path.is_file():
                        with path.open('rb') as content:
                            archive.addfile(info, content)
                    else:
                        archive.addfile(info)
    if stage.exists():
        shutil.rmtree(stage)
    temporary.rename(stage)
    archive_path = distribution / (name + '.tar.gz')
    os.replace(archive_temporary, archive_path)
    archive_digest = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    checksum_path = distribution / (name + '.tar.gz.sha256')
    checksum_path.write_text(archive_digest + '  ' + archive_path.name + '\n')
    print('Release directory: ' + str(stage))
    print('Release archive:   ' + str(archive_path))
    print('Archive checksum:  ' + str(checksum_path))
    print('Native target:     ' + target)
    print('No runtime configuration, data, tokens, or services were included or started.')
finally:
    if temporary.exists():
        shutil.rmtree(temporary)
PY
