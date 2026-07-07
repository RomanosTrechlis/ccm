// Generates all app imagery, dependency-free:
//   - tray-{green,amber,red,grey}.png  (32px status dots for the system tray)
//   - icon.ico                         (app icon: 16/32/48 BMP + 256 PNG entries)
//   - ../../static/favicon.png         (64px, served by SvelteKit)
// The app icon is a ring-gauge motif matching the dashboard's Overview gauge.
// Re-run with: node src-tauri/icons/gen-icons.mjs
// SETUP REQUIRED: for real branding replace via `npm run tauri icon <1024.png>`.
import { deflateSync } from 'node:zlib';
import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const dir = dirname(fileURLToPath(import.meta.url));

// ------------------------------------------------------------------ PNG

const crcTable = Array.from({ length: 256 }, (_, n) => {
	let c = n;
	for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
	return c >>> 0;
});
const crc32 = (buf) => {
	let c = 0xffffffff;
	for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
	return (c ^ 0xffffffff) >>> 0;
};

function pngChunk(type, data) {
	const out = Buffer.alloc(12 + data.length);
	out.writeUInt32BE(data.length, 0);
	out.write(type, 4);
	data.copy(out, 8);
	out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
	return out;
}

function png(size, px) {
	const ihdr = Buffer.alloc(13);
	ihdr.writeUInt32BE(size, 0);
	ihdr.writeUInt32BE(size, 4);
	ihdr[8] = 8; // bit depth
	ihdr[9] = 6; // RGBA
	const raw = Buffer.alloc(size * (1 + size * 4)); // filter byte 0 per row
	for (let y = 0; y < size; y++)
		px.copy(raw, y * (1 + size * 4) + 1, y * size * 4, (y + 1) * size * 4);
	return Buffer.concat([
		Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
		pngChunk('IHDR', ihdr),
		pngChunk('IDAT', deflateSync(raw)),
		pngChunk('IEND', Buffer.alloc(0))
	]);
}

// ------------------------------------------------------------------ ICO

/** ICO image entry: 32bpp BMP (BITMAPINFOHEADER + bottom-up BGRA + AND mask) */
function bmpEntry(size, px) {
	const rowMask = Math.ceil(size / 32) * 4;
	const bmp = Buffer.alloc(40 + size * size * 4 + size * rowMask);
	bmp.writeUInt32LE(40, 0);
	bmp.writeInt32LE(size, 4);
	bmp.writeInt32LE(size * 2, 8); // XOR + AND mask height
	bmp.writeUInt16LE(1, 12);
	bmp.writeUInt16LE(32, 14);
	for (let y = 0; y < size; y++)
		for (let x = 0; x < size; x++) {
			const src = ((size - 1 - y) * size + x) * 4; // bottom-up
			const dst = 40 + (y * size + x) * 4;
			bmp[dst] = px[src + 2]; // BGRA
			bmp[dst + 1] = px[src + 1];
			bmp[dst + 2] = px[src];
			bmp[dst + 3] = px[src + 3];
		}
	return bmp;
}

/** multi-image .ico; images: [{ size, data }] where data is a BMP entry or whole PNG */
function ico(images) {
	const header = Buffer.alloc(6);
	header.writeUInt16LE(1, 2); // type: icon
	header.writeUInt16LE(images.length, 4);
	const entries = [];
	let offset = 6 + 16 * images.length;
	for (const img of images) {
		const e = Buffer.alloc(16);
		e[0] = img.size % 256; // 0 means 256
		e[1] = img.size % 256;
		e.writeUInt16LE(1, 4); // planes
		e.writeUInt16LE(32, 6); // bpp
		e.writeUInt32LE(img.data.length, 8);
		e.writeUInt32LE(offset, 12);
		offset += img.data.length;
		entries.push(e);
	}
	return Buffer.concat([header, ...entries, ...images.map((i) => i.data)]);
}

