"""We check tag validation and archive contents without compiling a binary."""

import hashlib
import importlib.util
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile


SCRIPT = Path(__file__).resolve().parents[1] / "release.py"
SPEC = importlib.util.spec_from_file_location("release", SCRIPT)
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def test_tags_match_the_final_cargo_version(self):
        for tag in ["v0.1.0", "v0.1.0-rc.1", "v0.1.0-rc.12"]:
            release.check_tag(tag, "0.1.0")
        for tag in ["v0.2.0", "v0.2.0-rc.1", "0.1.0", "v0.1.0-rc.x",
                    "v0.1.0-rc.1-extra", "v0.1.0-beta.1"]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.check_tag(tag, "0.1.0")

    def test_smoke_requires_version_interface_extension_and_manifest(self):
        good_version = f"dutchman-electronics {release.version()}\n"
        good_info = '{"interface": 1, "extensions": ["pcbdoc"]}'
        cases = [
            (good_version, good_info, True, True),
            (good_version + "extra line\n", good_info, True, False),
            (good_version, '{"interface": 2, "extensions": ["pcbdoc"]}', True, False),
            (good_version, '{"interface": 1, "extensions": []}', True, False),
            (good_version, good_info, False, False),
        ]
        for banner, info, manifest, valid in cases:
            def read_board(args, check):
                self.assertTrue(check)
                self.assertEqual(args[-2:], ["--accept-overlay", "laminate"])
                self.assertTrue(Path(args[2]).is_file())
                if manifest:
                    (Path(args[4]) / "manifest.json").write_text("{}")

            with self.subTest(banner=banner, info=info, manifest=manifest), patch.object(
                release.subprocess, "check_output", side_effect=[banner, info]
            ), patch.object(release.subprocess, "run", side_effect=read_board):
                if valid:
                    release.smoke("binary")
                else:
                    with self.assertRaises(ValueError):
                        release.smoke("binary")

    def test_all_six_archives_have_one_folder_and_a_valid_checksum(self):
        targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu",
                   "x86_64-pc-windows-msvc", "aarch64-apple-darwin",
                   "x86_64-apple-darwin", "macos-universal"]
        for target in targets:
            for install in [False, True]:
                with self.subTest(target=target, install=install), tempfile.TemporaryDirectory() as folder:
                    root = Path(folder)
                    binary = root / "binary"
                    binary.write_bytes(b"test binary")
                    binary.chmod(0o755)
                    for name in ["README.md", "LICENSE", "LICENSE-MIT"]:
                        (root / name).write_text(name)
                    if install:
                        (root / "INSTALL.md").write_text("installation")
                    archive = release.package(binary, target, root)
                    prefix = f"dutchman-electronics-{release.version()}-{target}"
                    windows = target == "x86_64-pc-windows-msvc"
                    self.assertEqual(archive.name, prefix + (".zip" if windows else ".tar.gz"))
                    expected = {f"{prefix}/{name}" for name in [
                        "dutchman-electronics.exe" if windows else "dutchman-electronics",
                        "README.md", "LICENSE", "LICENSE-MIT",
                    ]}
                    if install:
                        expected.add(f"{prefix}/INSTALL.md")
                    if windows:
                        with zipfile.ZipFile(archive) as contents:
                            names = contents.namelist()
                            self.assertEqual(contents.read(f"{prefix}/dutchman-electronics.exe"), b"test binary")
                    else:
                        with tarfile.open(archive) as contents:
                            names = contents.getnames()
                            self.assertEqual(contents.getmember(f"{prefix}/dutchman-electronics").mode & 0o111, 0o111)
                    self.assertEqual(set(names), expected)
                    checksum = archive.with_name(archive.name + ".sha256").read_text()
                    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
                    self.assertEqual(checksum, f"{digest}  {archive.name}\n")


if __name__ == "__main__":
    unittest.main()
