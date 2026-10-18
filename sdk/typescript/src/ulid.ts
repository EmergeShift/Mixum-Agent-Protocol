import { randomBytes } from "node:crypto";

const ENCODING = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"; // Crockford base32

let lastTime = 0;
let lastRandom: Uint8Array | undefined;

/**
 * ULID generator (canonical 26-char). Monotonic within a process for the
 * same millisecond; randomness from crypto. Spec: envelope.md identifiers.
 */
export function ulid(time: number = Date.now()): string {
  if (time === lastTime && lastRandom) {
    increment(lastRandom);
  } else {
    lastRandom = new Uint8Array(randomBytes(10));
  }
  lastTime = time;
  return encodeTime(time) + encodeRandom(lastRandom!);
}

function increment(bytes: Uint8Array): void {
  for (let i = bytes.length - 1; i >= 0; i--) {
    if (bytes[i]! < 0xff) {
      bytes[i]! += 1;
      return;
    }
    bytes[i] = 0;
  }
}

function encodeTime(time: number): string {
  let out = "";
  for (let i = 9; i >= 0; i--) {
    out = ENCODING[time % 32]! + out;
    time = Math.floor(time / 32);
  }
  return out;
}

function encodeRandom(bytes: Uint8Array): string {
  let out = "";
  let bits = 0;
  let acc = 0;
  for (const byte of bytes) {
    acc = (acc << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      out += ENCODING[(acc >>> (bits - 5)) & 31]!;
      bits -= 5;
    }
  }
  if (bits > 0) out += ENCODING[(acc << (5 - bits)) & 31]!;
  return out; // 16 bytes -> 26 chars with the time part
}

/** Full 26-char ULID from a 16-byte binary form (wire stdio frames). */
export function ulidFromBytes(bytes: Uint8Array): string {
  if (bytes.length !== 16) throw new Error("ULID binary form must be 16 bytes");
  let time = 0;
  for (let i = 0; i < 6; i++) time = time * 256 + bytes[i]!;
  return encodeTime(time) + encodeRandom(bytes.slice(6));
}