// -------------------------------------------------------------- drawing

const clamp01 = (v) => Math.max(0, Math.min(1, v));
const lerp = (a, b, t) => a.map((v, i) => v + (b[i] - v) * t);

const GREEN = [61, 220, 132];
const AMBER = [245, 166, 35];
const RED = [255, 95, 86];
const BG = [21, 21, 31];
const TRACK = [44, 44, 60];

/** gauge color along the arc: green → amber → red */
function gaugeColor(t) {
	return t < 0.5 ? lerp(GREEN, AMBER, t * 2) : lerp(AMBER, RED, (t - 0.5) * 2);
}

/** size x size RGBA solid dot (tray) */
function dotPixels(size, rgb) {
	const px = Buffer.alloc(size * size * 4);
	const c = (size - 1) / 2;
	const rad = size / 2 - 1;
	for (let y = 0; y < size; y++)
		for (let x = 0; x < size; x++) {
			const a = clamp01(rad - Math.hypot(x - c, y - c) + 0.5);
			const i = (y * size + x) * 4;
			px[i] = rgb[0];
			px[i + 1] = rgb[1];
			px[i + 2] = rgb[2];
			px[i + 3] = Math.round(a * 255);
		}
	return px;
}

/** the app icon: dark disc + 270° gauge track + green→amber→red arc at 72% */
function gaugePixels(size) {
	const px = Buffer.alloc(size * size * 4);
	const c = (size - 1) / 2;
	const bgR = size / 2 - Math.max(1, size / 48);
	const rMid = size * 0.335;
	const rHalf = size * 0.085; // ring half-thickness
	const START = Math.PI * 0.75; // gauge arc: 135° …
	const SWEEP = Math.PI * 1.5; // … clockwise 270°
	const FILL = 0.72;

	for (let y = 0; y < size; y++)
		for (let x = 0; x < size; x++) {
			const dx = x - c;
			const dy = y - c;
			const d = Math.hypot(dx, dy);

			// dark disc background
			let r = BG[0], g = BG[1], b = BG[2];
			let a = clamp01(bgR - d + 0.5);

			// ring band, anti-aliased
			const ringA = clamp01(rHalf - Math.abs(d - rMid) + 0.5);
			if (ringA > 0) {
				// angle position along the clockwise sweep from START, in [0, 1)
				const t = (((Math.atan2(dy, dx) - START) % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI) / SWEEP;
				if (t <= 1) {
					const col = t <= FILL ? gaugeColor(t / FILL) : TRACK;
					r = r + (col[0] - r) * ringA;
					g = g + (col[1] - g) * ringA;
					b = b + (col[2] - b) * ringA;
					a = Math.max(a, ringA);
				}
			}

			const i = (y * size + x) * 4;
			px[i] = Math.round(r);
			px[i + 1] = Math.round(g);
			px[i + 2] = Math.round(b);
			px[i + 3] = Math.round(a * 255);
		}
	return px;
}

// --------------------------------------------------------------- output

const dots = { green: GREEN, amber: AMBER, red: RED, grey: [122, 122, 132] };
for (const [name, rgb] of Object.entries(dots))
	writeFileSync(join(dir, `tray-${name}.png`), png(32, dotPixels(32, rgb)));

writeFileSync(
	join(dir, 'icon.ico'),
	ico([
		{ size: 16, data: bmpEntry(16, gaugePixels(16)) },
		{ size: 32, data: bmpEntry(32, gaugePixels(32)) },
		{ size: 48, data: bmpEntry(48, gaugePixels(48)) },
		{ size: 256, data: png(256, gaugePixels(256)) }
	])
);

const staticDir = join(dir, '..', '..', 'static');
mkdirSync(staticDir, { recursive: true });
writeFileSync(join(staticDir, 'favicon.png'), png(64, gaugePixels(64)));

console.log('wrote tray dots, gauge icon.ico (16/32/48/256), static/favicon.png');
