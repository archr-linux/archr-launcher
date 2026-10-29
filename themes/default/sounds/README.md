# Default theme sounds

Short interface sounds, one file per event: `boot`, `move`, `page`, `select`,
`back`, `launch`, `menu_open`, `menu_close`, `toggle`, `error`, `notice` and
`favorite`. The theme has no per-system sounds; see `docs/temas.md`.

Origin: synthesized from scratch by `generate.py` (sine tones with short
envelopes, mono, 44.1 kHz, Ogg Vorbis). No recording or third-party material
is involved, so there is nothing to attribute: they are released under the
same license as the launcher (GPL-2.0-or-later).

To change them, edit the notes in `generate.py` and run it again
(it needs Python 3 and ffmpeg with libvorbis).
