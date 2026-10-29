#!/usr/bin/env python3
"""Chiptune interface sounds: square, triangle and noise voices with short
envelopes, synthesized from scratch (no samples). One set per theme.

Usage: python3 sons.py <retro|praia> <out-dir>   (needs ffmpeg with libvorbis)
Writes <event>.ogg for every launcher event and systems/<name>.ogg jingles.
"""
import math
import random
import struct
import subprocess
import sys
import wave
from pathlib import Path

RATE = 22050


def note(name):
    """'C5', 'F#4' ... to Hz."""
    names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
    pitch, octave = name[:-1], int(name[-1])
    return 440.0 * 2 ** ((names.index(pitch) + 12 * (octave + 1) - 69) / 12)


def voice(kind, freq, length, gain=0.3, duty=0.5, slide=0.0, decay=None, vibrato=0.0):
    """One voice; `slide` bends the pitch by that many octaves over the note."""
    decay = decay or length
    out, phase = [], 0.0
    noise_state, noise_value = 0, 1.0
    rnd = random.Random(int(freq))
    for n in range(int(RATE * length)):
        t = n / RATE
        f = freq * 2 ** (slide * t / length) * (1 + vibrato * math.sin(2 * math.pi * 6 * t))
        phase = (phase + f / RATE) % 1.0
        if kind == "square":
            v = 1.0 if phase < duty else -1.0
        elif kind == "triangle":
            # Four-bit stepped triangle, like the old sound chips.
            v = round((4 * abs(phase - 0.5) - 1) * 7) / 7
        else:
            noise_state += 1
            if noise_state >= max(1, int(RATE / f)):
                noise_state, noise_value = 0, rnd.choice([-1.0, 1.0])
            v = noise_value
        envelope = min(1.0, t / 0.002) * max(0.0, 1 - t / decay) * min(1.0, (length - t) / 0.004)
        out.append(gain * v * envelope)
    return out


def mix(*parts):
    """(start seconds, samples) pairs added together."""
    end = max(int(RATE * start) + len(s) for start, s in parts)
    out = [0.0] * end
    for start, s in parts:
        o = int(RATE * start)
        for i, v in enumerate(s):
            out[o + i] += v
    return out


def arpeggio(kind, notes, step, length, gain=0.25, duty=0.5):
    return mix(*[(i * step, voice(kind, note(n), length, gain, duty)) for i, n in enumerate(notes)])


def retro():
    """8-bit console: bright pulse waves."""
    sq = "square"
    return {
        "move": voice(sq, note("A5"), 0.03, 0.18, 0.25),
        "page": mix((0, voice(sq, note("E5"), 0.035, 0.2, 0.25)), (0.04, voice(sq, note("A5"), 0.04, 0.2, 0.25))),
        "select": mix((0, voice(sq, note("C5"), 0.05, 0.25, 0.5)), (0.05, voice(sq, note("G5"), 0.08, 0.25, 0.5))),
        "back": mix((0, voice(sq, note("G5"), 0.05, 0.22, 0.5)), (0.05, voice(sq, note("C5"), 0.08, 0.22, 0.5))),
        "launch": mix((0, arpeggio(sq, ["C5", "E5", "G5", "C6"], 0.06, 0.12, 0.22, 0.25)),
                      (0.24, voice("triangle", note("C4"), 0.25, 0.35))),
        "menu_open": voice(sq, note("D5"), 0.09, 0.2, 0.125, slide=1.0),
        "menu_close": voice(sq, note("D6"), 0.09, 0.2, 0.125, slide=-1.0),
        "toggle": mix((0, voice(sq, note("B5"), 0.025, 0.2, 0.25)), (0.03, voice(sq, note("E6"), 0.03, 0.2, 0.25))),
        "error": mix((0, voice(sq, note("D3"), 0.12, 0.25, 0.5)), (0.13, voice(sq, note("C#3"), 0.18, 0.25, 0.5))),
        "notice": arpeggio(sq, ["E6", "B5", "E6"], 0.07, 0.07, 0.18, 0.25),
        "favorite": arpeggio(sq, ["G5", "B5", "D6", "G6"], 0.045, 0.06, 0.18, 0.25),
        "boot": mix((0, arpeggio(sq, ["C5", "E5", "G5", "C6", "E6", "G6"], 0.07, 0.1, 0.2, 0.25)),
                    (0.42, voice("triangle", note("C4"), 0.5, 0.4, decay=0.6)),
                    (0.42, voice(sq, note("C6"), 0.4, 0.15, 0.5, decay=0.5, vibrato=0.01))),
    }


