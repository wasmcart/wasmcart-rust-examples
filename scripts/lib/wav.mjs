// WAV writing: 16-bit PCM, mono or stereo (interleaved Float32 in -1..1).
import { writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';

/**
 * @param {number} rate
 * @param {Float32Array | number[]} samples interleaved when channels is 2
 * @param {number} [channels]
 */
export function wav(rate, samples, channels = 1) {
  const data = Buffer.alloc(samples.length * 2);
  for (let i = 0; i < samples.length; i++) {
    data.writeInt16LE(Math.round(Math.max(-1, Math.min(1, samples[i])) * 32767), i * 2);
  }
  const h = Buffer.alloc(44);
  h.write('RIFF', 0); h.writeUInt32LE(36 + data.length, 4); h.write('WAVE', 8);
  h.write('fmt ', 12); h.writeUInt32LE(16, 16); h.writeUInt16LE(1, 20); h.writeUInt16LE(channels, 22);
  h.writeUInt32LE(rate, 24); h.writeUInt32LE(rate * 2 * channels, 28); h.writeUInt16LE(2 * channels, 32); h.writeUInt16LE(16, 34);
  h.write('data', 36); h.writeUInt32LE(data.length, 40);
  return Buffer.concat([h, data]);
}

/** Write `samples` to `file` as a WAV, making its directory. */
export function writeWav(file, rate, samples, channels = 1) {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, wav(rate, samples, channels));
}
