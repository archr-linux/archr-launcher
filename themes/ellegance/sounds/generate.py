#!/usr/bin/env python3
"""Generates the Ellegance theme sounds, synthesized from scratch.

The character aims at a premium console interface: navigation ticks so soft
they are felt more than heard, a rounded "pop" to confirm, a dull "tock" for
switches, and struck glass for the moments that matter (a game starting, a
notice, the boot). No recording or third-party material is used.

Struck glass rings with the inharmonic partials of a free bar (1, 2.756,
5.404, 8.933 times the fundamental), each decaying faster than the one
below, so a note starts bright and settles into a pure tone. Ticks and pops
are short sines with a fast pitch drop, low-passed so nothing is shrill on
the console's small speaker. Mono, 44.1 kHz, Ogg Vorbis.

Usage: python3 generate.py   (needs Python 3 and ffmpeg with libvorbis).
"""
import math
import random
import struct
import subprocess
import wave
from pathlib import Path

RATE = 44100
OUT = Path(__file__).parent
MODES = [(1.0, 1.0, 1.0), (2.756, 0.38, 0.5), (5.404, 0.16, 0.3), (8.933, 0.06, 0.18)]
random.seed(7)  # the noise is part of the sound: keep the files reproducible


def lowpass(samples, cutoff_hz):
    """One-pole low-pass, to round off a sound's top end."""
    a = 1 - math.exp(-2 * math.pi * cutoff_hz / RATE)
    out, last = [], 0.0
    for v in samples:
        last += a * (v - last)
        out.append(last)
    return out


def glass(freq, length, gain=1.0, decay=0.12, brightness=1.0):
    """One struck-glass note; brightness scales the upper partials."""
    out = []
    for n in range(int(RATE * length)):
        t = n / RATE
        value = 0.0
        for ratio, level, rel in MODES:
            f = freq * ratio
            if f > RATE / 2.4:
                continue
            weight = level * (brightness if ratio > 1 else 1.0)
            value += weight * math.sin(2 * math.pi * f * t) * math.exp(-t / (decay * rel))
        attack = min(t / 0.0015, 1.0)
        release = min((length - t) / 0.01, 1.0)
        out.append(gain * value * attack * release)
    return out


def pop(start_hz, end_hz, length, gain=1.0, decay=None):
    """A rounded pop: a sine whose pitch falls quickly, like a soft bubble."""
    decay = decay or length / 3
    out, phase = [], 0.0
    for n in range(int(RATE * length)):
        t = n / RATE
        f = end_hz + (start_hz - end_hz) * math.exp(-t / (length / 5))
        phase += 2 * math.pi * f / RATE
        attack = min(t / 0.001, 1.0)
        release = min((length - t) / 0.006, 1.0)
        out.append(gain * math.sin(phase) * math.exp(-t / decay) * attack * release)
    return out


def tap(length=0.01, gain=0.2, cutoff_hz=3500):
    """The strike: a short burst of low-passed noise."""
    noise = [random.uniform(-1, 1) * math.exp(-(n / RATE) / (length / 4)) for n in range(int(RATE * length))]
    return [gain * v for v in lowpass(noise, cutoff_hz)]


def air(length, start_hz, end_hz, gain=0.1):
    """A breath of noise whose brightness sweeps: a sheet sliding in or out."""
    out, last = [], 0.0
    total = int(RATE * length)
    for n in range(total):
        x = n / total
        cut = start_hz + (end_hz - start_hz) * x
        a = 1 - math.exp(-2 * math.pi * cut / RATE)
        last += a * (random.uniform(-1, 1) - last)
        out.append(gain * last * math.sin(math.pi * x) ** 2)
    return out


def mix(*notes):
    """Notes are (start seconds, samples) pairs, added together."""
    size = max(int(RATE * start) + len(samples) for start, samples in notes) + 1
    buf = [0.0] * size
    for start, samples in notes:
        offset = int(RATE * start)
        for i, v in enumerate(samples):
            buf[offset + i] += v
    return buf


