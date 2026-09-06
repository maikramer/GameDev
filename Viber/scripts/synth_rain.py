#!/usr/bin/env python3
"""Synthesize the ambient-audio OGGs of the Juice pass (WS-A) from noise.

Outputs (written next to this repo's shared-assets pool, `--out`):

* ``rain_loop.ogg``        — 8 s of filtered rain hiss + droplet patter,
  seam-crossfaded so it loops without a click (consumed by the rain SFX
  loop in ``src/ambient.rs``, volume ∝ rain intensity).
* ``footstep.ogg``         — one dry, soft step (~0.16 s) for
  ``SfxClip::Footstep``.
* ``footstep_water.ogg``   — one shallow-water splash step (~0.22 s) for
  ``SfxClip::FootstepWater``.

Everything is deterministic (fixed RNG seed) so re-runs are byte-stable
modulo the vorbis encoder. Requires numpy + ffmpeg on PATH.

    python3 scripts/synth_rain.py [--out examples/shared-assets/public]
"""

from __future__ import annotations

import argparse
import subprocess
import tempfile
import wave
from pathlib import Path

import numpy as np

RATE = 44100
SEED = 0x51A1  # fixed: deterministic synthesis


def _write_wav(path: Path, samples: np.ndarray) -> None:
    mono = np.clip(samples, -1.0, 1.0)
    pcm = (mono * 32767.0).astype("<i2")
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(RATE)
        handle.writeframes(pcm.tobytes())


def _encode_ogg(wav: Path, out: Path) -> None:
    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-loglevel",
            "error",
            "-f",
            "s16le",
            "-ar",
            str(RATE),
            "-ac",
            "1",
            "-i",
            str(wav),
            "-c:a",
            "libvorbis",
            "-q:a",
            "3",
            str(out),
        ],
        check=True,
    )


def _lowpass_shaped_noise(seconds: float, rolloff: float, rng: np.random.Generator) -> np.ndarray:
    """White noise with a gentle 1/(1+(f/f0)^rolloff) spectral tilt — the
    broadband hiss of steady rain."""
    n = int(seconds * RATE)
    noise = rng.standard_normal(n)
    spectrum = np.fft.rfft(noise)
    freqs = np.fft.rfftfreq(n, d=1.0 / RATE)
    freqs[0] = freqs[1]  # avoid /0 on DC
    tilt = 1.0 / (1.0 + (freqs / 1400.0) ** rolloff)
    shaped = np.fft.irfft(spectrum * tilt, n=n)
    peak = np.abs(shaped).max()
    return shaped / peak if peak > 0 else shaped


def make_rain_loop(seconds: float = 8.0) -> np.ndarray:
    """8 s rain bed: hiss + slow swell + sparse droplet ticks, seamless."""
    rng = np.random.default_rng(SEED)
    fade = 0.5
    n = int(seconds * RATE)
    f = int(fade * RATE)
    # Synthesize the body PLUS one extra fade worth of tail to fold in.
    bed = _lowpass_shaped_noise(seconds + fade, 0.6, rng)

    # Slow swell (±20 %): rain comes and goes even inside one loop.
    swell_t = np.linspace(0.0, 2.0 * np.pi * seconds / 5.3, n + f)
    swell = 1.0 + 0.2 * np.sin(swell_t + 1.3)
    # Patter: short bright ticks (individual drops) at Poisson-ish times.
    patter = np.zeros(n + f)
    for _ in range(int(seconds * 26)):
        start = rng.integers(0, n + f - 900)
        length = int(rng.integers(120, 900))
        tick_t = np.arange(length) / RATE
        env = np.exp(-tick_t * rng.uniform(90.0, 220.0))
        patter[start : start + length] += env * rng.uniform(0.05, 0.22)
    patter += rng.standard_normal(n + f) * 0.012  # unbanded tick noise

    mix = bed * swell * 0.55 + patter * 0.45
    # Fold the tail over the head: y[:F] = head·fade-in + tail·fade-out —
    # the classic clickless seam for noise beds.
    body = mix[:n].copy()
    tail = mix[n : n + f]
    ramp = np.linspace(0.0, 1.0, f)
    body[:f] = body[:f] * ramp + tail * (1.0 - ramp)
    peak = np.abs(body).max()
    return (body / peak * 0.72) if peak > 0 else body


def _burst(length: float, decay: float, band: tuple[float, float], rng: np.random.Generator) -> np.ndarray:
    n = int(length * RATE)
    noise = rng.standard_normal(n)
    spectrum = np.fft.rfft(noise)
    freqs = np.fft.rfftfreq(n, d=1.0 / RATE)
    freqs[0] = freqs[1]
    lo, hi = band
    shape = np.exp(-0.5 * ((np.log(freqs) - np.log(np.sqrt(lo * hi))) / (0.5 * np.log(hi / lo))) ** 2)
    shaped = np.fft.irfft(spectrum * shape, n=n)
    env = np.exp(-np.arange(n) / RATE * decay)
    env[: int(0.0015 * RATE)] *= np.linspace(0.0, 1.0, int(0.0015 * RATE))
    return shaped * env


def make_footstep() -> np.ndarray:
    """One dry step: low thud + faint scuff, ~0.16 s."""
    rng = np.random.default_rng(SEED + 1)
    n = int(0.16 * RATE)
    out = np.zeros(n)
    thud = _burst(0.09, 55.0, (90.0, 700.0), rng)
    out[: len(thud)] += thud * 0.9
    scuff = _burst(0.08, 130.0, (1400.0, 6500.0), rng)
    out[int(0.03 * RATE) : int(0.03 * RATE) + len(scuff)] += scuff * 0.18
    peak = np.abs(out).max()
    return (out / peak * 0.6) if peak > 0 else out


def make_footstep_water() -> np.ndarray:
    """One shallow-water step: two splashy bursts (heel-toe), ~0.22 s."""
    rng = np.random.default_rng(SEED + 2)
    n = int(0.22 * RATE)
    out = np.zeros(n)
    for start, gain in ((0.0, 0.75), (0.055, 0.55)):
        splash = _burst(0.12, 70.0, (600.0, 5200.0), rng)
        begin = int(start * RATE)
        out[begin : begin + len(splash)] += splash * gain
    peak = np.abs(out).max()
    return (out / peak * 0.6) if peak > 0 else out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "examples/shared-assets/public",
        help="asset pool root (the folder containing assets/)",
    )
    args = parser.parse_args()
    target = args.out / "assets/audio/sfx/ambient"
    target.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        for name, samples in (
            ("rain_loop", make_rain_loop()),
            ("footstep", make_footstep()),
            ("footstep_water", make_footstep_water()),
        ):
            wav = Path(tmp) / f"{name}.wav"
            ogg = target / f"{name}.ogg"
            _write_wav(wav, samples)
            _encode_ogg(wav, ogg)
            print(f"✓ {ogg.relative_to(args.out)} ({ogg.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
