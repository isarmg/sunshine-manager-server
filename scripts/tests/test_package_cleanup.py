import importlib.util
from pathlib import Path
import stat
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("package_release", ROOT / "scripts/package-release.py")
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)


class CleanupTests(unittest.TestCase):
    def test_read_only_release_and_current_links_are_cleaned_without_following_links(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            outside = parent / "outside"
            outside.mkdir()
            protected = outside / "protected"
            protected.write_text("preserve")
            protected.chmod(0o444)
            outside.chmod(0o555)
            stage = parent / "stage"
            stage.mkdir()
            physical = stage / "release"
            physical.mkdir()
            resource = physical / "asset"
            resource.write_text("asset")
            resource.chmod(0o444)
            physical.chmod(0o555)
            (stage / "current").symlink_to(physical)
            (stage / "outside-link").symlink_to(outside)
            (stage / "file-link").symlink_to(protected)
            (stage / "missing-link").symlink_to(parent / "missing")
            PACKAGE.chmod_tree_for_cleanup(stage)
            self.assertEqual(stat.S_IMODE(resource.stat().st_mode), 0o600)
            self.assertEqual(stat.S_IMODE(physical.stat().st_mode), 0o700)
            self.assertEqual(stat.S_IMODE(outside.stat().st_mode), 0o555)
            self.assertEqual(stat.S_IMODE(protected.stat().st_mode), 0o444)
            self.assertTrue((stage / "current").is_symlink())
            outside.chmod(0o700)


if __name__ == "__main__":
    unittest.main()
