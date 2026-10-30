"""Tests of the packaging driver (`build_portable.py`), standard library only.

    python -m unittest scripts.test_build_portable        # from the repository root
    python scripts/test_build_portable.py

They stage a package from a fake executable in a scratch folder under `target/`, so nothing is compiled
and `dist/` is left alone. They check what ends up in the package: the licences that have to travel with
the program, the resources it looks for beside the executable, and that a missing file is reported.
"""
from __future__ import annotations

import importlib.util
import os
import shutil
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("build_portable", ROOT / "build_portable.py")
build_portable = importlib.util.module_from_spec(SPEC)
sys.modules["build_portable"] = build_portable
SPEC.loader.exec_module(build_portable)  # type: ignore[union-attr]

SCRATCH = ROOT / "target" / "tmp" / "test_build_portable"


class StagedPackage(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if SCRATCH.exists():
            shutil.rmtree(SCRATCH)
        SCRATCH.mkdir(parents=True)
        cls.old_dist = build_portable.DIST
        build_portable.DIST = SCRATCH / "dist"
        fake = SCRATCH / "neoveg.exe"
        fake.write_bytes(b"MZ fake executable")
        cls.stage = build_portable.stage_package("win-x64", fake, "9.9.9", ffmpeg=False)

    @classmethod
    def tearDownClass(cls) -> None:
        build_portable.DIST = cls.old_dist
        shutil.rmtree(SCRATCH, ignore_errors=True)

    def test_the_executable_and_the_project_licences_are_there(self) -> None:
        for name in ("neoveg.exe", "LICENSE", "LICENSE-MIT", "LICENSE-APACHE", "AUTHORS.md"):
            self.assertTrue((self.stage / name).is_file(), name)

    def test_the_licences_of_the_compiled_in_fonts_travel_with_the_program(self) -> None:
        for name in ("OFL-Inter.txt", "OFL-JetBrainsMono.txt", "OFL-NotoSerif.txt"):
            self.assertTrue((self.stage / "third_party" / "fonts" / name).is_file(), name)

    def test_the_colour_tables_are_where_the_program_looks_for_them(self) -> None:
        cubes = sorted(p.name for p in (self.stage / "assets" / "luts").glob("*.cube"))
        self.assertEqual(cubes, sorted(p.name for p in (ROOT / "assets" / "luts").glob("*.cube")))
        self.assertGreaterEqual(len(cubes), 1)

    def test_the_other_third_party_licences_are_there(self) -> None:
        for relative in (
            "third_party/filmcraft/LICENSE-MIT",
            "third_party/flat-color-icons/LICENSE-MIT",
            "third_party/nnnoiseless/COPYING",
        ):
            self.assertTrue((self.stage / relative).is_file(), relative)

    def test_every_file_the_package_promises_exists_in_the_repository(self) -> None:
        for source in build_portable.PAYLOAD_FILES:
            self.assertTrue((ROOT / source).is_file(), f"{source} is listed but missing")

    def test_the_archive_is_a_zip_with_the_folder_inside(self) -> None:
        import zipfile

        archive = build_portable.make_archive(self.stage, "win-x64", "9.9.9")
        self.assertEqual(archive.name, "Neoveg_v9.9.9_win-x64_Portable.zip")
        with zipfile.ZipFile(archive) as bundle:
            names = bundle.namelist()
        self.assertIn("Neoveg-Portable-win-x64/neoveg.exe", names)
        self.assertTrue(any(n.endswith("assets/luts/CINEMATIC.cube") for n in names))


class FfmpegLicence(unittest.TestCase):
    def test_the_ffmpeg_licence_is_copied_when_it_exists(self) -> None:
        folder = SCRATCH / "ffmpeg_case"
        (folder / "ffmpeg").mkdir(parents=True, exist_ok=True)
        (folder / "ffmpeg" / "LICENSE.txt").write_text("LGPL text", encoding="utf-8")
        stage = folder / "stage"
        stage.mkdir(exist_ok=True)
        old = os.environ.get("FFMPEG_DIR")
        os.environ["FFMPEG_DIR"] = str(folder / "ffmpeg")
        try:
            build_portable.copy_ffmpeg_license(stage)
        finally:
            if old is None:
                os.environ.pop("FFMPEG_DIR", None)
            else:
                os.environ["FFMPEG_DIR"] = old
        self.assertEqual(
            (stage / "third_party" / "ffmpeg" / "LICENSE.txt").read_text(encoding="utf-8"), "LGPL text"
        )
        shutil.rmtree(folder, ignore_errors=True)

    def test_without_ffmpeg_nothing_is_copied(self) -> None:
        stage = SCRATCH / "no_ffmpeg"
        stage.mkdir(parents=True, exist_ok=True)
        old = os.environ.pop("FFMPEG_DIR", None)
        try:
            build_portable.copy_ffmpeg_license(stage)
        finally:
            if old is not None:
                os.environ["FFMPEG_DIR"] = old
        self.assertFalse((stage / "third_party").exists())


class CleanTarget(unittest.TestCase):
    """`clean_target` deletes the Cargo output after a build, and only when that is safe."""

    def setUp(self) -> None:
        self.case = SCRATCH / "clean_case"
        if self.case.exists():
            shutil.rmtree(self.case)
        self.target = self.case / "target"
        (self.target / "release").mkdir(parents=True)
        (self.target / "release" / "neoveg.exe").write_bytes(b"x")
        self.old = (build_portable.TARGET_DIR, build_portable.DIST)
        build_portable.TARGET_DIR = self.target
        build_portable.DIST = self.case / "dist"

    def tearDown(self) -> None:
        build_portable.TARGET_DIR, build_portable.DIST = self.old
        shutil.rmtree(self.case, ignore_errors=True)

    def test_a_target_inside_the_repository_is_deleted(self) -> None:
        self.assertTrue(build_portable.clean_target())
        self.assertFalse(self.target.exists())

    def test_a_read_only_file_does_not_stop_the_deletion(self) -> None:
        locked = self.target / "release" / "locked.pdb"
        locked.write_bytes(b"x")
        os.chmod(locked, 0o444)
        self.assertTrue(build_portable.clean_target())
        self.assertFalse(self.target.exists())

    def test_a_target_outside_the_repository_is_left_alone(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as outside:
            far = Path(outside) / "target"
            far.mkdir()
            (far / "keep.txt").write_text("x", encoding="utf-8")
            build_portable.TARGET_DIR = far
            self.assertFalse(build_portable.clean_target())
            self.assertTrue((far / "keep.txt").is_file())

    def test_packages_inside_the_target_are_never_deleted(self) -> None:
        build_portable.DIST = self.target / "dist_check"
        build_portable.DIST.mkdir()
        (build_portable.DIST / "package.zip").write_bytes(b"x")
        self.assertFalse(build_portable.clean_target())
        self.assertTrue((build_portable.DIST / "package.zip").is_file())

    def test_a_missing_target_is_fine(self) -> None:
        shutil.rmtree(self.target)
        self.assertTrue(build_portable.clean_target())


class QaReport(unittest.TestCase):
    """The QA report is written even when a step fails and the later ones never ran."""

    def setUp(self) -> None:
        self.case = SCRATCH / "qa_case"
        self.case.mkdir(parents=True, exist_ok=True)
        self.old = (build_portable.QA_JSON, build_portable.QA_JS)
        build_portable.QA_JSON = self.case / "qa_report.json"
        build_portable.QA_JS = self.case / "qa_report.js"

    def tearDown(self) -> None:
        build_portable.QA_JSON, build_portable.QA_JS = self.old
        shutil.rmtree(self.case, ignore_errors=True)

    def test_a_step_that_never_ran_is_reported_as_skipped_not_a_crash(self) -> None:
        import json

        steps = [
            {"name": "rustfmt", "pillar": "Maintainability", "standard": "x", "status": "pass"},
            {"name": "Testes", "pillar": "Reliability", "standard": "x", "status": "fail"},
            {"name": "Build", "pillar": "Portability", "standard": "x"},  # no status: not reached
        ]
        build_portable.write_qa_report(steps, "fail")
        report = json.loads(build_portable.QA_JSON.read_text(encoding="utf-8"))
        self.assertEqual(report["outcome"], "fail")
        self.assertEqual(report["pillars"]["Maintainability"]["status"], "pass")
        self.assertEqual(report["pillars"]["Reliability"]["status"], "fail")
        self.assertEqual(report["pillars"]["Portability"]["status"], "skipped")
        self.assertEqual(report["pillars"]["Portability"]["passed"], 0)
        self.assertTrue(build_portable.QA_JS.read_text(encoding="utf-8").startswith("window.NEOVEG_QA_REPORT"))


if __name__ == "__main__":
    unittest.main()
