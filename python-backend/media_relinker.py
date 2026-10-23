"""
Neoveg — Automated 3-Stage Missing Media Search & Relinker (`python-backend/media_relinker.py`)
Mirrors Neoveg's Missing Media resolution engine:
1. Recursive Directory Scanner across user-selected folders/drives.
2. 3-Stage Matching Algorithm:
   - Stage 1: Exact Filename & Extension Match (case-insensitive).
   - Stage 2: Fuzzy Camera Prefix/Suffix & Token Similarity Match (e.g., `CAM01_neoveg_cyber_scene_FINAL.mp4` -> `neoveg_cyber_scene.mp4`).
   - Stage 3: Metadata Validation via Isolated FFprobe & Rust FFI (verifies video/audio stream presence and duration compatibility).
3. Same-Directory Sibling Auto-Relink: Once one missing file is found in a folder, automatically relinks any other missing project files located in that same folder.
4. Rust FFI `.sfk` Peak Cache Regeneration for relinked audio/video files.
"""

from __future__ import annotations
import argparse
from difflib import SequenceMatcher
import json
from pathlib import Path
import re
from typing import Any, Dict, List, Optional, Tuple

import ffmpeg_manager
import rust_bridge

VIDEO_EXTS = {".mp4", ".mov", ".mkv", ".avi", ".webm", ".m4v", ".mxf"}
AUDIO_EXTS = {".wav", ".mp3", ".flac", ".aac", ".ogg", ".m4a", ".aiff"}

CAMERA_NOISE_PATTERNS = [
    r"^cam\d+[_-]?",
    r"^dji[_-]?\d*[_-]?",
    r"^gopro[_-]?",
    r"^a\d{3}[_-]?c\d{3}[_-]?",
    r"[_-]?final\d*$",
    r"[_-]?master$",
    r"[_-]?proxy$",
    r"[_-]?v\d+$",
    r"[_-]?copy$",
    r"[_-]?take\d+$",
]


def normalize_stem(filename: str) -> str:
    """Strips extension and common camera/render prefixes/suffixes for Stage 2 Fuzzy Matching."""
    stem = Path(filename).stem.lower().strip()
    for pat in CAMERA_NOISE_PATTERNS:
        stem = re.sub(pat, "", stem, flags=re.IGNORECASE).strip("_- ")
    return stem or Path(filename).stem.lower()


def compute_fuzzy_similarity(expected_filename: str, candidate_filename: str) -> float:
    s1 = normalize_stem(expected_filename)
    s2 = normalize_stem(candidate_filename)
    if not s1 or not s2:
        return 0.0
    if s1 == s2:
        return 0.98
    if s1 in s2 or s2 in s1:
        return 0.88
    return round(SequenceMatcher(None, s1, s2).ratio(), 4)


def is_compatible_media_type(expected_type: str, candidate_path: Path) -> bool:
    ext = candidate_path.suffix.lower()
    if expected_type.lower() == "audio":
        return ext in AUDIO_EXTS or ext in VIDEO_EXTS
    return ext in VIDEO_EXTS


def validate_candidate_metadata(
    mgr: ffmpeg_manager.IsolatedFFmpegManager,
    candidate_path: Path,
    expected_type: str,
    expected_duration: float,
    duration_tolerance_sec: float = 12.0,
) -> Tuple[bool, Dict[str, Any]]:
    """Stage 3: Validates candidate file via Isolated FFprobe to confirm stream type and duration compatibility."""
    if not candidate_path.exists() or not candidate_path.is_file():
        return False, {}

    meta = mgr.probe_media(candidate_path)
    actual_type = str(meta.get("type", "video")).lower()
    actual_dur = float(meta.get("duration", 0.0))

    if expected_type.lower() == "video" and actual_type != "video":
        return False, meta

    if expected_duration > 0.1 and actual_dur > 0.1:
        if abs(actual_dur - expected_duration) > max(duration_tolerance_sec, expected_duration * 2.5):
            return False, meta

    return True, meta


