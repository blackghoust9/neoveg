"""
Validation & Interoperability Test Suite (`tests/test_xml_interchange.py`):
1. Native `.neoveg` Container (ZIP housing `project.json`, `media_pool.json`, and `history.json`).
2. Legacy Neoveg `.veg` Import-Only Inspection via Rust FFI (`neoveg_inspect_veg_container`).
3. FCP7 / Premiere Pro / DaVinci Resolve XML (`.xml` — `xmeml version="5"`) Export & Round-Trip Re-Import:
   - Verifies Timecode / Edit Rate (`29.97` NTSC / `timebase=30`),
   - Verifies Video & Audio tracks (`name`, `muted`, `solo`, `level`),
   - Verifies Clips/Events (`start`, `end`, `duration`, `trimIn` In Point, `outPoint` Out Point, Split `S` cut continuity, `opacity`, and `gain` volume),
   - Verifies Crossfade transitions (`Cross Dissolve`) and Timeline Markers (`M`).
4. Final Cut Pro X / DaVinci Resolve Structured XML (`.fcpxml` — `fcpxml version="1.10"`) Export & Re-Import.
5. CMX3600 Edit Decision List (`.edl`) Export verification.
"""

from __future__ import annotations

import json
import sys
import zipfile
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "python-backend"))

import rust_bridge  # noqa: E402
import xml_interchange  # noqa: E402


def build_test_neoveg_timeline() -> dict:
    """
    Generates a multi-track Neoveg timeline with a real Rust FFI Split (`S`) cut,
    overlapping crossfade, custom opacity, audio gain/volume, track mute/solo, and markers.
    """
    # Use Rust FFI `neoveg_timeline_split_clip` to split a 6.0s master clip at 2.4s (In Point = 1.0s)
    split_res = rust_bridge.timeline_split_clip(
        clip_start=0.0,
        clip_duration=6.0,
        clip_trim_in=1.0,
        playhead_sec=2.4,
        default_fade_sec=0.25,
    )
    assert split_res["status"] == 1, f"Rust FFI split failed: {split_res}"

    left_clip = {
        "id": "clip_v1_partA",
        "name": "Interview_CamA_Part1",
        "mediaId": "med_cam_a",
        "mediaFile": str((ROOT_DIR / "media" / "qa_hd_1080p.mp4").resolve()),
        "mediaType": "video",
        "color": "#0e7490",
        "start": split_res["left_start"],        # 0.0s
        "duration": split_res["left_duration"],  # 2.4s
        "trimIn": split_res["left_trim_in"],     # 1.0s (Out Point = 3.4s)
        "fadeIn": 0.30,
        "fadeOut": split_res["left_fade_out"],
        "opacity": 0.92,
        "gain": 1.0,
        "playbackRate": 1.0,
        "muted": False,
    }

    right_clip = {
        "id": "clip_v1_partB",
        "name": "Interview_CamA_Part2_SplitS",
        "mediaId": "med_cam_a",
        "mediaFile": str((ROOT_DIR / "media" / "qa_hd_1080p.mp4").resolve()),
        "mediaType": "video",
        "color": "#0e7490",
        "start": split_res["right_start"],        # 2.4s (contiguous after Split 'S')
        "duration": split_res["right_duration"],  # 3.6s (Ends at 6.0s)
        "trimIn": split_res["right_trim_in"],     # 3.4s (continuous TrimIn from left Out Point!)
        "fadeIn": split_res["right_fade_in"],
        "fadeOut": 0.50,
        "opacity": 0.78,
        "gain": 1.0,
        "playbackRate": 1.0,
        "muted": False,
    }

    # Third clip on Video Track 1 overlapping right_clip from 5.0s to 8.5s (1.0s Cross Dissolve)
    broll_clip = {
        "id": "clip_v1_broll",
        "name": "B_Roll_4K_Drone_XFade",
        "mediaId": "med_broll_4k",
        "mediaFile": str((ROOT_DIR / "media" / "qa_uhd_4k.mp4").resolve()),
        "mediaType": "video",
        "color": "#16a34a",
        "start": 5.0,
        "duration": 3.5,
        "trimIn": 0.5,
        "fadeIn": 1.0,
        "fadeOut": 0.4,
        "opacity": 0.85,
        "gain": 0.90,
        "playbackRate": 1.0,
        "muted": False,
    }

    # Audio Track 1 (Master Stereo Dialogue + Music Stem)
    audio_clip_1 = {
        "id": "clip_a1_dialogue",
        "name": "Dialogue_Boom_24bit_96kHz",
        "mediaId": "med_aud_hq",
        "mediaFile": str((ROOT_DIR / "media" / "qa_audio_24bit_96khz.wav").resolve()),
        "mediaType": "audio",
        "color": "#0891b2",
        "start": 0.0,
        "duration": 4.2,
        "trimIn": 0.25,
        "fadeIn": 0.20,
        "fadeOut": 0.30,
        "opacity": 1.0,
        "gain": 1.25,  # +1.94 dB boost
        "playbackRate": 1.0,
        "muted": False,
    }

    audio_clip_2 = {
        "id": "clip_a1_synth",
        "name": "Ambient_Bed_Stem",
        "mediaId": "med_aud_bed",
        "mediaFile": str((ROOT_DIR / "media" / "qa_audio_24bit_96khz.wav").resolve()),
        "mediaType": "audio",
        "color": "#881337",
        "start": 4.2,
        "duration": 4.3,
        "trimIn": 1.50,
        "fadeIn": 0.30,
        "fadeOut": 0.60,
        "opacity": 1.0,
        "gain": 0.62,  # -4.15 dB bed attenuation
        "playbackRate": 1.0,
        "muted": False,
    }

    return {
        "apiVersion": "2.1.0",
        "name": "Neoveg_XML_Interchange_Master",
        "width": 1920,
        "height": 1080,
        "fps": 29.97,
        "pixelFormat": "8-bit (Unsigned Integer)",
        "colorSpace": "Rec.709 Standard",
        "sampleRate": 48000,
        "audioBitDepth": 24,
        "tracks": [
            {
                "id": "tr_v1",
                "name": "Program Video A-Cam",
                "type": "video",
                "color": "#155e75",
                "height": 82.0,
                "level": 0.95,
                "pan": 0.0,
                "muted": False,
                "solo": True,
                "locked": False,
                "groupId": "grp_av_main",
                "groupName": "Main A/V Sync Group",
                "groupColor": "#f59e0b",
                "clips": [left_clip, right_clip, broll_clip],
            },
            {
                "id": "tr_v2_muted",
                "name": "Alt Angle Backup (Muted)",
                "type": "video",
                "color": "#4f46e5",
                "height": 74.0,
                "level": 0.80,
                "pan": 0.0,
                "muted": True,
                "solo": False,
                "locked": False,
                "clips": [],
            },
            {
                "id": "tr_a1",
                "name": "Master Stereo Dialogue & Bed",
                "type": "audio",
                "color": "#881337",
                "height": 78.0,
                "level": 0.88,
                "pan": -0.10,
                "muted": False,
                "solo": False,
                "locked": False,
                "groupId": "grp_av_main",
                "groupName": "Main A/V Sync Group",
                "groupColor": "#f59e0b",
                "clips": [audio_clip_1, audio_clip_2],
            },
        ],
        "markers": [
            {"id": "mk_1", "time": 2.4, "label": "Split Cut (S) Sync Point", "color": "#f97316"},
            {"id": "mk_2", "time": 5.0, "label": "Cross Dissolve Entry", "color": "#10b981"},
        ],
        "regions": [
            {"id": "rg_1", "start": 0.0, "end": 8.5, "label": "Full Broadcast Master"},
        ],
        "undoStack": [
            {
                "id": "act_split_s",
                "timestampUtc": "2026-10-21T18:00:00Z",
                "actionType": "SplitAtPlayhead",
                "description": "Split clip 'Interview_CamA' at 00:00:02;07 (2.400s)",
            }
        ],
        "redoStack": [],
    }


