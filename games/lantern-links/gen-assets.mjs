#!/usr/bin/env node
// Generate Lantern Links' sounds and music into assets/ (committed).
// Everything is synthesized here, so it is original and CC0. The course
// itself is assets/holes.txt, written by hand.
//
//   node games/lantern-links/gen-assets.mjs
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  RATE, buf, tone, kick, hat, note, master, fade, loop, lowpass, echo, noise, reseed, mixInto,
} from '../../scripts/lib/synth.mjs';
import { writeWav } from '../../scripts/lib/wav.mjs';

const OUT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'assets');
const save = (name, b) => writeWav(path.join(OUT, name), RATE, b);
const N = (s) => note(s);

/** A short knock: a damped tone plus a click of noise. */
function knock(b, at, f, decay, vol, click = 0.4) {
  const s = Math.round(at * RATE);
  for (let i = 0; i < 0.25 * RATE && s + i < b.length; i++) {
    const t = i / RATE;
    b[s + i] += vol * (Math.sin(2 * Math.PI * f * t) * Math.exp(-t * decay) + click * noise() * Math.exp(-t * 300));
  }
}

// Putt: a soft hollow tock.
{
  reseed(1);
  const b = buf(0.3);
  knock(b, 0, 820, 45, 0.7, 0.5);
  knock(b, 0, 1640, 80, 0.2, 0);
  save('putt.wav', fade(master(lowpass(b, 6000), 0.8), 0, 30));
}

// Wall: a wooden knock.
{
  reseed(2);
  const b = buf(0.3);
  knock(b, 0, 260, 30, 0.8, 0.6);
  knock(b, 0, 610, 55, 0.3, 0);
  save('wall.wav', fade(master(lowpass(b, 3500), 0.8), 0, 30));
}

// Bumper: a springy boing.
{
  const b = buf(0.6);
  let p = 0;
  for (let i = 0; i < b.length; i++) {
    const t = i / RATE;
    const f = 300 + 500 * Math.exp(-t * 14) + 30 * Math.sin(2 * Math.PI * 24 * t) * Math.exp(-t * 4);
    p += f / RATE;
    b[i] = Math.sin(2 * Math.PI * p) * Math.exp(-t * 6) * 0.6;
  }
  save('bumper.wav', fade(master(b, 0.75), 1, 60));
}

// Turnstile: a hard clack.
{
  reseed(3);
  const b = buf(0.3);
  knock(b, 0, 420, 50, 0.7, 0.9);
  knock(b, 0.035, 300, 60, 0.4, 0.5);
  save('bar.wav', fade(master(lowpass(b, 4500), 0.8), 0, 30));
}

// Lip: a quick rattle round the rim.
{
  reseed(4);
  const b = buf(0.35);
  for (let k = 0; k < 4; k++) knock(b, k * 0.045, 1200 + 150 * k, 90, 0.4 - k * 0.07, 0.3);
  save('lip.wav', fade(master(b, 0.6), 0, 30));
}

// Splash: a noisy plunge and a few bubbles.
{
  reseed(5);
  const b = buf(1.0);
  for (let i = 0; i < b.length; i++) {
    const t = i / RATE;
    b[i] = noise() * Math.exp(-t * 7) * 0.8;
  }
  lowpass(b, (i) => 3500 * Math.exp(-i / RATE * 4) + 300);
  for (const [at, f] of [[0.18, 700], [0.3, 900], [0.42, 600], [0.55, 1100]]) {
    tone(b, at, 0.03, 12 * Math.log2(f / 440) + 69, { wave: 'sine', vol: 0.18, slide: 7, env: { a: 0.002, d: 0.05, s: 0, r: 0.04 } });
  }
  save('splash.wav', fade(master(b, 0.8), 1, 120));
}

// Sink: the rattle into the cup and a little chime.
{
  reseed(6);
  const b = buf(1.4);
  for (let k = 0; k < 3; k++) knock(b, k * 0.06, 900 - 120 * k, 70, 0.35, 0.3);
  knock(b, 0.2, 180, 25, 0.5, 0.2);
  [['E6', 0.3], ['G6', 0.4], ['C7', 0.5]].forEach(([n, at]) => tone(b, at, 0.05, N(n), { wave: 'sine', vol: 0.25, env: { a: 0.002, d: 0.3, s: 0, r: 0.5 } }));
  save('sink.wav', fade(master(echo(b, 0.17, 0.3, 0.25), 0.75), 1, 150));
}