def scan_and_relink_missing_media(
    search_or_root_dir: Path,
    missing_items_or_search_dir: Any,
    maybe_missing_items: Optional[List[Dict[str, Any]]] = None,
) -> Dict[str, Any]:
    """
    Runs the 3-Stage Relink Algorithm + Same-Directory Sibling Auto-Relink + Rust FFI `.sfk` regeneration.
    Supports both `(search_dir, missing_items)` and `(root_dir, search_dir, missing_items)` call signatures.
    """
    if maybe_missing_items is not None:
        root_dir = Path(search_or_root_dir)
        search_dir = Path(missing_items_or_search_dir)
        missing_items = list(maybe_missing_items)
    else:
        root_dir = Path(__file__).resolve().parent.parent
        search_dir = Path(search_or_root_dir)
        missing_items = list(missing_items_or_search_dir)

    mgr = ffmpeg_manager.IsolatedFFmpegManager(root_dir)

    if not search_dir.exists():
        search_dir = root_dir / "media"

    all_candidates: List[Path] = []
    for p in search_dir.rglob("*"):
        if p.is_file() and (p.suffix.lower() in VIDEO_EXTS or p.suffix.lower() in AUDIO_EXTS):
            all_candidates.append(p)

    results: List[Dict[str, Any]] = []
    discovered_directories: List[Path] = []

    # Pass 1: Search across `all_candidates` using 3-Stage Algorithm
    for item in missing_items:
        item_id = str(item.get("id", ""))
        orig_path = str(item.get("originalPath") or item.get("filePath") or "")
        expected_name = Path(orig_path).name if orig_path else str(item.get("fileName") or item.get("name") or "")
        expected_type = str(item.get("mediaType") or item.get("type") or "video")
        expected_dur = float(item.get("expectedDuration") or item.get("duration") or 4.0)

        matched_path: Optional[Path] = None
        match_stage = "None"
        similarity = 0.0
        probed_meta: Dict[str, Any] = {}

        # Stage 1: Exact Filename Match
        for cand in all_candidates:
            if cand.name.lower() == expected_name.lower():
                ok_meta, meta = validate_candidate_metadata(mgr, cand, expected_type, expected_dur)
                if ok_meta:
                    matched_path = cand
                    match_stage = "Stage 1: Exact Filename + FFprobe Metadata"
                    similarity = 1.0
                    probed_meta = meta
                    break

        # Stage 2 & Stage 3: Fuzzy Camera Name Match + FFprobe Metadata Validation
        if matched_path is None:
            scored: List[Tuple[float, Path]] = []
            for cand in all_candidates:
                if not is_compatible_media_type(expected_type, cand):
                    continue
                sim = compute_fuzzy_similarity(expected_name, cand.name)
                if sim >= 0.55:
                    scored.append((sim, cand))
            scored.sort(key=lambda x: x[0], reverse=True)

            for sim, cand in scored:
                ok_meta, meta = validate_candidate_metadata(mgr, cand, expected_type, expected_dur)
                if ok_meta:
                    matched_path = cand
                    match_stage = f"Stage 2+3: Fuzzy Match ({int(sim * 100)}%) + FFprobe Validated"
                    similarity = sim
                    probed_meta = meta
                    break

        if matched_path is not None:
            if matched_path.parent not in discovered_directories:
                discovered_directories.append(matched_path.parent)

            # Regenerate Rust FFI .sfk audio peak cache
            sfk_info = rust_bridge.build_or_load_sfk_cache(matched_path, num_buckets=96)
            sfk_path_val = sfk_info.get("sfk_path", str(matched_path) + ".sfk")
            results.append({
                "id": item_id,
                "fileName": expected_name,
                "originalPath": orig_path,
                "resolvedPath": str(matched_path.resolve()),
                "found": True,
                "matchStage": match_stage,
                "similarity": similarity,
                "sfkPath": sfk_path_val,
                "sfkCachePath": sfk_path_val,
                "peaks": sfk_info.get("peaks", []),
                "duration": probed_meta.get("duration", expected_dur),
            })
        else:
            results.append({
                "id": item_id,
                "fileName": expected_name,
                "originalPath": orig_path,
                "resolvedPath": "",
                "found": False,
                "matchStage": "Unresolved",
                "similarity": 0.0,
                "sfkPath": "",
                "peaks": [],
                "duration": expected_dur,
            })

    # Pass 2: Same-Directory Sibling Auto-Relink for any remaining unresolved items
    for res_entry in results:
        if res_entry["found"]:
            continue
        expected_name = res_entry["fileName"]
        for sibling_dir in discovered_directories:
            for sibling_file in sibling_dir.iterdir():
                if not sibling_file.is_file():
                    continue
                sim = compute_fuzzy_similarity(expected_name, sibling_file.name)
                if sibling_file.name.lower() == expected_name.lower() or sim >= 0.55:
                    sfk_info = rust_bridge.build_or_load_sfk_cache(sibling_file, num_buckets=96)
                    sfk_path_val = sfk_info.get("sfk_path", str(sibling_file) + ".sfk")
                    res_entry["resolvedPath"] = str(sibling_file.resolve())
                    res_entry["found"] = True
                    res_entry["matchStage"] = "Auto-Relink (Same Directory Sibling)"
                    res_entry["similarity"] = max(sim, 0.90)
                    res_entry["sfkPath"] = sfk_path_val
                    res_entry["sfkCachePath"] = sfk_path_val
                    res_entry["peaks"] = sfk_info.get("peaks", [])
                    break
            if res_entry["found"]:
                break

    found_count = sum(1 for r in results if r["found"])
    return {
        "ok": True,
        "searchDir": str(search_dir),
        "totalMissing": len(missing_items),
        "relinkedCount": found_count,
        "unresolvedCount": len(missing_items) - found_count,
        "results": results,
        "relinked": [r for r in results if r["found"]],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Neoveg 3-Stage Media Relinker CLI")
    parser.add_argument("--search-dir", required=True, help="Root directory to scan recursively")
    parser.add_argument("--missing-json", required=True, help="Path to JSON file listing missing items")
    args = parser.parse_args()

    root_dir = Path(__file__).resolve().parent.parent
    missing_items = json.loads(Path(args.missing_json).read_text(encoding="utf-8"))
    out = scan_and_relink_missing_media(root_dir, Path(args.search_dir), missing_items)
    print(json.dumps(out, ensure_ascii=False))


if __name__ == "__main__":
    main()
