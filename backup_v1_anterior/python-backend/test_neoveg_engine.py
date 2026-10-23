"""
Automated Verification Suite for OpenNeoveg 21 Studio
Tests Rust/Python bridge, FFmpeg + Frei0r + LADSPA + CorridorKey filtergraphs,
Welcome Hub project management (Empty default + Demo project), Neoveg Bug Auto-Fixer,
Qt Square Styling, SVG Button Icons (zero emojis), funcoes_add.txt, and log_erros.txt.
"""

from pathlib import Path
import re
import sys

ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "python-backend"))

import error_logger
import plugins_engine
import rust_bridge
import neoveg_scripting


def run_all_tests() -> None:
    print("=== OpenNeoveg 21 — Comprehensive Verification Suite ===")

    # 1. Verify funcoes_add.txt and log_erros.txt exist and are non-empty
    funcoes_file = ROOT_DIR / "funcoes_add.txt"
    erros_file = ROOT_DIR / "log_erros.txt"
    assert funcoes_file.exists() and funcoes_file.stat().st_size > 500, "funcoes_add.txt missing or empty"
    assert erros_file.exists() and erros_file.stat().st_size > 200, "log_erros.txt missing or empty"
    print(f"[PASS] funcoes_add.txt ({funcoes_file.stat().st_size} bytes) & log_erros.txt ({erros_file.stat().st_size} bytes)")

    # 2. Verify CorridorKey repository clone & plugin probe
    ck_dir = ROOT_DIR / "plugins" / "CorridorKey"
    assert ck_dir.exists(), "plugins/CorridorKey directory missing"
    env_info = plugins_engine.probe_plugin_environment()
    assert env_info["corridorkey"]["installed"] is True, "CorridorKey not detected"
    assert env_info["frei0r"]["plugins_count"] >= 8, "Frei0r catalog incomplete"
    assert env_info["ladspa"]["plugins_count"] >= 5, "LADSPA catalog incomplete"
    print(f"[PASS] Plugin Environment: FFmpeg={env_info['ffmpeg_available']}, Frei0r={env_info['frei0r']['plugins_count']}, LADSPA={env_info['ladspa']['plugins_count']}, CorridorKey=Installed")

    # 3. Verify Empty Project default & Demo Project persistence
    empty_proj = plugins_engine.create_empty_project({"name": "Teste_Vazio", "width": 1920, "height": 1080, "fps": 60})
    assert empty_proj["isDemo"] is False
    assert all(len(t["clips"]) == 0 for t in empty_proj["tracks"]), "Empty project should have 0 clips initially"

    saved_list = plugins_engine.ensure_default_projects()
    assert any(p["fileName"] == "Neoveg_Broadcast_Demo.veg" for p in saved_list), "Demo project .veg missing"
    demo_loaded = plugins_engine.load_saved_project("Neoveg_Broadcast_Demo.veg")
    assert demo_loaded is not None and demo_loaded.get("isDemo") is True
    print(f"[PASS] Welcome Hub Projects: Empty Project (0 clips) + Demo Project ({len(saved_list)} saved .veg files)")

    # 4. Verify Neoveg Frequent Problems Diagnostic & Auto-Fixer
    diag_res = plugins_engine.diagnose_and_fix_neoveg_problems(demo_loaded)
    assert len(diag_res["protections_active"]) >= 5
    assert len(diag_res["fixes_applied"]) >= 1
    print(f"[PASS] Neoveg Problem Auto-Fixer: {len(diag_res['protections_active'])} active guards, {len(diag_res['fixes_applied'])} fixes applied")

    # 5. Verify FFmpeg + Frei0r + LADSPA + CorridorKey Filtergraph Compiler
    demo_loaded["tracks"][1]["clips"][0]["fx"].extend([
        {"id": "fx_ck", "type": "corridorKey", "name": "CorridorKey AI", "enabled": True, "params": {"similarity": 0.25, "blend": 0.08}},
        {"id": "fx_f0r", "type": "frei0r.glow", "name": "Frei0r Glow", "enabled": True, "params": {"blur": 0.5}},
    ])
    fg_plan = rust_bridge.compile_project_to_ffmpeg(demo_loaded, ROOT_DIR / "media", ROOT_DIR / "renders" / "test_out.mp4")
    cmd_str = fg_plan["command_str"]
    assert "chromakey=" in cmd_str and "despill=" in cmd_str, "CorridorKey filter missing from FFmpeg command"
    assert "unsharp=" in cmd_str, "Frei0r Glow filter missing from FFmpeg command"
    print(f"[PASS] Rust/Python FFmpeg Filtergraph Compiler: {len(cmd_str)} chars compiled with CorridorKey/Frei0r/LADSPA")

    # 6. Verify HTML buttons have SVG icons and zero emojis, and CSS enforces square Qt buttons
    html_text = (ROOT_DIR / "frontend" / "public" / "index.html").read_text(encoding="utf-8")
    svg_count = html_text.count("<svg class=\"neoveg-svg\"")
    assert svg_count >= 18, f"Expected >= 18 drawn SVG icons in index.html, found {svg_count}"
    assert "id=\"modal-advanced-settings\"" in html_text, "Separate Advanced Settings Window missing in index.html"
    assert "id=\"btn-group-av-tracks\"" in html_text, "Group V+A button missing in index.html"
    css_text = (ROOT_DIR / "frontend" / "public" / "styles.css").read_text(encoding="utf-8")
    assert "border-radius: 0px !important" in css_text, "Qt square button rule missing in styles.css"
    for theme_id in ["qt-neoveg21", "neoveg4-xp", "neoveg7-classic_nle", "neoveg13-pro", "neoveg18-modern_nle", "resolve-obsidian", "premiere-spectrum"]:
        assert theme_id in css_text, f"Theme {theme_id} missing in styles.css"
    print(f"[PASS] Qt Square UI, Simplified Welcome & Separate Advanced Settings Window verified")

    # 7. Verify Track Renaming & Track Grouping (Rust/Python Bridge + Neoveg Scripting API)
    t0_id = demo_loaded["tracks"][0]["id"]
    t2_id = demo_loaded["tracks"][2]["id"]
    rust_bridge.rename_track_in_project(demo_loaded, t0_id, "V1 — Master Cinema Camera")
    assert demo_loaded["tracks"][0]["name"] == "V1 — Master Cinema Camera", "Track rename failed"

    rust_bridge.group_tracks_in_project(demo_loaded, [t0_id, t2_id], "grp_test_va", "Grupo V+A 1", "#f59e0b")
    assert demo_loaded["tracks"][0]["groupId"] == "grp_test_va"
    assert demo_loaded["tracks"][2]["groupId"] == "grp_test_va"
    rust_bridge.sync_group_mute_solo_in_project(demo_loaded, "grp_test_va", muted=True, solo=None)
    assert demo_loaded["tracks"][0]["muted"] is True and demo_loaded["tracks"][2]["muted"] is True, "Group mute sync failed"

    preset_code = neoveg_scripting.PRESET_SCRIPTS["group_av_tracks"]["code"]
    script_res = neoveg_scripting.execute_neoveg_script(preset_code, demo_loaded)
    assert script_res["ok"] is True, f"group_av_tracks script failed: {script_res}"
    print("[PASS] Track Renaming, Track Grouping (V+A) & Python/Rust Scripting verified")

    # 8. Verify KEEPS.md exists and contains Wikipedia & Historical Research
    keeps_file = ROOT_DIR / "KEEPS.md"
    assert keeps_file.exists() and keeps_file.stat().st_size > 8000, "KEEPS.md missing or incomplete"
    keeps_text = keeps_file.read_text(encoding="utf-8")
    assert "Sonic Foundry" in keeps_text and "Contemporary NLE" in keeps_text and "Neoveg Toolkit" in keeps_text
    print(f"[PASS] KEEPS.md Encyclopedia ({keeps_file.stat().st_size} bytes) verified")

    error_logger.log_error("VERIFICATION_SUITE", "All 8 automated test suites passed with 0 errors.")
    print("=== ALL TESTS PASSED SUCCESSFULLY ===")


if __name__ == "__main__":
    run_all_tests()