// Hole in one: a sparkling fanfare.
{
  const b = buf(2.6);
  [['C5', 0], ['E5', 0.1], ['G5', 0.2], ['C6', 0.3], ['E6', 0.5], ['G6', 0.6], ['C7', 0.7]].forEach(([n, at], i) => {
    tone(b, at, i === 6 ? 0.8 : 0.2, N(n), { wave: 'tri', vol: 0.2, vib: i === 6 ? 0.15 : 0, env: { a: 0.005, d: 0.2, s: 0.5, r: 0.5 } });
    tone(b, at, 0.05, N(n) + 12, { wave: 'sine', vol: 0.08, env: { a: 0.002, d: 0.2, s: 0, r: 0.3 } });
  });
  for (const n of ['C4', 'E4', 'G4']) tone(b, 0.7, 1.0, N(n), { wave: 'tri', vol: 0.1, env: { a: 0.05, d: 0.3, s: 0.6, r: 0.6 } });
  save('ace.wav', fade(master(echo(b, 0.2, 0.3, 0.3), 0.75), 2, 250));
}

// End of the round: a warm resolving phrase.
{
  const b = buf(3.0);
  [['G4', 0, 0.3], ['A4', 0.3, 0.3], ['C5', 0.6, 0.3], ['E5', 0.9, 0.5], ['D5', 1.4, 0.3], ['C5', 1.7, 1.0]].forEach(([n, at, l]) => {
    tone(b, at, l, N(n), { wave: 'sine', vol: 0.3, vib: 0.1, env: { a: 0.02, d: 0.2, s: 0.7, r: 0.4 } });
  });
  for (const [ch, at] of [[['F3', 'A3', 'C4', 'E4'], 0], [['G3', 'B3', 'D4', 'F4'], 0.9], [['C3', 'E3', 'G3', 'B3'], 1.7]]) {
    for (const n of ch) tone(b, at, 0.8, N(n), { wave: 'tri', vol: 0.07, env: { a: 0.02, d: 0.4, s: 0.5, r: 0.6 } });
  }
  save('round.wav', fade(master(echo(b, 0.25, 0.3, 0.3), 0.7), 2, 300));
}

// Start and pause.
{
  const b = buf(0.7);
  tone(b, 0, 0.12, N('G5'), { wave: 'sine', vol: 0.35, env: { a: 0.005, d: 0.1, s: 0.6, r: 0.2 } });
  tone(b, 0.12, 0.25, N('D6'), { wave: 'sine', vol: 0.35, env: { a: 0.005, d: 0.1, s: 0.6, r: 0.3 } });
  save('start.wav', fade(master(echo(b, 0.15, 0.3, 0.3), 0.6), 1, 80));
}
{
  const b = buf(0.25);
  tone(b, 0, 0.06, N('E5'), { wave: 'sine', vol: 0.3, env: { a: 0.002, d: 0.05, s: 0.3, r: 0.1 } });
  tone(b, 0.07, 0.06, N('B4'), { wave: 'sine', vol: 0.3, env: { a: 0.002, d: 0.05, s: 0.3, r: 0.1 } });
  save('pause.wav', fade(master(b, 0.55), 1, 30));
}

/** A soft electric-piano note: sine with a bell-like overtone. */
function keys(out, at, len, midi, vol) {
  tone(out, at, len, midi, { wave: 'sine', vol, env: { a: 0.005, d: 0.6, s: 0.35, r: 0.5 } });
  tone(out, at, len * 0.4, midi + 19, { wave: 'sine', vol: vol * 0.15, env: { a: 0.002, d: 0.2, s: 0.0, r: 0.2 } });
}
/** Brushed snare: a long soft noise swish. */
function brush(out, at, vol) {
  const s = Math.round(at * RATE);
  let lp = 0;
  for (let i = 0; i < 0.18 * RATE && s + i < out.length; i++) {
    const t = i / RATE;
    const nz = noise();
    lp += 0.25 * (nz - lp);
    out[s + i] += vol * (nz - lp) * Math.min(1, t / 0.02) * Math.exp(-t * 14);
  }
}

