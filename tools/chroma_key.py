"""Deterministic green-screen matte for the pink/white/blue homewear artwork.

Green-dominant edge pixels use the closest original opaque foreground color.
Opaque non-green pixels retain their original RGB values and coordinates.
Requires Pillow, NumPy and SciPy; these are build-time art tools, not app dependencies.
"""

import numpy as np
from PIL import Image, ImageFilter
from scipy.ndimage import distance_transform_edt


def extract_green(image):
    source = np.asarray(image.convert("RGBA"))
    height, width = source.shape[:2]
    size = min(32, height // 4, width // 4)
    if size < 1:
        raise ValueError("Green-screen input is too small")
    corners = np.concatenate([
        source[:size, :size].reshape(-1, 4),
        source[:size, -size:].reshape(-1, 4),
        source[-size:, :size].reshape(-1, 4),
        source[-size:, -size:].reshape(-1, 4),
    ])
    background = np.median(corners[:, :3], axis=0).astype(np.float32)
    screen_excess = float(background[1] - max(background[0], background[2]))
    if screen_excess < 100 or np.any(corners[:, 3] != 255):
        raise ValueError("Expected an opaque green screen with clear green corners")
    rgb = source[:, :, :3].astype(np.float32)
    excess = np.maximum(rgb[:, :, 1] - np.maximum(rgb[:, :, 0], rgb[:, :, 2]), 0)
    alpha = np.clip(1 - excess / screen_excess, 0, 1)
    # Measured screen noise must not become isolated low-alpha speckles.
    alpha[excess >= screen_excess - 6] = 0
    alpha[alpha <= 3 / 255] = 0
    alpha_byte = np.rint(alpha * 255).astype(np.uint8)
    # The generated screen contains darker corner noise. Keep soft pixels only
    # within three source pixels of a solid silhouette, including fine hair edges.
    support = Image.fromarray((alpha_byte >= 128).astype(np.uint8) * 255)
    near_subject = np.asarray(support.filter(ImageFilter.MaxFilter(7))) > 0
    alpha_byte[~near_subject] = 0
    result = np.zeros_like(source)
    opaque = alpha_byte == 255
    partial = (alpha_byte > 0) & ~opaque
    if not np.any(opaque):
        raise ValueError("Input has no opaque foreground")
    # Estimate coverage from the nearest source foreground color. Solving against
    # that color avoids converting green contamination into a yellow/white halo.
    nearest = distance_transform_edt(~opaque, return_distances=False, return_indices=True)
    foreground = rgb[nearest[0][partial], nearest[1][partial]]
    direction = foreground - background
    coverage = np.sum((rgb[partial] - background) * direction, axis=1) / np.sum(direction ** 2, axis=1)
    alpha_byte[partial] = np.rint(np.clip(coverage, 0, 1) * 255).astype(np.uint8)
    result[opaque, :3] = source[opaque, :3]
    result[partial, :3] = foreground.astype(np.uint8)
    result[:, :, 3] = alpha_byte
    result[alpha_byte == 0] = 0
    preserved = np.array_equal(result[opaque, :3], source[opaque, :3])
    if not preserved:
        raise ValueError("Opaque character RGB changed during extraction")
    return Image.fromarray(result), {
        "backgroundRgb": background.tolist(),
        "inputSize": [width, height],
        "outputSize": [width, height],
        "geometryTransform": "identity",
        "edgeColorMethod": "nearest original opaque foreground; projected alpha",
        "opaqueRgbPreserved": preserved,
        "opaquePixelCount": int(np.count_nonzero(opaque)),
        "edgePixelCount": int(np.count_nonzero(partial)),
        "transparentPixelCount": int(np.count_nonzero(alpha_byte == 0)),
    }
