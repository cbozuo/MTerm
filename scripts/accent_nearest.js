// 找出 46 色强调网格中与 #1FC8B4 最接近的色格（CIEDE2000 + 色相/明度辅助）
// 用法: node scripts/accent_nearest.js
const fs = require('fs');
const src = fs.readFileSync(__dirname + '/../src/theme/palettes.rs', 'utf8');
const block = src.split('pub static ACCENT_PRESETS')[1].split('];')[0];
const rows = [...block.matchAll(/\(\s*"([^"]+)",\s*"([^"]+)",\s*(0x[0-9A-Fa-f]+)\s*\)/g)]
  .map(m => ({ key: m[1], name: m[2], hex: parseInt(m[3], 16) }));
console.log('网格解析:', rows.length, '色');

// ── sRGB → CIELab (D65) ──
const lin = v => { v /= 255; return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
function toLab(hex) {
  const r = lin((hex >> 16) & 255), g = lin((hex >> 8) & 255), b = lin(hex & 255);
  const X = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
  const Y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  const Z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
  const f = t => t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116;
  const fx = f(X), fy = f(Y), fz = f(Z);
  return [116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz)];
}
const ciede = (l1, l2) => {
  const [L1, a1, b1] = l1, [L2, a2, b2] = l2;
  const C1 = Math.hypot(a1, b1), C2 = Math.hypot(a2, b2);
  const Cb = (C1 + C2) / 2;
  const G = 0.5 * (1 - Math.sqrt(Math.pow(Cb, 7) / (Math.pow(Cb, 7) + Math.pow(25, 7))));
  const a1p = (1 + G) * a1, a2p = (1 + G) * a2;
  const C1p = Math.hypot(a1p, b1), C2p = Math.hypot(a2p, b2);
  const h1p = (Math.atan2(b1, a1p) * 180 / Math.PI + 360) % 360;
  const h2p = (Math.atan2(b2, a2p) * 180 / Math.PI + 360) % 360;
  const dLp = L2 - L1, dCp = C2p - C1p;
  let dhp = h2p - h1p;
  if (Math.abs(dhp) > 180) dhp -= Math.sign(dhp) * 360;
  const dHp = 2 * Math.sqrt(C1p * C2p) * Math.sin(dhp * Math.PI / 360);
  const Lb = (L1 + L2) / 2, Cbp = (C1p + C2p) / 2;
  let hbp = h1p + h2p;
  if (Math.abs(h1p - h2p) > 180) hbp += 360;
  hbp /= 2;
  const T = 1 - 0.17 * Math.cos((hbp - 30) * Math.PI / 180) + 0.24 * Math.cos(2 * hbp * Math.PI / 180)
    + 0.32 * Math.cos((3 * hbp + 6) * Math.PI / 180) - 0.20 * Math.cos((4 * hbp - 63) * Math.PI / 180);
  const dTh = 30 * Math.exp(-Math.pow((hbp - 275) / 25, 2));
  const Rc = 2 * Math.sqrt(Math.pow(Cbp, 7) / (Math.pow(Cbp, 7) + Math.pow(25, 7)));
  const Sl = 1 + (0.015 * Math.pow(Lb - 50, 2)) / Math.sqrt(20 + Math.pow(Lb - 50, 2));
  const Sc = 1 + 0.045 * Cbp, Sh = 1 + 0.015 * Cbp * T;
  const Rt = -Math.sin(2 * dTh * Math.PI / 180) * Rc;
  return Math.sqrt(Math.pow(dLp / Sl, 2) + Math.pow(dCp / Sc, 2) + Math.pow(dHp / Sh, 2) + Rt * (dCp / Sc) * (dHp / Sh));
};
const hue = h => { const r = (h >> 16) & 255, g = (h >> 8) & 255, b = h & 255;
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn;
  if (!d) return 0; let x = mx === r ? ((g - b) / d) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
  x *= 60; return (x + 360) % 360; };
const hslL = h => { const r = (h >> 16) & 255, g = (h >> 8) & 255, b = h & 255;
  return Math.round(((Math.max(r, g, b) + Math.min(r, g, b)) / 2) / 255 * 100); };

const TARGET = 0x1FC8B4;
const tLab = toLab(TARGET), tHue = hue(TARGET);
console.log(`目标 #1FC8B4  色相 ${tHue.toFixed(0)}°  明度(L%) ${hslL(TARGET)}`);
const scored = rows.map(r => ({
  ...r, dE: ciede(tLab, toLab(r.hex)),
  dh: Math.abs(hue(r.hex) - tHue) > 180 ? 360 - Math.abs(hue(r.hex) - tHue) : Math.abs(hue(r.hex) - tHue),
  dl: Math.abs(hslL(r.hex) - hslL(TARGET)),
})).sort((a, b) => a.dE - b.dE);
console.log('\n按 CIEDE2000 排序（前 8）:');
for (const s of scored.slice(0, 8)) {
  console.log(`  ΔE ${s.dE.toFixed(1).padStart(5)}  色相差 ${s.dh.toFixed(0).padStart(3)}°  明度差 ${s.dl}  #${s.hex.toString(16).toUpperCase().padStart(6, '0')}  ${s.name}(${s.key})`);
}
