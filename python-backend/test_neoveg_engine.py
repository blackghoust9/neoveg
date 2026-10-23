"""
Pytest & CLI entry point delegating to `scripts/test_neoveg_engine.py`.
"""

from pathlib import Path
import importlib.util
import sys

ROOT_DIR = Path(__file__).resolve().parent.parent
SCRIPTS_TEST = ROOT_DIR / "scripts" / "test_neoveg_engine.py"


def _load_suite_module():
    spec = importlib.util.spec_from_file_location("test_neoveg_engine_scripts", SCRIPTS_TEST)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    sys.modules["test_neoveg_engine_scripts"] = mod
    spec.loader.exec_module(mod)
    return mod


def test_neoveg_engine_suite() -> None:
    mod = _load_suite_module()
    mod.run_all_tests()


if __name__ == "__main__":
    _load_suite_module().run_all_tests()
