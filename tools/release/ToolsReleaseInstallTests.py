"""Installer regression tests using synthetic files, without services or databases."""
from pathlib import Path
import ast
import fcntl
import hashlib
import io
import json
import os
import platform
import subprocess
import tarfile
import tempfile
import unittest


RELEASE_TOOLS = Path(__file__).resolve().parent
INSTALLER = RELEASE_TOOLS / "ToolsReleaseInstall.sh"


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="rayport install test ")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.destination = self.base / "installation"
        machine = platform.machine().lower()
        architecture = "aarch64" if machine in {"arm64", "aarch64"} else "x86_64"
        suffix = "apple-darwin" if platform.system() == "Darwin" else "unknown-linux-gnu"
        self.target = architecture + "-" + suffix
        self.first = self.fixture("first")

    def fixture(self, name, version="1.0.0", target=None):
        package = self.base / name
        package.mkdir()
        (package / "web").mkdir()
        (package / "docs/delivery").mkdir(parents=True)
        binary = package / "rayport-server"
        binary.write_text("#!/bin/sh\nexit 0\n")
        binary.chmod(0o755)
        (package / "web/index.html").write_text("<p>" + version + "</p>")
        (package / "ToolsReleaseInstall.sh").write_text(INSTALLER.read_text())
        (package / "docs/delivery/DocsDeliveryRayportV1Operations.md").write_text("Synthetic fixture only")
        (package / "manifest.json").write_text(json.dumps({
            "product": "rayport", "version": version, "target": target or self.target,
        }))
        checksums = "".join(
            hashlib.sha256(path.read_bytes()).hexdigest() + "  " + path.relative_to(package).as_posix() + "\n"
            for path in sorted(package.rglob("*")) if path.is_file()
        )
        (package / "SHA256SUMS").write_text(checksums)
        return package

    def install(self, package, stopped=False):
        return subprocess.run(
            ["bash", str(INSTALLER), str(package), str(self.destination)]
            + (["--service-stopped"] if stopped else []),
            capture_output=True, text=True, check=False,
        )

    def assert_success(self, result):
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_shell_and_embedded_python_syntax(self):
        for name in ("ToolsReleaseInstall.sh", "ToolsReleasePackage.sh"):
            script = RELEASE_TOOLS / name
            subprocess.run(["bash", "-n", str(script)], check=True)
            source = script.read_text().split("<<'PY'\n", 1)[1].rsplit("\nPY", 1)[0]
            ast.parse(source)

    def test_initial_install_creates_managed_web_link_without_running_binary(self):
        self.assert_success(self.install(self.first))
        self.assertEqual(os.readlink(self.destination / "instance/web"), "../current/web")
        self.assertTrue((self.destination / "current/rayport-server").is_file())
        self.assertFalse((self.destination / "instance/config.json").exists())

    def test_upgrade_requires_stopped_flag_and_preserves_configuration_and_data(self):
        self.assert_success(self.install(self.first))
        old = os.readlink(self.destination / "current")
        instance = self.destination / "instance"
        (instance / "config.json").write_text("preserve configuration")
        (instance / "data").mkdir()
        (instance / "data/snapshot.json").write_text("preserve data")
        second = self.fixture("second", "1.0.1")
        self.assertNotEqual(self.install(second).returncode, 0)
        self.assertEqual(os.readlink(self.destination / "current"), old)
        self.assert_success(self.install(second, stopped=True))
        self.assertEqual(os.readlink(self.destination / "previous"), old)
        self.assertNotEqual(os.readlink(self.destination / "current"), old)
        self.assertEqual((instance / "config.json").read_text(), "preserve configuration")
        self.assertEqual((instance / "data/snapshot.json").read_text(), "preserve data")

    def test_rollback_swaps_release_pointers(self):
        self.assert_success(self.install(self.first))
        old = os.readlink(self.destination / "current")
        second = self.fixture("second", "1.0.1")
        self.assert_success(self.install(second, stopped=True))
        new = os.readlink(self.destination / "current")
        self.assert_success(self.install(self.destination / "previous", stopped=True))
        self.assertEqual(os.readlink(self.destination / "current"), old)
        self.assertEqual(os.readlink(self.destination / "previous"), new)

    def test_tampered_or_unlisted_files_are_rejected(self):
        (self.first / "web/index.html").write_text("tampered")
        self.assertNotEqual(self.install(self.first).returncode, 0)
        other = self.fixture("other")
        (other / "unlisted.txt").write_text("unexpected")
        self.assertNotEqual(self.install(other).returncode, 0)
        self.assertFalse((self.destination / "current").exists())

    def test_archive_installation_preserves_executable_and_activates(self):
        archive = self.base / "valid.tar.gz"
        with tarfile.open(archive, "w:gz") as output:
            output.add(self.first, arcname="rayport-test")
        self.assert_success(self.install(archive))
        self.assertTrue(os.access(self.destination / "current/rayport-server", os.X_OK))

    def test_path_traversal_and_archive_symlink_are_rejected(self):
        traversal = self.base / "traversal.tar.gz"
        with tarfile.open(traversal, "w:gz") as output:
            entry = tarfile.TarInfo("../escaped")
            entry.size = 3
            output.addfile(entry, io.BytesIO(b"bad"))
        self.assertNotEqual(self.install(traversal).returncode, 0)
        linked = self.base / "linked.tar.gz"
        with tarfile.open(linked, "w:gz") as output:
            entry = tarfile.TarInfo("package/link")
            entry.type = tarfile.SYMTYPE
            entry.linkname = "/etc/passwd"
            output.addfile(entry)
        self.assertNotEqual(self.install(linked).returncode, 0)
        self.assertFalse((self.destination / "current").exists())

    def test_internal_symlink_and_wrong_target_are_rejected(self):
        (self.first / "web/link").symlink_to(self.base)
        self.assertNotEqual(self.install(self.first).returncode, 0)
        wrong = self.fixture("wrong", target="unsupported-unknown-linux-gnu")
        self.assertNotEqual(self.install(wrong).returncode, 0)

    def test_existing_unmanaged_web_directory_is_not_deleted(self):
        unmanaged = self.destination / "instance/web"
        unmanaged.mkdir(parents=True)
        (unmanaged / "custom.html").write_text("preserve custom files")
        self.assertNotEqual(self.install(self.first).returncode, 0)
        self.assertEqual((unmanaged / "custom.html").read_text(), "preserve custom files")
        self.assertFalse((self.destination / "current").exists())

    def test_concurrent_installer_lock_rejects_second_operation(self):
        self.destination.mkdir()
        with (self.destination / ".install.lock").open("w") as lock:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = self.install(self.first)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("already running", result.stderr)
        self.assert_success(self.install(self.first))


if __name__ == "__main__":
    unittest.main(verbosity=2)
