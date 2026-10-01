// Generates the 1024x1024 source image that `tauri icon` expands into the
// per-platform icon set. Run: node tools/make-icon.mjs
//
// Written by hand rather than pulled from a drawing library so the repo carries
// no binary source asset and the shape stays reproducible.

import { writeFileSync, mkdirSync } from "node:fs";
import { deflateSync } from "node:zlib";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const SIZE = 1024;
const BG = [0x1e, 0x1e, 0x22];
const TRACK = [0x3a, 0x3a, 0x42];
const ACCENT = [0xd9, 0x77, 0x57];
/** Fraction of the ring drawn in the accent colour. */
const FILLED = 0.72;

const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);

/** Signed distance to a rounded rectangle centred on the origin. */
function sdRoundedRect(x, y, halfW, halfH, radius) {
  const dx = Math.abs(x) - halfW + radius;
  const dy = Math.abs(y) - halfH + radius;
  const outside = Math.hypot(Math.max(dx, 0), Math.max(dy, 0));
  return outside + Math.min(Math.max(dx, dy), 0) - radius;
}

/**
 * Coverage of a shape at a pixel, from its signed distance in pixel units.
 * Distance is already linear near the edge, so a one-pixel ramp antialiases it
 * without supersampling.
 */
const coverage = (distance) => clamp01(0.5 - distance);

function blend(dst, offset, colour, alpha) {
  if (alpha <= 0) return;
  for (let c = 0; c < 3; c++) {
    dst[offset + c] = Math.round(dst[offset + c] * (1 - alpha) + colour[c] * alpha);
  }
  dst[offset + 3] = Math.round(dst[offset + 3] * (1 - alpha) + 255 * alpha);
}

function render() {
  const px = new Uint8Array(SIZE * SIZE * 4);
  const centre = SIZE / 2;
  const ringRadius = SIZE * 0.3;
  const ringHalfWidth = SIZE * 0.052;
  // Start at 12 o'clock and sweep clockwise.
  const startAngle = -Math.PI / 2;

  for (let y = 0; y < SIZE; y++) {
    for (let x = 0; x < SIZE; x++) {
      const offset = (y * SIZE + x) * 4;
      const px_ = x + 0.5 - centre;
      const py = y + 0.5 - centre;

      const bgAlpha = coverage(
        sdRoundedRect(px_, py, SIZE * 0.46, SIZE * 0.46, SIZE * 0.22),
      );
      blend(px, offset, BG, bgAlpha);
      if (bgAlpha <= 0) continue;

      const distance = Math.hypot(px_, py);
      const ringAlpha =
        coverage(Math.abs(distance - ringRadius) - ringHalfWidth) * bgAlpha;
      if (ringAlpha <= 0) continue;

      let sweep = Math.atan2(py, px_) - startAngle;
      while (sweep < 0) sweep += Math.PI * 2;
      const fraction = sweep / (Math.PI * 2);

      blend(px, offset, fraction <= FILLED ? ACCENT : TRACK, ringAlpha);
    }
  }
  return px;
}

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(buf) {
  let c = 0xffffffff;
  for (const byte of buf) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

function encodePng(pixels, size) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  ihdr[10] = 0; // deflate
  ihdr[11] = 0; // adaptive filtering
  ihdr[12] = 0; // no interlace

  // Each scanline is prefixed with its filter type; 0 means "store as is".
  const raw = Buffer.alloc(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) {
    const rowStart = y * (size * 4 + 1);
    raw[rowStart] = 0;
    Buffer.from(pixels.buffer, y * size * 4, size * 4).copy(raw, rowStart + 1);
  }

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

const outDir = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri");
mkdirSync(outDir, { recursive: true });
const target = join(outDir, "icon-source.png");
writeFileSync(target, encodePng(render(), SIZE));
console.log(`wrote ${target} (${SIZE}x${SIZE})`);