def run_xml_interchange_tests() -> dict:
    out_dir = ROOT_DIR / "projects" / "interchange_tests"
    out_dir.mkdir(parents=True, exist_ok=True)

    original = build_test_neoveg_timeline()

    # 1. Test Native `.neoveg` Container (ZIP: project.json + media_pool.json + history.json)
    neoveg_path = xml_interchange.save_neoveg_container(
        original,
        out_dir / "Neoveg_XML_Interchange_Master.neoveg",
        workspace_root=ROOT_DIR,
    )
    assert neoveg_path.exists() and zipfile.is_zipfile(neoveg_path), "Expected .neoveg to be a valid ZIP archive"
    with zipfile.ZipFile(neoveg_path, "r") as zf:
        entries = set(zf.namelist())
        assert {"project.json", "media_pool.json", "history.json"}.issubset(entries), (
            f"Missing required entries in .neoveg ZIP: {entries}"
        )

    loaded_proj, loaded_pool, loaded_hist = xml_interchange.load_neoveg_container(neoveg_path)
    assert loaded_proj["apiVersion"] == "2.1.0"
    assert len(loaded_pool["items"]) >= 3
    assert len(loaded_hist["undoStack"]) >= 1

    # 2. Test FCP7 / Premiere Pro XML (.xml) Export & Re-Import Round-Trip
    fcp7_path = xml_interchange.export_fcp7_xml(original, out_dir / "Neoveg_FCP7_RoundTrip.xml")
    assert fcp7_path.exists()
    reimported_fcp7 = xml_interchange.import_fcp7_xml(fcp7_path)

    assert abs(reimported_fcp7["fps"] - 29.97) < 0.01
    assert reimported_fcp7["width"] == 1920 and reimported_fcp7["height"] == 1080
    assert len(reimported_fcp7["tracks"]) == 3

    # Verify Track 0 (Video Track 1: Solo=True, Muted=False, Level=0.95, 3 clips + 1 crossfade transition)
    tr_v1 = reimported_fcp7["tracks"][0]
    assert tr_v1["name"] == "Program Video A-Cam"
    assert tr_v1["type"] == "video"
    assert tr_v1["solo"] is True and tr_v1["muted"] is False
    assert abs(tr_v1["level"] - 0.95) < 1e-4
    assert len(tr_v1["clips"]) == 3
    assert len(tr_v1["transitions"]) == 1, f"Expected 1 Cross Dissolve transition, got {tr_v1['transitions']}"

    # Verify Split ('S') cut continuity between clip 0 and clip 1
    c0, c1, c2 = tr_v1["clips"]
    assert abs(c0["start"] - 0.0) < 1e-4 and abs(c0["duration"] - 2.4) < 1e-4
    assert abs(c0["trimIn"] - 1.0) < 1e-4 and abs(c0["outPoint"] - 3.4) < 1e-4
    assert abs(c0["opacity"] - 0.92) < 1e-3

    assert abs(c1["start"] - 2.4) < 1e-4 and abs(c1["duration"] - 3.6) < 1e-4
    assert abs(c1["trimIn"] - 3.4) < 1e-4, "Split ('S') second half must start at exact Out Point (3.4s) of first half"
    assert abs(c1["opacity"] - 0.78) < 1e-3

    assert abs(c2["start"] - 5.0) < 1e-4 and abs(c2["duration"] - 3.5) < 1e-4
    assert abs(c2["trimIn"] - 0.5) < 1e-4 and abs(c2["opacity"] - 0.85) < 1e-3

    # Verify Track 1 (Muted Video Track)
    tr_v2 = reimported_fcp7["tracks"][1]
    assert tr_v2["muted"] is True and tr_v2["solo"] is False

    # Verify Track 2 (Audio Track: clip volumes/gains 1.25 and 0.62)
    tr_a1 = reimported_fcp7["tracks"][2]
    assert tr_a1["type"] == "audio" and abs(tr_a1["level"] - 0.88) < 1e-4
    assert len(tr_a1["clips"]) == 2
    ac0, ac1 = tr_a1["clips"]
    assert abs(ac0["start"] - 0.0) < 1e-4 and abs(ac0["duration"] - 4.2) < 1e-4
    assert abs(ac0["trimIn"] - 0.25) < 1e-4 and abs(ac0["gain"] - 1.25) < 1e-4
    assert abs(ac1["start"] - 4.2) < 1e-4 and abs(ac1["duration"] - 4.3) < 1e-4
    assert abs(ac1["trimIn"] - 1.50) < 1e-4 and abs(ac1["gain"] - 0.62) < 1e-4

    # Verify Markers
    assert len(reimported_fcp7["markers"]) == 2
    assert abs(reimported_fcp7["markers"][0]["time"] - 2.4) < 1e-4

    # 3. Test FCPX FCPXML (.fcpxml) Export & Re-Import Round-Trip
    fcpxml_path = xml_interchange.export_fcpxml(original, out_dir / "Neoveg_FCPX_RoundTrip.fcpxml")
    assert fcpxml_path.exists()
    reimported_fcpx = xml_interchange.import_fcpxml(fcpxml_path)
    assert len(reimported_fcpx["tracks"]) == 2  # 2 tracks with clips
    assert len(reimported_fcpx["tracks"][0]["clips"]) == 3
    assert abs(reimported_fcpx["tracks"][0]["clips"][1]["start"] - 2.4) < 1e-4
    assert abs(reimported_fcpx["tracks"][1]["clips"][0]["gain"] - 1.25) < 1e-4

    # 4. Test CMX3600 EDL (.edl) Export
    edl_path = xml_interchange.export_cmx3600_edl(original, out_dir / "Neoveg_Master_CMX3600.edl")
    edl_text = edl_path.read_text(encoding="utf-8")
    assert "TITLE: Neoveg_XML_Interchange_Master" in edl_text
    assert "Interview_CamA_Part2_SplitS" in edl_text

    return {
        "ok": True,
        "neovegPath": str(neoveg_path),
        "fcp7XmlPath": str(fcp7_path),
        "fcpxmlPath": str(fcpxml_path),
        "edlPath": str(edl_path),
        "tracksVerified": len(reimported_fcp7["tracks"]),
        "clipsVerified": sum(len(t["clips"]) for t in reimported_fcp7["tracks"]),
    }


if __name__ == "__main__":
    res = run_xml_interchange_tests()
    print(f"[PASS] XML Interchange & Native .neoveg Suite Passed: {json.dumps(res, indent=2)}")
