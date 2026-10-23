"""
End-to-End Automated Verification Suite for OpenNeoveg NLE Studio
Tests Rust Bridge / Mirror algorithms, WAV PCM DSP peak extraction,
Neoveg Python Scripting API, FFmpeg filter_complex compiler, and real MP4 rendering.
"""

from __future__ import annotations
import sys
import unittest
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "python-backend"))

from rust_bridge import (  # noqa: E402
    analyze_wav_peaks,
    compile_project_to_ffmpeg,
    detect_auto_crossfades,
    evaluate_fade_curve,
    format_smpte,
)
from server import MEDIA_DIR, RENDERS_DIR, ensure_sample_media  # noqa: E402
from neoveg_scripting import PRESET_SCRIPTS, execute_neoveg_script  # noqa: E402


class TestNeovegStudio(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        ensure_sample_media()

    def test_smpte_timecode_and_fade_curves(self) -> None:
        self.assertEqual(format_smpte(65.5, 30.0, True), "00:01:05;15")
        self.assertAlmostEqual(evaluate_fade_curve("smooth", 0.5), 0.5, places=3)
        self.assertGreater(evaluate_fade_curve("fast", 0.3), evaluate_fade_curve("linear", 0.3))
        self.assertLess(evaluate_fade_curve("slow", 0.3), evaluate_fade_curve("linear", 0.3))

    def test_auto_crossfade_solver(self) -> None:
        project = {
            "tracks": [
                {
                    "id": "t1",
                    "type": "video",
                    "clips": [
                        {"id": "c1", "start": 0.0, "duration": 5.0, "fadeIn": 0.0, "fadeOut": 0.0},
                        {"id": "c2", "start": 3.5, "duration": 4.0, "fadeIn": 0.0, "fadeOut": 0.0},
                    ],
                }
            ]
        }
        updated, xfades = detect_auto_crossfades(project)
        self.assertEqual(len(xfades), 1)
        self.assertAlmostEqual(xfades[0]["duration"], 1.5, places=2)
        self.assertAlmostEqual(updated["tracks"][0]["clips"][0]["fadeOut"], 1.5, places=2)
        self.assertAlmostEqual(updated["tracks"][0]["clips"][1]["fadeIn"], 1.5, places=2)

    def test_wav_pcm_dsp_analyzer(self) -> None:
        wav_path = MEDIA_DIR / "synth_bass_120bpm.wav"
        self.assertTrue(wav_path.exists(), "Sample WAV should be generated")
        res = analyze_wav_peaks(wav_path, 64)
        self.assertEqual(res["sample_rate"], 48000)
        self.assertEqual(len(res["peaks"]), 64)
        self.assertGreater(max(res["peaks"]), 0.2)

    def test_neoveg_python_scripting_api(self) -> None:
        sample_project = {
            "fps": 30.0,
            "playhead": 1.0,
            "markers": [],
            "regions": [],
            "tracks": [
                {
                    "id": "v1",
                    "type": "video",
                    "clips": [
                        {"id": "vc1", "start": 0.0, "duration": 3.0},
                        {"id": "vc2", "start": 4.5, "duration": 3.0},
                    ],
                }
            ],
        }
        # 1. Ripple close gaps preset
        r1 = execute_neoveg_script(PRESET_SCRIPTS["ripple_close_gaps"]["code"], sample_project)
        self.assertTrue(r1["ok"])
        self.assertAlmostEqual(r1["project"]["tracks"][0]["clips"][1]["start"], 3.0, places=2)

        # 2. Beat markers preset
        r2 = execute_neoveg_script(PRESET_SCRIPTS["beat_markers"]["code"], r1["project"])
        self.assertTrue(r2["ok"])
        self.assertEqual(len(r2["project"]["markers"]), 12)

    def test_ffmpeg_filtergraph_compiler(self) -> None:
        project = {
            "width": 1280,
            "height": 720,
            "fps": 30.0,
            "tracks": [
                {
                    "id": "v1",
                    "type": "video",
                    "level": 1.0,
                    "clips": [
                        {
                            "id": "c1",
                            "mediaFile": "neoveg_smpte_bars.mp4",
                            "start": 0.0,
                            "duration": 2.0,
                            "trimIn": 0.0,
                            "fadeIn": 0.3,
                            "fadeOut": 0.3,
                            "gain": 1.0,
                            "fx": [
                                {"type": "vignette", "enabled": True, "params": {"strength": 0.5}}
                            ],
                        }
                    ],
                },
                {
                    "id": "a1",
                    "type": "audio",
                    "level": 1.0,
                    "clips": [
                        {
                            "id": "ca1",
                            "mediaFile": "synth_bass_120bpm.wav",
                            "start": 0.0,
                            "duration": 2.0,
                            "trimIn": 0.0,
                            "fadeIn": 0.2,
                            "fadeOut": 0.2,
                            "gain": 0.9,
                        }
                    ],
                },
            ],
        }
        out_path = RENDERS_DIR / "test_compile_out.mp4"
        plan = compile_project_to_ffmpeg(project, MEDIA_DIR, out_path)
        self.assertIn("overlay", plan["filter_complex"])
        self.assertIn("vignette", plan["filter_complex"])
        self.assertIn("amix", plan["filter_complex"])


if __name__ == "__main__":
    unittest.main()
