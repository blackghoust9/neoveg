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

    def test_the_i18n_string_catalogs_travel_with_the_package(self) -> None:
        staged = sorted(p.name for p in (self.stage / "Assets" / "i18n" / "strings").glob("*.json"))
        repo = sorted(p.name for p in (ROOT / "assets" / "i18n" / "strings").glob("*.json"))
        self.assertEqual(staged, repo)
        self.assertGreaterEqual(len(staged), 7)

    def test_the_other_third_party_licences_are_there(self) -> None:
        for relative in (
            "third_party/filmcraft/LICENSE-MIT",
            "third_party/flat-color-icons/LICENSE-MIT",
            "third_party/lucide/LICENSE",
            "third_party/phosphor-icons/LICENSE-MIT",
            "third_party/remix-icon/LICENSE-APACHE",
            "third_party/material-symbols/LICENSE-APACHE",
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


class FfmpegEnvironment(unittest.TestCase):
    """`ffmpeg_environment` accepts LLVM 19 and 22+ (`ffmpeg-next` 9.x / `bindgen` 0.73) and rejects missing headers."""

    def test_accepts_llvm_19_and_llvm_22_plus(self) -> None:
        case = SCRATCH / "ffmpeg_env_case"
        (case / "ffmpeg" / "include").mkdir(parents=True, exist_ok=True)
        (case / "llvm").mkdir(parents=True, exist_ok=True)
        old_env = (os.environ.get("FFMPEG_DIR"), os.environ.get("LIBCLANG_PATH"))
        old_major = build_portable.libclang_major
        os.environ["FFMPEG_DIR"] = str(case / "ffmpeg")
        os.environ["LIBCLANG_PATH"] = str(case / "llvm")
        try:
            for version in (19, 20, 22, 23):
                build_portable.libclang_major = lambda _folder, v=version: v  # type: ignore[assignment]
                self.assertEqual(build_portable.ffmpeg_environment(), [], f"LLVM {version} rejected")
            build_portable.libclang_major = lambda _folder: 14  # type: ignore[assignment]
            self.assertEqual(len(build_portable.ffmpeg_environment()), 1)
        finally:
            build_portable.libclang_major = old_major  # type: ignore[assignment]
            for key, value in zip(("FFMPEG_DIR", "LIBCLANG_PATH"), old_env):
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value
            shutil.rmtree(case, ignore_errors=True)

    def test_unix_accepts_pkg_config_and_system_clang_while_windows_requires_explicit_paths(self) -> None:
        old_env = (os.environ.pop("FFMPEG_DIR", None), os.environ.pop("LIBCLANG_PATH", None))
        old_system = build_portable.platform.system
        old_pkg = build_portable.pkg_config_has_ffmpeg
        old_capture = build_portable.capture
        try:
            build_portable.pkg_config_has_ffmpeg = lambda: True  # type: ignore[assignment]
            build_portable.capture = lambda _cmd: "clang version 19.1.7"  # type: ignore[assignment]
            for unix_os in ("Linux", "Darwin"):
                build_portable.platform.system = lambda os_name=unix_os: os_name  # type: ignore[assignment]
                self.assertEqual(build_portable.ffmpeg_environment(), [], unix_os)
            build_portable.platform.system = lambda: "Windows"  # type: ignore[assignment]
            self.assertEqual(len(build_portable.ffmpeg_environment()), 2)
        finally:
            build_portable.platform.system = old_system  # type: ignore[assignment]
            build_portable.pkg_config_has_ffmpeg = old_pkg  # type: ignore[assignment]
            build_portable.capture = old_capture  # type: ignore[assignment]
            for key, value in zip(("FFMPEG_DIR", "LIBCLANG_PATH"), old_env):
                if value is not None:
                    os.environ[key] = value

    def test_libclang_major_finds_clang_in_sibling_bin_folder(self) -> None:
        case = SCRATCH / "llvm_sibling_case"
        (case / "llvm" / "lib").mkdir(parents=True, exist_ok=True)
        (case / "llvm" / "bin").mkdir(parents=True, exist_ok=True)
        (case / "llvm" / "bin" / "clang").write_text("#!/bin/sh\n", encoding="utf-8")
        old_capture = build_portable.capture
        try:
            build_portable.capture = lambda _cmd: "Homebrew clang version 19.1.7"  # type: ignore[assignment]
            self.assertEqual(build_portable.libclang_major(case / "llvm" / "lib"), 19)
        finally:
            build_portable.capture = old_capture  # type: ignore[assignment]
            shutil.rmtree(case, ignore_errors=True)


class MultiOsStagingAndCi(unittest.TestCase):
    """Linux/macOS portable staging and GitHub Actions workflow invariants across all three OS families."""

    def test_linux_and_macos_packages_stage_and_archive(self) -> None:
        import tarfile

        case = SCRATCH / "multios_case"
        if case.exists():
            shutil.rmtree(case)
        (case / "ffmpeg" / "lib").mkdir(parents=True)
        (case / "ffmpeg" / "lib" / "libavcodec.so.61").write_bytes(b"\x7fELF fake so")
        (case / "ffmpeg" / "lib" / "libswscale.so.8").write_bytes(b"\x7fELF fake sws")
        (case / "ffmpeg" / "LICENSE.txt").write_text("LGPL v2.1+", encoding="utf-8")
        fake_bin = case / "neoveg"
        fake_bin.write_bytes(b"\x7fELF fake neoveg")

        old_dist = build_portable.DIST
        old_ffmpeg = os.environ.get("FFMPEG_DIR")
        build_portable.DIST = case / "dist"
        os.environ["FFMPEG_DIR"] = str(case / "ffmpeg")
        try:
            linux_stage = build_portable.stage_package("linux-x64", fake_bin, "9.9.9", ffmpeg=True)
            self.assertTrue((linux_stage / "neoveg.sh").is_file())
            self.assertTrue((linux_stage / "lib" / "libavcodec.so.61").is_file())
            self.assertTrue((linux_stage / "third_party" / "ffmpeg" / "LICENSE.txt").is_file())
            linux_archive = build_portable.make_archive(linux_stage, "linux-x64", "9.9.9")
            self.assertEqual(linux_archive.name, "Neoveg_v9.9.9_linux-x64_Portable.tar.gz")
            with tarfile.open(linux_archive, "r:gz") as bundle:
                tar_names = bundle.getnames()
            self.assertIn("Neoveg-Portable-linux-x64/neoveg.sh", tar_names)
            self.assertIn("Neoveg-Portable-linux-x64/lib/libavcodec.so.61", tar_names)

            os.environ.pop("FFMPEG_DIR", None)
            mac_stage = build_portable.stage_package("osx-arm64", fake_bin, "9.9.9", ffmpeg=True, macos_min="11.0")
            self.assertTrue((mac_stage / "Neoveg.app" / "Contents" / "MacOS" / "neoveg").is_file())
            plist = (mac_stage / "Neoveg.app" / "Contents" / "Info.plist").read_text(encoding="utf-8")
            self.assertIn("LSMinimumSystemVersion", plist)
            mac_archive = build_portable.make_archive(mac_stage, "osx-arm64", "9.9.9")
            self.assertEqual(mac_archive.name, "Neoveg_v9.9.9_osx-arm64_Portable.tar.gz")
            with tarfile.open(mac_archive, "r:gz") as bundle:
                mac_member = bundle.getmember("Neoveg-Portable-osx-arm64/Neoveg.app/Contents/MacOS/neoveg")
            self.assertEqual(mac_member.mode, 0o755)
        finally:
            build_portable.DIST = old_dist
            if old_ffmpeg is None:
                os.environ.pop("FFMPEG_DIR", None)
            else:
                os.environ["FFMPEG_DIR"] = old_ffmpeg
            shutil.rmtree(case, ignore_errors=True)

    def test_ci_workflow_covers_build_test_clippy_llvm_cov_ffmpeg_x264_and_the_windows_package(self) -> None:
        workflow_path = ROOT / ".github" / "workflows" / "rust.yml"
        action_path = ROOT / ".github" / "actions" / "setup-ffmpeg" / "action.yml"
        self.assertTrue(workflow_path.is_file(), "missing .github/workflows/rust.yml")
        self.assertTrue(action_path.is_file(), "missing .github/actions/setup-ffmpeg/action.yml")

        workflow = workflow_path.read_text(encoding="utf-8")
        action = action_path.read_text(encoding="utf-8")

        for snippet in (
            "cargo fmt --all -- --check",
            "cargo clippy --workspace --all-targets --locked -- -D warnings",
            "cargo clippy --workspace --all-targets --features neoveg_ui/ffmpeg --locked -- -D warnings",
            "cargo build --workspace --features neoveg_ui/ffmpeg --locked",
            "cargo test --workspace --features neoveg_ui/ffmpeg --locked",
            "cargo llvm-cov --workspace --features neoveg_ui/ffmpeg --locked",
            "cargo deny check",
            "build_portable.py build --ffmpeg --targets ${{ matrix.rid }} --skip-qa --keep-target",
            "rid: win-x64",
            "windows-latest",
        ):
            self.assertIn(snippet, workflow, f"missing in rust.yml: {snippet}")

        for snippet in ("prefer-cli-x264", "libx264", "lgpl-shared-7.1", "ffmpeg@7"):
            self.assertIn(snippet, action, f"missing in setup-ffmpeg/action.yml: {snippet}")

        try:
            import yaml  # type: ignore[import-not-found]
        except ImportError:
            yaml = None
        if yaml is not None:
            parsed_wf = yaml.safe_load(workflow)
            parsed_act = yaml.safe_load(action)
            self.assertEqual(set(parsed_wf["jobs"].keys()), {"quality", "coverage", "package"})
            self.assertEqual(parsed_act["runs"]["using"], "composite")


class WindowsLinkerMitigation(unittest.TestCase):
    """Windows `LNK1201`/`LNK1104` mitigation: `rust-lld.exe` config plus automatic transient-lock retry."""

    def test_cargo_config_uses_rust_lld_and_disables_build_script_debuginfo(self) -> None:
        cargo_cfg = (ROOT / ".cargo" / "config.toml").read_text(encoding="utf-8")
        cargo_toml = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        self.assertIn("[target.x86_64-pc-windows-msvc]", cargo_cfg)
        self.assertIn('linker = "rust-lld.exe"', cargo_cfg)
        self.assertIn("[profile.dev.build-override]", cargo_toml)

    def test_run_retries_once_on_windows_linker_lock_and_not_on_compile_errors(self) -> None:
        old_system = build_portable.platform.system
        old_run_once = build_portable._run_once
        old_clear = build_portable.clear_windows_link_locks
        calls: list[list[str]] = []
        cleared = 0
        try:
            build_portable.platform.system = lambda: "Windows"  # type: ignore[assignment]

            def fake_clear() -> int:
                nonlocal cleared
                cleared += 1
                return 2

            build_portable.clear_windows_link_locks = fake_clear  # type: ignore[assignment]

            responses = [
                (101, "LINK : fatal error LNK1201: error writing to program database 'foo.pdb'"),
                (0, ""),
            ]

            def fake_once(cmd: list[str], _env: dict[str, str]) -> tuple[int, str]:
                calls.append(cmd)
                return responses.pop(0)

            build_portable._run_once = fake_once  # type: ignore[assignment]
            self.assertEqual(build_portable.run(["cargo", "test"], check=True), 0)
            self.assertEqual(len(calls), 2)
            self.assertEqual(cleared, 1)

            calls.clear()
            responses[:] = [(101, "error[E0308]: mismatched types")]
            self.assertEqual(build_portable.run(["cargo", "test"], check=False), 101)
            self.assertEqual(len(calls), 1)
            self.assertEqual(cleared, 1)
        finally:
            build_portable.platform.system = old_system  # type: ignore[assignment]
            build_portable._run_once = old_run_once  # type: ignore[assignment]
            build_portable.clear_windows_link_locks = old_clear  # type: ignore[assignment]



class MemoryProfile(unittest.TestCase):
    """8 GB or less builds with less memory; above that the profile of Cargo.toml is used."""

    GIB = 1024**3

    def setUp(self) -> None:
        self.saved = (build_portable.total_memory_bytes, dict(os.environ))
        for key in ("NEOVEG_LOW_MEMORY", "CARGO_PROFILE_RELEASE_LTO", "CARGO_PROFILE_RELEASE_CODEGEN_UNITS"):
            os.environ.pop(key, None)

    def tearDown(self) -> None:
        build_portable.total_memory_bytes = self.saved[0]  # type: ignore[assignment]
        os.environ.clear()
        os.environ.update(self.saved[1])

    def machine(self, gib: float) -> None:
        build_portable.total_memory_bytes = lambda: int(gib * self.GIB)  # type: ignore[assignment]

    def jobs(self, value=None):
        import argparse

        return build_portable.job_flags(argparse.Namespace(jobs=value))

    def test_a_machine_of_8_gb_or_less_gets_thin_lto_and_one_job(self) -> None:
        for gib in (4, 7.89, 8):
            self.machine(gib)
            env = build_portable.release_env("x86_64-pc-windows-msvc")
            self.assertEqual(env["CARGO_PROFILE_RELEASE_LTO"], "thin", gib)
            self.assertEqual(env["CARGO_PROFILE_RELEASE_CODEGEN_UNITS"], "16", gib)
            self.assertIn("crt-static", env["RUSTFLAGS"])
            self.assertEqual(self.jobs(), ["-j", "1"], gib)

    def test_a_bigger_machine_keeps_the_profile_of_cargo_toml_and_every_core(self) -> None:
        for gib in (16, 32):
            self.machine(gib)
            env = build_portable.release_env("x86_64-unknown-linux-gnu")
            self.assertNotIn("CARGO_PROFILE_RELEASE_LTO", env)
            self.assertNotIn("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", env)
            self.assertEqual(self.jobs(), [])
        self.assertEqual(self.jobs(4), ["-j", "4"], "--jobs always wins")

    def test_an_unreadable_memory_size_is_not_taken_as_low_and_the_override_wins(self) -> None:
        build_portable.total_memory_bytes = lambda: None  # type: ignore[assignment]
        self.assertFalse(build_portable.low_memory())
        os.environ["NEOVEG_LOW_MEMORY"] = "1"
        self.assertTrue(build_portable.low_memory())
        self.machine(64)
        self.assertTrue(build_portable.low_memory(), "forced")
        os.environ["NEOVEG_LOW_MEMORY"] = "0"
        self.machine(2)
        self.assertFalse(build_portable.low_memory(), "forced")

    def test_a_value_set_by_the_user_is_not_overridden_and_macos_keeps_its_target(self) -> None:
        self.machine(8)
        os.environ["CARGO_PROFILE_RELEASE_LTO"] = "off"
        env = build_portable.release_env("aarch64-apple-darwin", "11.0")
        self.assertEqual(env["CARGO_PROFILE_RELEASE_LTO"], "off")
        self.assertEqual(env["MACOSX_DEPLOYMENT_TARGET"], "11.0")

    def test_this_machine_reports_its_memory(self) -> None:
        build_portable.total_memory_bytes = self.saved[0]  # type: ignore[assignment]
        total = build_portable.total_memory_bytes()
        self.assertTrue(total is None or total > self.GIB)


class CompanionHostsAndQaFixes(unittest.TestCase):
    """Regression coverage for companion hosts, per-crate low-memory QA, macOS min env, triple PDB cleanup and NEOVEG_DIST."""

    def test_companion_vst3_and_ofx_hosts_are_built_staged_and_marked_executable(self) -> None:
        import tarfile

        self.assertEqual(
            build_portable.release_package_args(True),
            ["-p", "neoveg_ui", "-p", "neoveg_audio", "-p", "neoveg_ofx", "--features", "neoveg_ui/ffmpeg"],
        )
        case = SCRATCH / "companions_case"
        if case.exists():
            shutil.rmtree(case)
        (case / "win").mkdir(parents=True)
        for name in ("neoveg.exe", "neoveg_vst3_host.exe", "neoveg_ofx_host.exe"):
            (case / "win" / name).write_bytes(b"MZ fake")
        (case / "unix").mkdir(parents=True)
        for name in ("neoveg", "neoveg_vst3_host", "neoveg_ofx_host"):
            (case / "unix" / name).write_bytes(b"\x7fELF fake")

        old_dist = build_portable.DIST
        build_portable.DIST = case / "dist"
        try:
            win_stage = build_portable.stage_package("win-x64", case / "win" / "neoveg.exe", "9.9.9", ffmpeg=False)
            self.assertTrue((win_stage / "neoveg_vst3_host.exe").is_file())
            self.assertTrue((win_stage / "neoveg_ofx_host.exe").is_file())

            linux_stage = build_portable.stage_package("linux-x64", case / "unix" / "neoveg", "9.9.9", ffmpeg=False)
            self.assertTrue((linux_stage / "neoveg_vst3_host").is_file())
            self.assertTrue((linux_stage / "neoveg_ofx_host").is_file())
            linux_tar = build_portable.make_archive(linux_stage, "linux-x64", "9.9.9")
            with tarfile.open(linux_tar, "r:gz") as bundle:
                for member_name in ("neoveg", "neoveg_vst3_host", "neoveg_ofx_host", "neoveg.sh"):
                    member = bundle.getmember(f"Neoveg-Portable-linux-x64/{member_name}")
                    self.assertEqual(member.mode, 0o755, member_name)

            mac_stage = build_portable.stage_package("osx-arm64", case / "unix" / "neoveg", "9.9.9", ffmpeg=False)
            macos_dir = mac_stage / "Neoveg.app" / "Contents" / "MacOS"
            self.assertTrue((macos_dir / "neoveg_vst3_host").is_file())
            self.assertTrue((macos_dir / "neoveg_ofx_host").is_file())
        finally:
            build_portable.DIST = old_dist
            shutil.rmtree(case, ignore_errors=True)

    def test_qa_steps_runs_crate_by_crate_on_low_memory_and_workspace_on_large_machines(self) -> None:
        import argparse

        old_low = os.environ.get("NEOVEG_LOW_MEMORY")
        try:
            os.environ["NEOVEG_LOW_MEMORY"] = "1"
            low_steps = build_portable.qa_steps(argparse.Namespace(jobs=None, ffmpeg=True, macos_min=""))
            names = [str(s["name"]) for s in low_steps]
            self.assertIn("Testes (neoveg_common)", names)
            self.assertIn("Testes (neoveg_engine)", names)
            self.assertIn("Testes com FFmpeg (neoveg_video)", names)
            self.assertNotIn("Testes do workspace", names)

            os.environ["NEOVEG_LOW_MEMORY"] = "0"
            high_steps = build_portable.qa_steps(argparse.Namespace(jobs=None, ffmpeg=True, macos_min=""))
            high_names = [str(s["name"]) for s in high_steps]
            self.assertIn("Testes do workspace", high_names)
            self.assertIn("Testes com FFmpeg", high_names)
        finally:
            if old_low is None:
                os.environ.pop("NEOVEG_LOW_MEMORY", None)
            else:
                os.environ["NEOVEG_LOW_MEMORY"] = old_low

    def test_qa_steps_forwards_macos_min_into_release_env(self) -> None:
        import argparse

        saved = (build_portable.host_rid, build_portable.host_triple, build_portable.macos_floor)
        try:
            build_portable.host_rid = lambda: "osx-arm64"  # type: ignore[assignment]
            build_portable.host_triple = lambda: "aarch64-apple-darwin"  # type: ignore[assignment]
            build_portable.macos_floor = lambda _triple: "11.0"  # type: ignore[assignment]
            steps = build_portable.qa_steps(argparse.Namespace(jobs=1, ffmpeg=False, macos_min="12.0"))
            release_step = steps[-1]
            self.assertEqual(release_step["env"]["MACOSX_DEPLOYMENT_TARGET"], "12.0")  # type: ignore[index]
        finally:
            build_portable.host_rid, build_portable.host_triple, build_portable.macos_floor = saved  # type: ignore[assignment]

    def test_clear_windows_link_locks_cleans_explicit_target_triple_subdirs(self) -> None:
        case = SCRATCH / "pdb_triple_case"
        if case.exists():
            shutil.rmtree(case)
        root_dep = case / "debug" / "deps"
        triple_dep = case / "x86_64-pc-windows-msvc" / "release" / "deps"
        keep_tmp = case / "tmp" / "debug"
        for folder in (root_dep, triple_dep, keep_tmp):
            folder.mkdir(parents=True)
        (root_dep / "a.pdb").write_bytes(b"pdb")
        (triple_dep / "b.pdb").write_bytes(b"pdb")
        (triple_dep / "b.ilk").write_bytes(b"ilk")
        (keep_tmp / "keep.pdb").write_bytes(b"keep")

        old_target = build_portable.TARGET_DIR
        old_sys = build_portable.platform.system
        old_sleep = build_portable.time.sleep
        build_portable.TARGET_DIR = case
        build_portable.platform.system = lambda: "Linux"  # type: ignore[assignment]
        build_portable.time.sleep = lambda _s: None  # type: ignore[assignment]
        try:
            removed = build_portable.clear_windows_link_locks()
            self.assertEqual(removed, 3)
            self.assertFalse((root_dep / "a.pdb").exists())
            self.assertFalse((triple_dep / "b.pdb").exists())
            self.assertFalse((triple_dep / "b.ilk").exists())
            self.assertTrue((keep_tmp / "keep.pdb").is_file())
        finally:
            build_portable.TARGET_DIR = old_target
            build_portable.platform.system = old_sys  # type: ignore[assignment]
            build_portable.time.sleep = old_sleep  # type: ignore[assignment]
            shutil.rmtree(case, ignore_errors=True)

    def test_default_dist_honors_neoveg_dist(self) -> None:
        old = os.environ.get("NEOVEG_DIST")
        custom = SCRATCH / "custom_env_dist"
        try:
            os.environ["NEOVEG_DIST"] = str(custom)
            self.assertEqual(build_portable.default_dist(), custom.resolve())
            os.environ.pop("NEOVEG_DIST", None)
            self.assertEqual(build_portable.default_dist(), ROOT / "dist")
        finally:
            if old is None:
                os.environ.pop("NEOVEG_DIST", None)
            else:
                os.environ["NEOVEG_DIST"] = old


if __name__ == "__main__":
    unittest.main()
