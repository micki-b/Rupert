#!/bin/bash
# preview.sh <lang> <question> <out.mp4>: an App Preview of Rupert on the "Rupert Media" simulator
# (run capture.sh for the language first: it leaves the app installed with the library and language set).
set -u
S=$(cd "$(dirname "$0")" && pwd); REPO=$(cd "$S/../../../.." && pwd); BID=io.github.micki-b.rupert
# The "Rupert Media" simulator (iPhone 17 Pro Max: 1320 × 2868 screenshots), made once.
D=$(xcrun simctl list devices | grep "Rupert Media (" | head -1 | grep -oE "[0-9A-F-]{36}")
[ -z "$D" ] && D=$(xcrun simctl create "Rupert Media" com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro-Max)
TMP=$(mktemp -d)
# Ask Rupert in the Simulator (it can't run Apple's model): an OpenAI-compatible server on the Mac.
ASSISTANT=${RUPERT_MEDIA_ASSISTANT:-http://localhost:1234|qwen/qwen3.6-35b-a3b}
LANG_=$1; Q=$2; OUT=$3; W=$TMP
C=$(xcrun simctl get_app_container $D $BID data); LIB="$C/Documents/Home Documents"
# The app in this language first (no recording).
setup=$(sed -e "s/__LANG__/$LANG_/" $S/setup.js)
xcrun simctl spawn $D log stream --style compact --predicate 'process == "Rupert"' > $W/setup.txt 2>&1 &
SP=$!; sleep 2
SIMCTL_CHILD_RUPERT_TEST_FOLDER="$LIB" SIMCTL_CHILD_RUPERT_TEST_INBOX="$LIB/Inbox" SIMCTL_CHILD_RUPERT_TEST_JS="$setup" \
  xcrun simctl launch --terminate-running-process $D $BID > /dev/null 2>&1
for i in $(seq 1 120); do grep -q "rupert-test: ready" $W/setup.txt && break; sleep 1; done
xcrun simctl terminate $D $BID 2>/dev/null; kill $SP 2>/dev/null
js=$(sed -e "s|__QUESTION__|$Q|" $S/preview.js)
xcrun simctl spawn $D log stream --style compact --predicate 'process == "Rupert"' > $W/log.txt 2>&1 &
LOGPID=$!; sleep 2
xcrun simctl terminate $D $BID 2>/dev/null
xcrun simctl io $D recordVideo --codec=h264 --force $W/take.mov > $W/rec.txt 2>&1 &
RECPID=$!
# Time zero is when the recorder says it started.
for i in $(seq 1 100); do grep -q "Recording started" $W/rec.txt && break; sleep 0.05; done
REC0=$(python3 -c 'import time;print(time.time())'); sleep 1
SIMCTL_CHILD_RUPERT_TEST_FOLDER="$LIB" SIMCTL_CHILD_RUPERT_TEST_INBOX="$LIB/Inbox" \
SIMCTL_CHILD_RUPERT_TEST_ASSISTANT="$ASSISTANT" \
SIMCTL_CHILD_RUPERT_TEST_JS="$js" xcrun simctl launch --terminate-running-process $D $BID > /dev/null 2>&1
for i in $(seq 1 300); do grep -q "rupert-test: done" $W/log.txt && break; sleep 1; done
sleep 1; kill -INT $RECPID; wait $RECPID 2>/dev/null; xcrun simctl terminate $D $BID 2>/dev/null; kill $LOGPID 2>/dev/null
python3 - "$W" "$REC0" "$OUT" <<'PY'
import sys, re, subprocess, datetime
w, rec0, out = sys.argv[1], float(sys.argv[2]), sys.argv[3]
marks = {}
for line in open(f"{w}/log.txt"):
    m = re.search(r"^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d\.\d+).*rupert-test: mark (\w+)", line)
    if m:
        t = datetime.datetime.strptime(m.group(1), "%Y-%m-%d %H:%M:%S.%f").timestamp()
        marks[m.group(2)] = t - rec0
print("marks", {k: round(v, 1) for k, v in marks.items()})
s, a, b, e = marks["start"], marks["asked"], marks["answered"], marks["end"]
keep_think = 1.5  # seconds of "thinking" kept before the jump to the answer
p1, p2 = a + keep_think - s, e - (b - 0.3)
if p1 + p2 > 29.5:  # Apple's limit is 30 s: start a little later, then end earlier
    s += min(p1 + p2 - 29.5, 2.0)
    e -= max(0, (a + keep_think - s) + p2 - 29.5)
parts = [(s, a + keep_think), (b - 0.3, e)]
total = sum(y - x for x, y in parts)
print("length", round(total, 1))
f = ";".join(f"[0:v]trim={x:.3f}:{y:.3f},setpts=PTS-STARTPTS,fps=30[v{i}]" for i, (x, y) in enumerate(parts))
f += f";{''.join(f'[v{i}]' for i in range(len(parts)))}concat=n={len(parts)}:v=1:a=0,scale=886:-2,crop=886:1920:0:(ih-1920)/2,format=yuv420p[v]"
# The simulator records only when the screen changes: make the take a steady
# 30 fps first, so still stretches (Thinking…) keep their length when cut.
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", f"{w}/take.mov", "-vf", "fps=30",
  "-fps_mode", "cfr", "-c:v", "libx264", "-crf", "12", "-preset", "fast", f"{w}/take30.mp4"], check=True)
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", f"{w}/take30.mp4",
  "-f", "lavfi", "-i", "anullsrc=channel_layout=stereo:sample_rate=44100",
  "-filter_complex", f, "-map", "[v]", "-map", "1:a", "-shortest",
  "-c:v", "libx264", "-profile:v", "high", "-level", "4.0", "-r", "30", "-b:v", "8M",
  "-c:a", "aac", "-b:a", "128k", "-movflags", "+faststart", out], check=True)
PY
ffprobe -v error -show_entries stream=codec_name,width,height,r_frame_rate:format=duration -of compact "$OUT"