def praia():
    """Beach: soft triangle tones, little noise splashes like surf."""
    tri = "triangle"
    splash = lambda length, gain=0.12: voice("noise", 4000, length, gain, decay=length)  # noqa: E731
    return {
        "move": voice(tri, note("E6"), 0.035, 0.35),
        "page": mix((0, voice(tri, note("C6"), 0.04, 0.35)), (0.045, voice(tri, note("G6"), 0.05, 0.35))),
        "select": mix((0, voice(tri, note("G5"), 0.07, 0.4)), (0.06, voice(tri, note("D6"), 0.1, 0.4)),
                      (0, splash(0.08, 0.06))),
        "back": mix((0, voice(tri, note("D6"), 0.06, 0.4)), (0.06, voice(tri, note("G5"), 0.1, 0.4))),
        "launch": mix((0, arpeggio(tri, ["C5", "E5", "G5", "C6", "E6"], 0.07, 0.16, 0.4)),
                      (0.3, splash(0.4, 0.1))),
        "menu_open": mix((0, voice(tri, note("A5"), 0.12, 0.35, slide=0.5)), (0, splash(0.12, 0.05))),
        "menu_close": mix((0, voice(tri, note("E6"), 0.12, 0.35, slide=-0.5)), (0, splash(0.12, 0.05))),
        "toggle": voice("square", note("A6"), 0.03, 0.12, 0.125),
        "error": mix((0, voice(tri, note("E4"), 0.14, 0.45)), (0.15, voice(tri, note("D#4"), 0.2, 0.45))),
        "notice": arpeggio(tri, ["G6", "E6", "C6"], 0.08, 0.09, 0.35),
        "favorite": mix((0, arpeggio(tri, ["E6", "G#6", "B6"], 0.05, 0.08, 0.35)),
                        (0.12, voice("square", note("E7"), 0.06, 0.06, 0.125))),
        "boot": mix((0, splash(0.9, 0.12)),
                    (0.15, arpeggio(tri, ["F5", "A5", "C6", "F6"], 0.12, 0.3, 0.4)),
                    (0.6, voice(tri, note("F4"), 0.6, 0.4, vibrato=0.006))),
    }


# Short per-system jingles: the same melody shape, in each machine's mood.
JINGLES = {
    "nes": ("square", ["C5", "G5", "E5", "C6"], 0.25),
    "snes": ("square", ["E5", "G#5", "B5", "E6"], 0.5),
    "gb": ("square", ["G5", "D6", "B5", "G6"], 0.125),
    "gbc": ("square", ["A5", "C#6", "E6", "A6"], 0.25),
    "gba": ("square", ["D5", "F#5", "A5", "D6"], 0.5),
    "genesis": ("square", ["E4", "B4", "E5", "G5"], 0.25),
    "n64": ("triangle", ["C5", "F5", "A5", "C6"], 0.5),
    "psx": ("triangle", ["F#5", "A5", "C#6", "F#6"], 0.5),
    "pico-8": ("square", ["C6", "E6", "G6", "B6"], 0.125),
    "scummvm": ("triangle", ["D5", "F5", "A5", "D6"], 0.5),
}


def jingle(kind, notes, duty):
    return arpeggio(kind, notes, 0.07, 0.11, 0.2 if kind == "square" else 0.35, duty)


def write(samples, path):
    peak = max(max(abs(v) for v in samples), 1e-9)
    scale = 0.8 / peak if peak > 0.8 else 1.0
    pcm = struct.pack(f"<{len(samples)}h", *(int(v * scale * 32767) for v in samples))
    wav = path.with_suffix(".wav")
    with wave.open(str(wav), "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        out.writeframes(pcm)
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", str(wav), "-c:a", "libvorbis",
                    "-q:a", "3", str(path)], check=True)
    wav.unlink()


def main(theme, out):
    sounds = {"retro": retro, "praia": praia}[theme]()
    (out / "systems").mkdir(parents=True, exist_ok=True)
    for event, samples in sounds.items():
        write(samples, out / f"{event}.ogg")
    for system, (kind, notes, duty) in JINGLES.items():
        if theme == "praia":
            kind = "triangle"
        write(jingle(kind, notes, duty), out / "systems" / f"{system}.ogg")


if __name__ == "__main__":
    main(sys.argv[1], Path(sys.argv[2]))