# A bright, consonant set: E major around the sixth octave.
E5, Gs5, B5 = 659.3, 830.6, 987.8
E6, Gs6, B6, Cs7, E7 = 1318.5, 1661.2, 1975.5, 2217.5, 2637.0

SOUNDS = {
    # Every cursor step: the smallest glass note. No pitch glide (a falling
    # pitch this short sounds like a bird) and no noise click (with a pure
    # tone it reads as a laser).
    "move": glass(B6, 0.06, 1.0, decay=0.014, brightness=0.45),
    # A page or letter jump: two of those glass notes, rising.
    "page": mix((0, glass(B6, 0.06, 1.0, decay=0.014, brightness=0.45)), (0.045, glass(E7, 0.065, 0.9, decay=0.015, brightness=0.45))),
    # Open (A): a rounded pop with a glint of glass on top.
    "select": mix((0, pop(1400, 520, 0.09, 1.0, 0.03)), (0.012, glass(E6 * 1.5, 0.16, 0.22, decay=0.04, brightness=0.6))),
    # Back (B): the pop alone, lower and softer.
    "back": lowpass(pop(1100, 380, 0.08, 1.0, 0.026), 3500),
    # Launch a game: a rising glass arpeggio that rings on.
    "launch": mix(
        (0, tap(0.012, 0.18)),
        *[(i * 0.065, glass(f, 0.8, 0.24, decay=0.24, brightness=0.85)) for i, f in enumerate([E6, Gs6, B6, E7])],
    ),
    # Menus: a breath of air, with a soft glint where the sheet lands.
    "menu_open": mix((0, air(0.15, 600, 5000, 0.11)), (0.085, glass(B6, 0.14, 0.13, decay=0.04, brightness=0.5))),
    "menu_close": mix((0, air(0.13, 5000, 600, 0.10)), (0.0, glass(Gs6, 0.1, 0.10, decay=0.03, brightness=0.4))),
    # A switch: a dull, firm "tock".
    "toggle": lowpass(mix((0, pop(1700, 900, 0.045, 1.0, 0.012)), (0, tap(0.005, 0.35, 3000))), 3800),
    # Error: two muted knocks, low and dull.
    "error": lowpass(mix((0, glass(440.0, 0.14, 0.34, decay=0.04, brightness=0.2)), (0.13, glass(370.0, 0.2, 0.34, decay=0.05, brightness=0.2))), 2500),
    # Notice: three gentle glass tones, rising.
    "notice": mix(*[(i * 0.085, glass(f, 0.45, 0.22, decay=0.13, brightness=0.7)) for i, f in enumerate([B5 * 2, E6 * 1.5, E7])]),
    # Favorite: a quick sparkle, three high glints.
    "favorite": mix(*[(i * 0.045, glass(f, 0.3, 0.2, decay=0.07, brightness=0.8)) for i, f in enumerate([B6, E7, 3322.4])]),
    # Boot: a soft glass chord that swells and fades.
    "boot": mix(
        *[(i * 0.09, glass(f, 1.3, 0.17, decay=0.45, brightness=0.7)) for i, f in enumerate([E5 * 2, Gs5 * 2, B5 * 2, E6 * 1.5])],
        (0.0, air(0.5, 200, 1500, 0.05)),
    ),
}

# Peak level per sound: the frequent ones stay quiet.
LEVEL = {"move": 0.55, "page": 0.58, "toggle": 0.5, "back": 0.6, "menu_open": 0.6, "menu_close": 0.55}

for name, samples in SOUNDS.items():
    peak = max(max(abs(v) for v in samples), 1e-9)
    level = LEVEL.get(name, 0.8)
    pcm = struct.pack(f"<{len(samples)}h", *(int(v / peak * 32767 * level) for v in samples))
    wav = OUT / f"{name}.wav"
    with wave.open(str(wav), "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        out.writeframes(pcm)
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-i", str(wav), "-c:a", "libvorbis", "-q:a", "4", str(OUT / f"{name}.ogg")],
        check=True,
    )
    wav.unlink()
