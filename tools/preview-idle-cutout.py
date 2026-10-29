"""Verify the exported RGBA sequence and render a checkerboard viewing copy."""

import argparse
import json
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image


root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--input", type=Path, default=root / "artifacts/design/preprocessed/frames/workbuddy-idle-cutout-v1")
output = parser.parse_args().input
paths = sorted((output / "frames").glob("[0-9][0-9][0-9][0-9].png"))
source = root / "artifacts/design/preprocessed/source-videos/workbuddy-idle-v1/idle-original.mp4"
probe = json.loads(subprocess.check_output([
    "ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
    "stream=nb_frames,avg_frame_rate", "-of", "json", str(source)
]))["streams"][0]
expected_frames = int(probe["nb_frames"])
assert len(paths) == expected_frames
rate = probe["avg_frame_rate"]
with Image.open(paths[0]) as first:
    width, height = first.size
y, x = np.indices((height, width))
shades = np.where((x // 24 + y // 24) % 2, 215, 245).astype(np.uint8)
checker = Image.fromarray(np.repeat(shades[:, :, None], 3, axis=2)).convert("RGBA")
encoder = subprocess.Popen([
    "ffmpeg", "-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24",
    "-s", f"{width}x{height}", "-r", rate, "-i", "pipe:0", "-an",
    "-c:v", "libx264", "-crf", "18", "-preset", "fast", "-pix_fmt", "yuv420p",
    "-movflags", "+faststart", str(output / "idle-checker-preview.mp4"),
], stdin=subprocess.PIPE)
face_min = 255
preview_frames = []
master_frames = []
try:
    for index, path in enumerate(paths):
        assert path.name == f"{index:04d}.png", "Non-contiguous frame sequence"
        with Image.open(path) as image:
            assert image.mode == "RGBA" and image.size == (width, height)
            pixels = np.asarray(image)
            alpha = pixels[:, :, 3]
            assert alpha.min() == 0 and alpha.max() == 255, path.name
            assert not any((alpha[0].any(), alpha[-1].any(), alpha[:, 0].any(), alpha[:, -1].any())), path.name
            rgb = pixels[:, :, :3].astype(np.int16)
            residue = (alpha > 32) & (rgb[:, :, 1] - np.maximum(rgb[:, :, 0], rgb[:, :, 2]) > 45)
            assert not residue.any(), f"Green residue: {path.name}"
            face = alpha[round(height * 0.24):round(height * 0.35), round(width * 0.33):round(width * 0.67)]
            face_min = min(face_min, int(face.min()))
            assert face.min() >= 250, f"Transparent hole in the face inspection region: {path.name}"
            composed = Image.alpha_composite(checker, image).convert("RGB")
            encoder.stdin.write(composed.tobytes())
            preview_frames.append(image.resize((width // 2, height // 2), Image.Resampling.LANCZOS))
            master_frames.append(image.copy())
            if index == 0:
                image.save(output / "first-frame-transparent.png")
                composed.save(output / "preview-poster.png")
finally:
    encoder.stdin.close()
    code = encoder.wait()
assert code == 0
rate_n, rate_d = map(int, rate.split("/"))
fps = rate_n / rate_d
durations = [round((i + 1) * 1000 / fps) - round(i * 1000 / fps) for i in range(len(preview_frames))]
print(f"All {len(paths)} RGBA frames passed; encoding transparent animations.", flush=True)
master_frames[0].save(
    output / "idle-transparent.webp", save_all=True,
    append_images=master_frames[1:], duration=durations, loop=0,
    lossless=True, method=1, exact=True, allow_mixed=False,
)
preview_frames[0].save(
    output / "idle-transparent-preview.webp", save_all=True,
    append_images=preview_frames[1:], duration=durations, loop=0,
    lossless=True, method=1, exact=True, allow_mixed=False,
)
report = {
    "status": "passed",
    "frames": len(paths), "frameSize": [width, height], "frameRate": rate,
    "contiguousSequence": True, "rgbaEveryFrame": True,
    "transparentBordersEveryFrame": True, "greenResiduePixels": 0,
    "faceInspectionRegionAlphaMinimum": face_min,
    "previewFile": "idle-checker-preview.mp4",
    "transparentViewingCopy": "idle-transparent-preview.webp",
    "viewingCopySize": [width // 2, height // 2],
    "viewingCopyColorEncoding": "lossless",
    "previewHasRealTransparency": False,
    "note": "MP4 checkerboard is a viewing copy. Use RGBA PNGs or the transparent WebP as artwork. This verifies matting, not seamless looping or model identity stability.",
}
(output / "qa.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, indent=2))
