"""Regression checks for background removal without regenerating the character."""

import unittest

import numpy as np
from PIL import Image

from chroma_key import extract_green


class ChromaKeyTests(unittest.TestCase):
    def test_preserves_subject_coordinates_and_opaque_rgb(self):
        pixels = np.full((64, 80, 4), [2, 249, 2, 255], dtype=np.uint8)
        pixels[12:52, 25:55] = [250, 183, 210, 255]
        pixels[18:25, 32:45] = [205, 229, 250, 255]
        keyed, report = extract_green(Image.fromarray(pixels))
        output = np.asarray(keyed)
        np.testing.assert_array_equal(output[12:52, 25:55], pixels[12:52, 25:55])
        self.assertEqual(keyed.getbbox(), (25, 12, 55, 52))
        self.assertEqual(report["geometryTransform"], "identity")
        self.assertTrue(report["opaqueRgbPreserved"])

    def test_recovers_soft_edge_without_green_spill_or_screen_noise(self):
        pixels = np.full((64, 80, 4), [2, 250, 2, 255], dtype=np.uint8)
        pixels[12:52, 25:55] = [100, 100, 100, 255]
        pixels[12:52, 24] = [51, 175, 51, 255]
        pixels[2, 30] = [3, 248, 3, 255]
        pixels[0, 0] = [16, 236, 14, 255]
        output = np.asarray(extract_green(Image.fromarray(pixels))[0])
        self.assertEqual(int(output[2, 30, 3]), 0)
        self.assertEqual(int(output[0, 0, 3]), 0)
        self.assertLessEqual(abs(int(output[20, 24, 3]) - 128), 2)
        self.assertLessEqual(int(output[20, 24, :3].max()) - int(output[20, 24, :3].min()), 2)
        self.assertTrue(np.all(output[:12, :, 3] == 0))

    def test_rejects_wrong_background_and_already_transparent_inputs(self):
        for color in ((255, 255, 255, 255), (0, 255, 0, 0)):
            with self.assertRaises(ValueError):
                extract_green(Image.new("RGBA", (64, 64), color))


if __name__ == "__main__":
    unittest.main()
