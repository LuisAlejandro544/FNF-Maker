"""
Archivo: tools/video-studio/test_video_pipeline.py
Propósito: Suite de pruebas unitarias para video_pipeline_lib.py (inversión de
           silencios con padding seguro, generación de edit.json, cálculo de
           resoluciones y generación de subtítulos ASS palabra por palabra).
"""

import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))

from video_pipeline_lib import (
    TimeSegment,
    build_ass_subtitles_from_words,
    build_edit_plan,
    compute_target_dimensions,
    format_ass_timestamp,
    invert_silences_to_keep_segments,
)


class VideoPipelineLibTest(unittest.TestCase):
    def test_invert_silences_preserves_speech_padding_and_drops_long_dead_air(self) -> None:
        keeps = invert_silences_to_keep_segments(
            total_duration=20.0,
            silences=[(2.0, 5.0), (10.0, 10.3), (14.0, 18.0)],
            min_silence_duration=0.65,
            pad_before=0.12,
            pad_after=0.14,
        )
        # (10.0, 10.3) dura 0.3s < 0.65s -> no se corta
        self.assertEqual(len(keeps), 3)
        self.assertAlmostEqual(keeps[0].start, 0.0, places=2)
        self.assertAlmostEqual(keeps[0].end, 2.14, places=2)
        self.assertAlmostEqual(keeps[1].start, 4.88, places=2)
        self.assertAlmostEqual(keeps[1].end, 14.14, places=2)
        self.assertAlmostEqual(keeps[2].start, 17.88, places=2)
        self.assertAlmostEqual(keeps[2].end, 20.0, places=2)

    def test_build_ass_subtitles_generates_karaoke_events(self) -> None:
        self.assertEqual(format_ass_timestamp(65.25), "0:01:05.25")
        ass = build_ass_subtitles_from_words(
            [
                {
                    "start": 0.5,
                    "end": 1.4,
                    "text": "Hola mundo real",
                    "words": [
                        {"word": "Hola", "start": 0.5, "end": 0.8},
                        {"word": "mundo", "start": 0.8, "end": 1.1},
                        {"word": "real", "start": 1.1, "end": 1.4},
                    ],
                }
            ]
        )
        self.assertIn("PlayResX: 1080", ass)
        self.assertIn("PlayResY: 1920", ass)
        self.assertIn("Dialogue: 0,0:00:00.50,0:00:01.40,ReelCaption", ass)
        self.assertIn("{\\k", ass)

    def test_edit_plan_and_dimensions_validate_inputs(self) -> None:
        self.assertEqual(compute_target_dimensions("vertical", 1920, 1080), (1080, 1920))
        self.assertEqual(compute_target_dimensions("preserve", 1921, 1081), (1920, 1080))
        plan = build_edit_plan(
            source_path="/tmp/long.mp4",
            segments=[TimeSegment(0.0, 12.5, "intro")],
            aspect="vertical",
            bgm_path="/tmp/music.mp3",
            bgm_volume=0.15,
        )
        self.assertEqual(plan["aspect"], "vertical")
        self.assertEqual(plan["normalize_audio_lufs"], -14.0)
        self.assertEqual(plan["bgm"]["volume"], 0.15)
        with self.assertRaises(ValueError):
            build_edit_plan("/tmp/long.mp4", [], aspect="vertical")

    def test_stickman_choreography_and_audio_sync_20s(self) -> None:
        from stickman_fight_lib import (
            build_fight_choreography,
            compute_skeleton_points,
            sample_fight_state,
            synthesize_fight_soundtrack,
        )

        b_keys, r_keys, hp_keys, impacts, sounds = build_fight_choreography()
        self.assertGreaterEqual(len(impacts), 10)
        self.assertGreaterEqual(len(sounds), 20)
        b_mid, r_mid, b_hp_end, r_hp_end = sample_fight_state(20.0, b_keys, r_keys, hp_keys)
        self.assertGreater(b_hp_end, 0.0)
        self.assertEqual(r_hp_end, 0.0)
        pts = compute_skeleton_points(b_mid, ground_y=568.0)
        self.assertIn("hand_f", pts)
        self.assertIn("foot_f", pts)
        self.assertLess(pts["head"][1], pts["pelvis"][1])
        pcm = synthesize_fight_soundtrack(20.0, sounds, sr=48000)
        self.assertEqual(pcm.shape, (48000 * 20, 2))
        self.assertGreater(int(pcm.max()), 5000)


if __name__ == "__main__":
    unittest.main()
