"""Pack the approved Codex state and costume-change videos into transparent atlases."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

from chroma_key import extract_green
from video_runtime import ffmpeg


ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "artifacts/design/preprocessed"
SOURCE = ART / "source-videos/codex-form-v2"
OUTPUT = ART / "frames/runtime-codex-v2"
BUILTIN = ROOT / "apps/pet/assets/builtin"
CELL = (192, 208)
COLUMNS = 16
BASELINE = 200
ROI = (246, 48, 714, 518)
DECODE = (
    "scale=960:540:force_original_aspect_ratio=decrease,"
    "pad=960:540:(ow-iw)/2:(oh-ih)/2:color=0x00ff00,"
    f"crop={ROI[2] - ROI[0]}:{ROI[3] - ROI[1]}:{ROI[0]}:{ROI[1]}"
)


def first_cell(page):
    with Image.open(BUILTIN / page) as atlas:
        return atlas.crop((0, 0, *CELL)).convert("RGBA")


def solid_bounds(image):
    return image.getchannel("A").point(lambda a: 255 if a >= 128 else 0).getbbox()


def head_band(image, top=0):
    """Measure the head above all hand gestures, excluding isolated hair tips."""
    alpha = np.asarray(image.getchannel("A")) >= 128
    spans = [np.flatnonzero(row) for row in alpha[43 - top:66 - top]]
    return np.median(
        [(xs[-1] - xs[0] + 1, (xs[0] + xs[-1]) / 2) for xs in spans if xs.size],
        axis=0,
    )


def grounded_floor(cell, center, half_width=12):
    left = round(center) - half_width
    bounds = solid_bounds(cell.crop((left, BASELINE - 20, left + 2 * half_width, CELL[1])))
    return BASELINE - 20 + bounds[3] if bounds else None


def fit_neutral(image, neutral):
    reference_bounds = solid_bounds(neutral)
    bounds = solid_bounds(image)
    scale = (reference_bounds[3] - reference_bounds[1]) / (bounds[3] - bounds[1])
    size = (round(image.width * scale), round(image.height * scale))
    draft = image.resize(size, Image.Resampling.LANCZOS)
    top = BASELINE - solid_bounds(draft)[3]
    width, _ = head_band(draft, top)
    ref_width, ref_center = head_band(neutral)
    size = round(size[0] * ref_width / width), size[1]
    _, center = head_band(image.resize(size, Image.Resampling.LANCZOS), top)
    return size, (round(ref_center - center), top)


def resample_rgba(image, y, x):
    pixels = np.asarray(image.convert("RGBa"))
    result = np.empty_like(pixels)
    for channel in range(4):
        result[:, :, channel] = ndimage.map_coordinates(
            pixels[:, :, channel], (y, x), order=1, mode="constant"
        )
    return Image.frombytes("RGBa", image.size, result.tobytes()).convert("RGBA")


def stabilize_shoe_contact(cell, floor, center):
    """Correct the shoe area when the generated pose briefly sinks into the floor."""
    current_floor = grounded_floor(cell, center)
    if current_floor is None or abs(current_floor - floor) <= 1:
        return cell

    # Interpolate in premultiplied RGBA so the localized warp keeps soft edges.
    left, right = round(center) - 25, round(center) + 25
    top = floor - 24
    strip = cell.crop((left, top, right, CELL[1]))
    y, x = np.indices((strip.height, strip.width), dtype=np.float32)
    horizontal = np.clip(np.minimum(x, strip.width - 1 - x) / 10, 0, 1)
    vertical = np.clip(y / 15, 0, 1)
    displaced_y = y + (current_floor - floor) * horizontal * vertical
    cell.paste(resample_rgba(strip, displaced_y, x), (left, top))
    return cell


def key_effect(rgb):
    """Keep soft sky-blue light and sparks in the forward change."""
    rgb = rgb.astype(np.float32)
    background = np.median(rgb[:, :36], axis=(0, 1))
    r, g, b = np.moveaxis(rgb, -1, 0)
    screen_green = background[1] - max(background[0], background[2])
    coverage = np.clip(1 - (g - np.maximum(r, b)) / screen_green, 0, 1)
    screen_noise = np.max(np.abs(rgb - background), axis=2) < 14
    coverage[(coverage < 0.06) | screen_noise] = 0
    support = ndimage.binary_dilation(coverage > 0.45, iterations=4)
    coverage[~support] = 0
    foreground = (rgb - background * (1 - coverage[:, :, None])) / np.maximum(
        coverage[:, :, None], 1 / 255
    )
    rgba = np.dstack((np.clip(foreground, 0, 255), coverage * 255)).round().astype(np.uint8)
    rgba[rgba[:, :, 3] == 0] = 0
    return Image.fromarray(rgba)


def read_frames(name, effect=False, neutral=None):
    source = SOURCE / name
    raw = subprocess.check_output([
        ffmpeg(), "-v", "error", "-i", str(source), "-vf", DECODE,
        "-fps_mode", "passthrough", "-an", "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"
    ])
    frames = []
    transform = None
    decoded = np.frombuffer(raw, np.uint8).reshape(-1, ROI[3] - ROI[1], ROI[2] - ROI[0], 3)
    center = head_band(neutral)[1] if neutral is not None else None
    floor = grounded_floor(neutral, center) if neutral is not None else None
    if neutral is not None and floor is None:
        raise ValueError("The Codex reference has no solid shoe contact")
    for index, rgb in enumerate(decoded):
        image = key_effect(rgb) if effect and index > 0 else extract_green(Image.fromarray(rgb))[0]
        if transform is None:
            if neutral is not None:
                transform = fit_neutral(image, neutral)
            else:
                bounds = solid_bounds(image)
                scale = 184 / (bounds[3] - bounds[1])
                size = round(image.width * scale), round(image.height * scale)
                foot_center = 480 - ROI[0]
                offset = round(CELL[0] / 2 - foot_center * scale), BASELINE - round(bounds[3] * scale)
                transform = size, offset
        cell = Image.new("RGBA", CELL)
        cell.alpha_composite(image.resize(transform[0], Image.Resampling.LANCZOS), transform[1])
        if floor is not None:
            if index == 0:
                width_delta, center_delta = head_band(cell) - head_band(neutral)
                first_floor = grounded_floor(cell, center)
                if (
                    abs(width_delta) > 2 or abs(center_delta) > 1
                    or first_floor is None or abs(first_floor - floor) > 1
                ):
                    raise ValueError(
                        f"{name}: neutral head width {width_delta}, center {center_delta}, floor {first_floor}"
                    )
            cell = stabilize_shoe_contact(cell, floor, center)
        if effect:
            # Feather the generated sparks below the floor line into the
            # available space. The figure and its shoes end above this band.
            alpha = np.array(cell.getchannel("A"), dtype=np.float32)
            edge = np.minimum(np.arange(CELL[0]), np.arange(CELL[0] - 1, -1, -1))
            floor_fade = np.arange(CELL[1] - 1, -1, -1)
            alpha *= np.clip(edge / 4, 0, 1)[None, :] * np.clip(floor_fade / 7, 0, 1)[:, None]
            cell.putalpha(Image.fromarray(alpha.round().astype(np.uint8)))
        if not (bbox := cell.getbbox()) or not (0 < bbox[0] < bbox[2] < CELL[0] and 0 < bbox[1] < bbox[3] < CELL[1]):
            raise ValueError(f"{name} frame {index} reaches the cell boundary: {bbox}")
        frames.append(cell)
    print(f"Keyed {len(frames)} frames of {name}", flush=True)
    return frames


def blend(a, b, weight):
    left = np.asarray(a, dtype=np.float32)
    right = np.asarray(b, dtype=np.float32)
    left_alpha = left[:, :, 3] * (1 - weight)
    right_alpha = right[:, :, 3] * weight
    alpha = left_alpha + right_alpha
    rgb = (left[:, :, :3] * left_alpha[:, :, None] + right[:, :, :3] * right_alpha[:, :, None])
    rgb /= np.maximum(1, alpha[:, :, None])
    return Image.fromarray(np.dstack((rgb, alpha)).round().astype(np.uint8))


def align_standing_width(frames, neutral):
    """Match the near-idle hair and hem silhouette while keeping the face and shoes planted."""
    def width(image, top, bottom):
        bounds = solid_bounds(image.crop((0, top, CELL[0], bottom)))
        return bounds[2] - bounds[0]

    window = 10
    # Use several near-neutral frames to pick a constant correction for each edge.
    hair, hem = (
        np.clip(
            width(neutral, top, bottom) / np.median(
                [width(frame, top, bottom) for frame in frames[1:6]]
            ) - 1,
            0,
            0.05,
        )
        for top, bottom in ((20, 70), (190, 201))
    )
    y, x = np.indices((CELL[1], CELL[0]), dtype=np.float32)
    relative_x = x - (CELL[0] - 1) / 2
    outer = np.clip((abs(relative_x) - 18) / 12, 0, 1)
    width_adjustment = outer * (
        hair * np.clip((94 - y) / 24, 0, 1) + hem * np.clip((y - 147) / 42, 0, 1)
    )
    mask = Image.fromarray(np.uint8(width_adjustment > 0) * 255)
    corrected = list(frames)
    for index in range(1, min(len(frames), window)):
        t = np.clip((window - index) / (window - 4), 0, 1)
        gain = width_adjustment * (t * t * (3 - 2 * t))
        source_x = x - relative_x * gain / (1 + gain)
        warped = resample_rgba(frames[index], y, source_x)
        corrected[index] = Image.composite(warped, frames[index], mask)
    return corrected


def start_at(frames, pose, length=5, align_width=False):
    if align_width:
        frames = align_standing_width(frames, pose)
    return [pose, *(blend(pose, frames[i], i / length) for i in range(1, length)), *frames[length:]]


def end_at(frames, pose, length=5, align_width=False):
    return start_at(frames[::-1], pose, length, align_width)[::-1]


def idle_loop(frames):
    return end_at(frames, frames[0], length=4)


def sustained(frames, start, hold, end, neutral, exit_frames=None, *, align_width=False):
    loop = idle_loop(frames[hold:end])
    enter = start_at(frames[start:hold] + [loop[0]], neutral, align_width=align_width)
    if exit_frames is None:
        leave = enter[::-1]
    else:
        leave = end_at([loop[0], *exit_frames], neutral, align_width=align_width)
    return enter, loop, leave


class Atlas:
    def __init__(self, filename, clips):
        self.filename = filename
        self.clips = clips
        self.frames = []

    def add(self, state, frames, fps=24, looping=False):
        start = len(self.frames)
        self.clips[state] = {
            "image": self.filename, "row": start // COLUMNS, "column": start % COLUMNS,
            "frames": len(frames), "fps": fps, "looping": looping,
        }
        self.frames.extend(frames)

    def add_sustained(self, state, takes, exit_fps=48):
        enter, loop, leave = takes
        self.add(state + "-enter", enter)
        self.add(state, loop, looping=True)
        self.add(state + "-exit", leave, fps=exit_fps)

    def save(self):
        page = Image.new("RGBA", (CELL[0] * COLUMNS, CELL[1] * ((len(self.frames) + COLUMNS - 1) // COLUMNS)))
        if max(page.size) > 4096:
            raise ValueError(f"{self.filename}: exceeds texture dimensions ({page.size})")
        for index, frame in enumerate(self.frames):
            page.paste(frame, (index % COLUMNS * CELL[0], index // COLUMNS * CELL[1]))
        page.save(OUTPUT / self.filename, lossless=True, method=4)
        print(f"Packed {self.filename}: {len(self.frames)} frames, {page.size}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--install", action="store_true")
    args = parser.parse_args()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((BUILTIN / "musume.json").read_text(encoding="utf-8"))
    codex = next(form for form in manifest["forms"] if form["id"] == "codex")
    neutral = first_cell("codex-idle.png")
    homewear = first_cell("spritesheet.png")
    clips = {"idle": {**codex["clips"]["idle"], "image": "codex-idle.png"}}
    pages = []

    thinking = read_frames("02-thinking/thinking-original.mp4", neutral=neutral)
    page = Atlas("codex-thinking.webp", clips)
    # Source indices are zero-based. The hand starts lowering after frame 148.
    page.add_sustained(
        "thinking", sustained(thinking, 14, 59, 149, neutral, thinking[149:185], align_width=True)
    )
    pages.append(page)

    working = read_frames("03-working/working-original.mp4", neutral=neutral)
    waiting = read_frames("04-waiting/waiting-original.mp4", neutral=neutral)
    page = Atlas("codex-work.webp", clips)
    page.add_sustained("working", sustained(working, 9, 42, 118, neutral))
    page.add_sustained(
        "waiting-permission", sustained(waiting, 8, 45, 115, neutral, align_width=True)
    )
    pages.append(page)

    notification = read_frames("05-notification/notification-original.mp4", neutral=neutral)
    done = read_frames("06-done/done-original.mp4", neutral=neutral)
    page = Atlas("codex-reactions.webp", clips)
    wave = start_at(notification[5:101], neutral, align_width=True)
    page.add("notification", end_at(wave, neutral, align_width=True), fps=25)
    page.add("notification-still", [notification[36]], fps=1)
    # Keep the open-mouth smile after the nod, then finish in the shared neutral pose.
    page.add("done", end_at(start_at(done[14:105], neutral), neutral), fps=32)
    page.add("done-still", [done[62]], fps=1)
    pages.append(page)

    error = read_frames("07-error/error-original.mp4", neutral=neutral)
    sleeping = read_frames("08-sleeping/sleeping-original.mp4", neutral=neutral)
    page = Atlas("codex-rest.webp", clips)
    page.add_sustained("error", sustained(error, 11, 57, 113, neutral, align_width=True), exit_fps=60)
    page.add_sustained("sleeping", sustained(sleeping, 10, 45, 116, neutral, align_width=True))
    pages.append(page)

    change = {}
    into_codex = read_frames("costume-change-v2/homewear-to-codex-original.mp4", effect=True)
    out_of_codex = read_frames("costume-change-v2/codex-to-homewear-original.mp4")
    page = Atlas("costume-change.webp", change)
    page.add("to-codex", end_at(start_at(into_codex[27:101], homewear), neutral), fps=30)
    page.add("to-homewear", end_at(start_at(out_of_codex[26:102], neutral), homewear), fps=30)
    pages.append(page)

    for page in pages:
        page.save()
    for form in manifest["forms"]:
        if form["id"] in ("default", "claude"):
            form["clips"]["form-to-codex"] = change["to-codex"]
    codex["image"] = "codex-idle.png"
    codex["clips"] = {
        **clips, "form-to-default": change["to-homewear"], "form-to-claude": change["to-homewear"],
    }
    # The primary Codex atlas is selected implicitly for idle.
    del codex["clips"]["idle"]["image"]
    manifest_text = json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    (OUTPUT / "musume.json").write_text(manifest_text, encoding="utf-8", newline="\n")

    sheet = Image.new("RGB", (11 * 144, len(pages) * 180), "#f4f5f7")
    draw = ImageDraw.Draw(sheet)
    for row, page in enumerate(pages):
        draw.text((4, row * 180 + 4), page.filename, fill="black")
        for index in range(11):
            frame = page.frames[round(index * (len(page.frames) - 1) / 10)]
            size = frame.resize((144, 156), Image.Resampling.LANCZOS)
            sheet.paste(size, (index * 144, row * 180 + 20), size.getchannel("A"))
    sheet.save(OUTPUT / "contact-sheet.png")

    if args.install:
        for page in pages:
            shutil.copyfile(OUTPUT / page.filename, BUILTIN / page.filename)
        shutil.copyfile(OUTPUT / "musume.json", BUILTIN / "musume.json")


if __name__ == "__main__":
    main()
