"""compose.py <take.mov> <recorder output> <scene log> <out.mp4>

Intermediate images (background, mask, captions) go next to the take.

Cuts a recorded take to its scene (marks from the scene log, first-frame
time from the recorder), puts the window on the background with rounded
corners, adds the captions, and encodes an App Store app preview:
1920 × 1080, 30 fps, H.264 High 4.0, ~11 Mbps, stereo AAC 256 kbps (silent).
"""
import re
import subprocess
import sys
import urllib.parse
from pathlib import Path

take, rec_out, scene_log, out = sys.argv[1:5]
here = Path(__file__).parent
work = Path(take).resolve().parent
chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"


def render(query: str, size: str, png: Path) -> None:
    subprocess.run([chrome, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
                    "--default-background-color=00000000", f"--user-data-dir={work / 'chrome'}",
                    f"--window-size={size}", f"--screenshot={png}",
                    f"file://{here / 'compose-assets.html'}?{query}"],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)


if not (work / "bg.png").exists():
    render("kind=bg", "1920,1080", work / "bg.png")
if not (work / "mask.png").exists():
    render("kind=mask", "1664,936", work / "mask.png")
first = int(re.search(r"firstframe (\d+)", Path(rec_out).read_text()).group(1))
marks = re.findall(r"rupert-test: mark (.+) (\d+)$", Path(scene_log).read_text(), re.M)
times = [(name, (int(ms) - first) / 1000) for name, ms in marks]
start = dict(times)["start"] - 0.6
end = dict(times)["end"] + 0.4
dur = end - start
assert 15 <= dur <= 30, f"preview must be 15–30 s, this is {dur:.1f}"

caps = [(n[4:], t - start) for n, t in times if n.startswith("cap:")]
spans = [(text, max(0.0, a), (caps[i + 1][1] if i + 1 < len(caps) else dur)) for i, (text, a) in enumerate(caps)]

inputs = ["-loop", "1", "-t", f"{dur}", "-i", str(work / "bg.png"),
          "-ss", f"{start:.3f}", "-t", f"{dur:.3f}", "-i", take,
          "-loop", "1", "-t", f"{dur}", "-i", str(work / "mask.png")]
for i, (text, _, _) in enumerate(spans):
    png = work / f"caption-{i}.png"
    render(f"kind=caption&text={urllib.parse.quote(text)}", "1920,120", png)
    inputs += ["-loop", "1", "-t", f"{dur}", "-i", str(png)]
inputs += ["-f", "lavfi", "-t", f"{dur}", "-i", "anullsrc=channel_layout=stereo:sample_rate=48000"]

# The window is 1664 × 936 points: a Retina screen records it at twice that.
f = ["[1:v]fps=30,scale=1664:936:flags=lanczos,format=rgba[w]", "[2:v]format=gray[m]", "[w][m]alphamerge[wa]",
     "[0:v][wa]overlay=128:120:shortest=1[v0]"]
last = "v0"
for i, (text, a, b) in enumerate(spans):
    fi = 3 + i
    fade_out = max(a + 0.4, b - 0.35)
    f.append(f"[{fi}:v]format=rgba,fade=t=in:st={a:.2f}:d=0.35:alpha=1,fade=t=out:st={fade_out:.2f}:d=0.35:alpha=1[c{i}]")
    f.append(f"[{last}][c{i}]overlay=0:0:enable='between(t,{a:.2f},{b:.2f})'[v{i + 1}]")
    last = f"v{i + 1}"
f.append(f"[{last}]fade=t=in:st=0:d=0.4,fade=t=out:st={dur - 0.5:.2f}:d=0.5,format=yuv420p[vout]")
audio = 3 + len(spans)

cmd = ["ffmpeg", "-v", "error", "-y", *inputs, "-filter_complex", ";".join(f),
       "-map", "[vout]", "-map", f"{audio}:a", "-t", f"{dur:.3f}",
       "-c:v", "libx264", "-profile:v", "high", "-level:v", "4.0", "-pix_fmt", "yuv420p", "-r", "30",
       "-b:v", "11M", "-minrate", "10M", "-maxrate", "12M", "-bufsize", "12M", "-x264-params", "nal-hrd=cbr",
       "-c:a", "aac", "-b:a", "256k", "-ar", "48000", "-ac", "2", "-movflags", "+faststart", out]
subprocess.run(cmd, check=True)
print(f"{out}: {dur:.1f} s, captions: " + " | ".join(f"{a:.1f}-{b:.1f} {t}" for t, a, b in spans))
