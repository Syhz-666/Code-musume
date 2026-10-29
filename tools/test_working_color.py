"""Guard the green-to-gray regression without requiring archived video files."""
import unittest

import numpy as np
from PIL import Image

from working_color import correct_lower_body, recover_green

PALETTES = {"trousers": [254, 250, 248], "cuffs": [254, 216, 219], "skin": [254, 238, 229]}


class WorkingColorTests(unittest.TestCase):
    def test_recovers_white_and_skin_brightness_from_green_mixture(self):
        for key in ("trousers", "skin"):
            foreground = np.array(PALETTES[key], dtype=float)
            for coverage in (.55, .75, .9):
                mixed = foreground * coverage + np.array([0, 255, 0]) * (1 - coverage)
                restored, weight = recover_green(mixed, foreground)
                np.testing.assert_allclose(restored, foreground, atol=.001)
                self.assertEqual(float(weight), 1)

    def test_real_gray_stain_regression_restores_brightness(self):
        restored, weight = recover_green([196, 251, 216], PALETTES["trousers"])
        self.assertEqual(float(weight), 1)
        self.assertTrue(np.all(restored >= 240), restored)
        self.assertGreater(restored.mean(), 245)

    def test_preserves_alpha_upper_body_and_clean_cuffs_and_folds(self):
        raw = np.zeros((768, 384, 4), dtype=np.uint8)
        raw[50:500, 110:280] = [70, 200, 140, 255]
        raw[510:686, 110:280] = [254, 250, 248, 255]
        raw[686:715, 110:280] = [254, 216, 219, 255]
        raw[715:740, 110:280] = [254, 238, 229, 255]
        raw[540:550, 120:140] = [180, 171, 169, 255]
        raw[600:630, 140:170] = [196, 251, 216, 255]
        raw[650, 110] = [196, 251, 216, 72]
        baseline = raw.copy()
        baseline[600:630, 140:170, :3] = 196
        restored, _ = correct_lower_body(Image.fromarray(raw), Image.fromarray(baseline), PALETTES)
        result = np.asarray(restored)
        np.testing.assert_array_equal(result[:, :, 3], baseline[:, :, 3])
        np.testing.assert_array_equal(result[:510], baseline[:510])
        np.testing.assert_array_equal(result[686:715], baseline[686:715])
        np.testing.assert_array_equal(result[540:550], baseline[540:550])
        self.assertTrue(np.all(result[600:630, 140:170, :3] >= 240))


if __name__ == "__main__":
    unittest.main()
