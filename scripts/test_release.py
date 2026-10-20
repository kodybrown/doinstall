"""Release boundary tests: malformed inputs must fail before publication."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import warnings
import zipfile

import collect_release
import release


class ReleaseTests(unittest.TestCase):
    def test_version_cli_supports_exact_task_capture(self):
        command = [sys.executable, "-B", str(release.ROOT / "scripts/release.py"), "version"]
        expected = release.version().encode()
        self.assertEqual(subprocess.check_output(command).strip(), expected)
        self.assertEqual(subprocess.check_output(command + ["--no-newline"]), expected)

    def test_tag_requires_exact_cargo_version(self):
        expected = release.version()
        self.assertEqual(release.version("v" + expected), expected)
        for bad in (expected, "v9.9.9", "v" + expected + "-preview", "v01.0.0", "v0.1.2610.1900"):
            with self.subTest(tag=bad), self.assertRaises(ValueError):
                release.version(bad)

    def test_scan_rejects_paths_and_secrets_in_both_encodings(self):
        for text in ("C:\\Users\\private\\project", "/home/private/project",
                     "github_pat_" + "A" * 60, "-----BEGIN OPENSSH PRIVATE KEY-----",
                     "GITHUB_TOKEN=private-value", "/custom/build/private.rs"):
            for encoding in ("utf-8", "utf-16le"):
                with self.subTest(text=text, encoding=encoding), self.assertRaises(ValueError):
                    release.scan(text.encode(encoding), "fixture", {"/custom/build"})
        release.scan(b"/doinstall/src/main.rs /cargo/registry /rustc/public", "public", set())

    def test_archive_rejects_traversal_and_duplicates(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tools.zip"
            for name in ("../escape", "/escape", "a\\escape", "a/./escape", "C:/escape"):
                with zipfile.ZipFile(path, "w") as output:
                    entry = zipfile.ZipInfo("placeholder")
                    entry.filename = name
                    output.writestr(entry, "data")
                with self.assertRaises(ValueError):
                    release.archive_entries(path)
            with warnings.catch_warnings():
                warnings.simplefilter("ignore", UserWarning)
                with zipfile.ZipFile(path, "w") as output:
                    output.writestr("same", "first")
                    output.writestr("same", "second")
            with self.assertRaises(ValueError):
                release.archive_entries(path)
            with zipfile.ZipFile(path, "w") as output:
                link = zipfile.ZipInfo("link")
                link.external_attr = 0o120777 << 16
                output.writestr(link, "target")
            with self.assertRaises(ValueError):
                release.archive_entries(path)

    def test_tar_roundtrip_preserves_executable_modes(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            stage = directory / "stage"
            stage.mkdir()
            (stage / "doinstall").write_bytes(b"native fixture")
            (stage / "doinstall").chmod(0o755)
            archive = directory / "tools.tar.gz"
            release.archive_tools(stage, archive, False)
            entries = release.archive_entries(archive)
            self.assertEqual(entries[0][:2], ("doinstall", b"native fixture"))
            if release.os.name != "nt":
                self.assertEqual(entries[0][2] & 0o777, 0o755)

    def test_collection_refuses_missing_tampered_and_dirty_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            value = release.version()
            for system, archive, suffix in (("windows", "zip", ".exe"), ("linux", "tar.gz", "")):
                directory = root / ("native-" + system)
                directory.mkdir()
                stem = f"doinstall-v{value}-{system}-x64"
                (directory / (stem + "-tools." + archive)).write_bytes(b"archive fixture")
                (directory / (stem + "-setup" + suffix)).write_bytes(b"installer fixture")
                release.write_json(directory / (stem + "-metadata.json"), {
                    "version": value, "tag": "v" + value, "source_commit": "a" * 40,
                    "source_dirty": False, "platform": system, "architecture": "x64"})
                (directory / "SHA256SUMS").write_text("".join(
                    f"{release.sha(p.read_bytes())}  {p.name}\n" for p in sorted(directory.iterdir())))
            collect_release.collect(root, root / "good", "v" + value, "a" * 40, True)
            sums = (root / "good" / "SHA256SUMS").read_text().splitlines()
            self.assertEqual(len(sums), 7)
            with self.assertRaises(ValueError):
                collect_release.collect(root, root / "wrong-commit", "v" + value, "b" * 40, True)
            metadata_path = root / "native-linux" / f"doinstall-v{value}-linux-x64-metadata.json"
            info = json.loads(metadata_path.read_text())
            info["source_dirty"] = True
            release.write_json(metadata_path, info)
            sums_path = metadata_path.parent / "SHA256SUMS"
            sums_path.write_text("".join(f"{release.sha(p.read_bytes())}  {p.name}\n"
                                        for p in sorted(metadata_path.parent.iterdir()) if p != sums_path))
            with self.assertRaises(ValueError):
                collect_release.collect(root, root / "dirty", "v" + value, "a" * 40, True)
            metadata_path.write_text("tampered")
            with self.assertRaises(ValueError):
                collect_release.collect(root, root / "tampered", "v" + value, "a" * 40)
            metadata_path.unlink()
            with self.assertRaises(ValueError):
                collect_release.collect(root, root / "missing", "v" + value, "a" * 40)


if __name__ == "__main__":
    unittest.main()
