// Archivo: branding/generate_pngs.mjs
// Propósito: Convierte los SVG maestros de "Beat Hammer" (/branding/svg/*) a PNG en todas
// las densidades Android (mdpi..xxxhdpi para iconos clásicos, redondos y capas adaptativas)
// además de los PNG para tiendas de apps (Uptodown 512x512, banner 1024x500) y miniaturas (1280x720).

import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const SVG_DIR = path.join(ROOT, "branding", "svg");
const PNG_DIR = path.join(ROOT, "branding", "png");
const RES_DIR = path.join(ROOT, "android", "app", "src", "main", "res");

const FONTS = [
  "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
  "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-ExtraBold.ttf",
  "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Bold.ttf",
];

function renderSvgToPng(svgName, outPath, width, withFonts = false) {
  mkdirSync(path.dirname(outPath), { recursive: true });
  const args = [
    "--yes",
    "@resvg/resvg-js-cli",
    "--fit-width",
    String(width),
    "--shape-rendering",
    "2",
    "--text-rendering",
    "2",
  ];
  if (withFonts) {
    for (const f of FONTS) {
      args.push("--font-file", f);
    }
  } else {
    args.push("--no-system-font");
  }
  args.push(path.join(SVG_DIR, svgName), outPath);
  execFileSync("npx", args, { stdio: "inherit" });
}

// 1. Densidades estándar Android (48dp legacy + 108dp adaptive layers)
const androidDensities = [
  { folder: "mipmap-mdpi", legacySize: 48, adaptiveSize: 108 },
  { folder: "mipmap-hdpi", legacySize: 72, adaptiveSize: 162 },
  { folder: "mipmap-xhdpi", legacySize: 96, adaptiveSize: 216 },
  { folder: "mipmap-xxhdpi", legacySize: 144, adaptiveSize: 324 },
  { folder: "mipmap-xxxhdpi", legacySize: 192, adaptiveSize: 432 },
];

for (const d of androidDensities) {
  const dir = path.join(RES_DIR, d.folder);
  renderSvgToPng("beat_hammer_icon.svg", path.join(dir, "ic_launcher.png"), d.legacySize);
  renderSvgToPng("beat_hammer_icon_round.svg", path.join(dir, "ic_launcher_round.png"), d.legacySize);
  renderSvgToPng("beat_hammer_foreground.svg", path.join(dir, "ic_launcher_foreground.png"), d.adaptiveSize);
  renderSvgToPng("beat_hammer_background.svg", path.join(dir, "ic_launcher_background.png"), d.adaptiveSize);
}

// 2. Iconos de tienda de apps (Uptodown, etc.) y miniaturas
renderSvgToPng("beat_hammer_icon.svg", path.join(PNG_DIR, "store_icon_512x512.png"), 512);
renderSvgToPng("beat_hammer_icon.svg", path.join(PNG_DIR, "store_icon_1024x1024.png"), 1024);
renderSvgToPng("beat_hammer_icon_round.svg", path.join(PNG_DIR, "store_icon_round_512x512.png"), 512);
renderSvgToPng("beat_hammer_store_banner.svg", path.join(PNG_DIR, "store_feature_banner_1024x500.png"), 1024, true);
renderSvgToPng("beat_hammer_thumbnail.svg", path.join(PNG_DIR, "thumbnail_1280x720.png"), 1280, true);
renderSvgToPng("beat_hammer_thumbnail.svg", path.join(PNG_DIR, "thumbnail_640x360.png"), 640, true);

console.log("Todos los PNG de Beat Hammer fueron generados correctamente.");
