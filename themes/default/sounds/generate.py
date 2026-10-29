#!/usr/bin/env python3
"""Generates the default theme sounds: plain sine tones with short envelopes.
Usage: python3 generate.py   (needs ffmpeg with libvorbis)."""
import math
import struct
import subprocess
import wave
from pathlib import Path


RATE = 44100
OUT = Path(__file__).parent


def tone(freq, length, gain=1.0, attack=0.004, decay=None, harmonic=0.0):
    decay = decay or length / 4.0
    samples = []
    for n in range(int(RATE * length)):
        t = n / RATE
        value = math.sin(2 * math.pi * freq * t) + harmonic * math.sin(4 * math.pi * freq * t)
        envelope = min(t / attack, 1.0) * math.exp(-t / decay) * min((length - t) / 0.01, 1.0)
        samples.append(gain * value * envelope / (1.0 + harmonic))
    return samples


def sequence(*notes):
    """Notes are (start seconds, samples) pairs, mixed together."""
    mix = [0.0] * (max(int(RATE * start) + len(samples) for start, samples in notes) + 1)
    for start, samples in notes:
        offset = int(RATE * start)
        for i, value in enumerate(samples):
            mix[offset + i] += value
    return mix


SOUNDS = {
    "move": tone(1200, 0.035, 0.22, decay=0.012),
    "page": sequence((0, tone(900, 0.05, 0.25, decay=0.02)), (0.04, tone(1350, 0.06, 0.25, decay=0.025))),
    "select": sequence((0, tone(660, 0.09, 0.3, decay=0.04, harmonic=0.2)), (0.05, tone(990, 0.12, 0.3, decay=0.05, harmonic=0.2))),
    "back": sequence((0, tone(880, 0.07, 0.28, decay=0.03)), (0.05, tone(587, 0.12, 0.28, decay=0.05))),
    "launch": sequence(*[(i * 0.07, tone(f, 0.25, 0.26, decay=0.1, harmonic=0.25)) for i, f in enumerate([523, 659, 784, 1047])]),
    "menu_open": sequence((0, tone(700, 0.08, 0.26, decay=0.03)), (0.06, tone(1050, 0.12, 0.26, decay=0.05))),
    "menu_close": sequence((0, tone(1050, 0.07, 0.26, decay=0.03)), (0.05, tone(700, 0.11, 0.26, decay=0.05))),
    "toggle": tone(1000, 0.06, 0.28, decay=0.025, harmonic=0.3),
    "error": sequence((0, tone(247, 0.1, 0.3, decay=0.06, harmonic=0.4)), (0.12, tone(196, 0.16, 0.3, decay=0.08, harmonic=0.4))),
    "notice": sequence((0, tone(1175, 0.18, 0.25, decay=0.07)), (0.1, tone(1568, 0.25, 0.25, decay=0.1))),
    "favorite": sequence(*[(i * 0.06, tone(f, 0.2, 0.24, decay=0.08, harmonic=0.15)) for i, f in enumerate([784, 988, 1319])]),
    "boot": sequence(*[(i * 0.11, tone(f, 0.5, 0.22, attack=0.01, decay=0.2, harmonic=0.2)) for i, f in enumerate([392, 494, 587, 784])]),
}

for name, samples in SOUNDS.items():
    peak = max(max(abs(v) for v in samples), 1.0)
    pcm = struct.pack(f"<{len(samples)}h", *(int(v / peak * 32767 * 0.9) for v in samples))
    wav = OUT / f"{name}.wav"
    with wave.open(str(wav), "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        out.writeframes(pcm)
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-i", str(wav), "-c:a", "libvorbis", "-q:a", "3", str(OUT / f"{name}.ogg")],
        check=True,
    )
    wav.unlink()
