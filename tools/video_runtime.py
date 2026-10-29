"""Resolve a local FFmpeg, with the optional imageio-ffmpeg wheel as fallback."""
import json
import shutil
import subprocess


def ffmpeg():
    executable = shutil.which("ffmpeg")
    if executable:
        return executable
    import imageio_ffmpeg
    return imageio_ffmpeg.get_ffmpeg_exe()


def video_info(source):
    probe = shutil.which("ffprobe")
    if probe:
        result = json.loads(subprocess.check_output([probe, "-v", "error", "-show_streams", "-show_format", "-of", "json", str(source)]))
        video = next(s for s in result["streams"] if s["codec_type"] == "video")
        numerator, denominator = map(int, video["avg_frame_rate"].split("/"))
        return {"fps": numerator / denominator, "size": [video["width"], video["height"]], "probe": result}
    import imageio_ffmpeg
    reader = imageio_ffmpeg.read_frames(str(source))
    try:
        metadata = next(reader)
    finally:
        reader.close()
    return {"fps": metadata["fps"], "size": list(metadata["size"]), "probe": metadata}
