/**
 * Generates the macOS app icon from `public/logo.avif`.
 *
 * The frontend UI keeps using `public/logo.svg` everywhere. This script only
 * drives the *native macOS app icon* (Dock / .app bundle), sourced from
 * `logo.avif` at build time, so the bundled icon always tracks the source
 * asset. It only touches macOS icon files — Android / iOS / Windows assets are
 * left untouched.
 *
 * Skips regeneration when the existing icon set is newer than `logo.avif`
 * (pass FORCE=1 to override).
 */

import { $ } from "bun";
import { existsSync, mkdirSync, mkdtempSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = process.cwd();
const avif = join(root, "public", "logo.avif");
const iconsDir = join(root, "src-tauri", "icons");
const iconIcns = join(iconsDir, "icon.icns");
const iconPng = join(iconsDir, "icon.png");
const sourcePng = join(root, "src-tauri", "icons", ".logo-source-1024.png");

// macOS-relevant PNGs referenced by tauri.conf.json + the .icns backing set.
const macPngs = ["icon.png", "32x32.png", "128x128.png", "128x128@2x.png"];

// Sizes required for a valid macOS .icns (iconutil iconset convention).
const icnsSizes: Array<[string, number]> = [
  ["icon_16x16.png", 16],
  ["icon_16x16@2x.png", 32],
  ["icon_32x32.png", 32],
  ["icon_32x32@2x.png", 64],
  ["icon_128x128.png", 128],
  ["icon_128x128@2x.png", 256],
  ["icon_256x256.png", 256],
  ["icon_256x256@2x.png", 512],
  ["icon_512x512.png", 512],
  ["icon_512x512@2x.png", 1024],
];

const force = process.env.FORCE === "1";

function newerThan(a: string, b: string): boolean {
  return statSync(a).mtimeMs > statSync(b).mtimeMs;
}

if (!existsSync(avif)) {
  console.error(`[gen-macos-icon] source not found: ${avif}`);
  process.exit(1);
}

const pngsPresent = macPngs.every((f) => existsSync(join(iconsDir, f)));
if (!force && pngsPresent && existsSync(iconIcns) && newerThan(iconPng, avif)) {
  console.log("[gen-macos-icon] macOS icon set up to date, skipping");
  process.exit(0);
}

console.log("[gen-macos-icon] converting public/logo.avif -> 1024px PNG");
await $`magick ${avif} -background none -resize 1024x1024 -define png:color-type=6 PNG32:${sourcePng}`;

const iconset = mkdtempSync(join(tmpdir(), "superflow-icon-")) + ".iconset";
mkdirSync(iconset);
console.log("[gen-macos-icon] building macOS iconset");
for (const [name, size] of icnsSizes) {
  await $`magick ${sourcePng} -resize ${size}x${size} -background none -flatten ${join(iconset, name)}`;
}

console.log("[gen-macos-icon] compiling icon.icns");
await $`iconutil -c icns ${iconset} -o ${iconIcns}`;

console.log("[gen-macos-icon] writing config-referenced PNGs");
await $`magick ${sourcePng} -resize 512x512 -background none -flatten ${iconPng}`;
await $`magick ${sourcePng} -resize 32x32 -background none -define png:color-type=6 PNG32:${join(iconsDir, "32x32.png")}`;
await $`magick ${sourcePng} -resize 128x128 -background none -flatten ${join(iconsDir, "128x128.png")}`;
await $`magick ${sourcePng} -resize 256x256 -background none -flatten ${join(iconsDir, "128x128@2x.png")}`;

rmSync(iconset, { recursive: true, force: true });
rmSync(sourcePng, { force: true });

console.log(
  "[gen-macos-icon] done — macOS app icon now sourced from public/logo.avif",
);