// Title: slow night chords, 8 bars at 66 bpm.
{
  const prog = [['F3', 'A3', 'C4', 'E4'], ['E3', 'G3', 'B3', 'D4'], ['D3', 'F3', 'A3', 'C4'], ['G2', 'F3', 'B3', 'D4']];
  const b = loop(66, 8, ({ out, beat }) => {
    for (let bar = 0; bar < 8; bar++) {
      const ch = prog[bar % 4];
      ch.forEach((n, k) => keys(out, beat(bar * 4) + k * 0.04, beat(3.6), N(n), 0.07));
      tone(out, beat(bar * 4), beat(3.5), N(ch[0]) - 12, { wave: 'sine', vol: 0.18, env: { a: 0.02, d: 0.5, s: 0.6, r: 0.6 } });
      // a few high notes like distant chimes
      for (const k of [1.5, 2.5]) keys(out, beat(bar * 4 + k), beat(1), N(ch[(bar + Math.round(k)) % 4]) + 12, 0.04);
    }
  });
  save('title.wav', master(echo(lowpass(b, 3500), 0.455, 0.35, 0.3), 0.6));
}

// Course music: a relaxed swing with walking bass and brushes, 16 bars at 92 bpm.
{
  const prog = [['F2', 'A3', 'C4', 'E4'], ['D2', 'F3', 'A3', 'C4'], ['G2', 'F3', 'A3', 'D4'], ['C2', 'E3', 'G3', 'B3']];
  const walk = [[0, 4, 7, 9], [0, 3, 7, 5], [0, 2, 5, 7], [0, 4, 7, 11]];
  const mel = ['C5', 'A4', 'G4', 'F4', 'A4', 'C5', 'D5', 'C5', 'Bb4', 'A4', 'G4', 'E4', 'G4', 'A4', 'C5', 'E5'];
  const b = loop(92, 16, ({ out, beat }) => {
    for (let bar = 0; bar < 16; bar++) {
      const ch = prog[bar % 4];
      const root = N(ch[0]);
      for (let k = 0; k < 4; k++) {
        tone(out, beat(bar * 4 + k), beat(0.9), root + walk[bar % 4][k], { wave: 'tri', vol: 0.22, env: { a: 0.01, d: 0.25, s: 0.5, r: 0.08 } });
        // swung ride: on the beat and the late eighth
        hat(out, beat(bar * 4 + k), 0.035, 40);
        if (k % 2 === 1) hat(out, beat(bar * 4 + k + 0.66), 0.025, 50);
        if (k % 2 === 1) brush(out, beat(bar * 4 + k), 0.06);
      }
      kick(out, beat(bar * 4), 0.12);
      // comping chords on 2 and the and of 3
      for (const at of [1, 2.66]) ch.slice(1).forEach((n) => keys(out, beat(bar * 4 + at), beat(0.5), N(n), 0.04));
      if (bar >= 4 && bar < 12) {
        for (let k = 0; k < 2; k++) {
          const n = mel[((bar - 4) * 2 + k) % mel.length];
          keys(out, beat(bar * 4 + k * 2 + (k ? 0.66 : 0)), beat(1.2), N(n), 0.08);
        }
      }
    }
  });
  save('music.wav', master(lowpass(b, 5000), 0.6));
}

// Night: crickets over a soft breeze, a seamless 4 second loop.
{
  reseed(9);
  const len = 4.0;
  const b = buf(len);
  // breeze: very low noise
  let lp = 0;
  for (let i = 0; i < b.length; i++) {
    lp += 0.01 * (noise() - lp);
    b[i] = lp * 0.6;
  }
  // crickets: trains of short high chirps
  for (const [f, rate, start, vol] of [[4300, 0.5, 0.1, 0.06], [4700, 0.62, 0.35, 0.045], [3900, 0.8, 0.6, 0.035]]) {
    for (let t = start; t < len; t += rate) {
      for (let k = 0; k < 3; k++) {
        const at = t + k * 0.03;
        const s = Math.round(at * RATE);
        for (let i = 0; i < 0.018 * RATE; i++) {
          const tt = i / RATE;
          const idx = (s + i) % b.length;
          b[idx] += vol * Math.sin(2 * Math.PI * f * tt) * Math.sin(Math.PI * tt / 0.018);
        }
      }
    }
  }
  const head = 600;
  for (let i = 0; i < head; i++) b[i] = b[i] * (i / head) + b[b.length - head + i] * (1 - i / head);
  save('night.wav', master(b, 0.35));
}
console.log('lantern-links assets written to', OUT);
void mixInto;
