// ═════════════════════════════════════════════════════════════════════
// #ansi-batch3 2026-10-21 —— 主题 ANSI 色表六项批量迁移（改动方案批次三）：
//   ① 门面解耦：mterm 两套青槽(6/14)离开 accent 家族（划选背景=accent 40%，
//     同色会浑）、槽0 脱离界面底色、蓝槽偏靛拉开 nord frost 色域；
//   ② 彩色槽(1-6,9-14) 全表压齐 ≥4.6（A 案：保 H/S 调 L；上游原味让位
//     「清晰易分辨」）；亮档派生套（nord-light/graphite-light）锚定配对
//     暗档的 H/S，官方亮档保自身 H/S；
//   ③ slot0 口径：暗档=比 tbg 深一档的独立黑（禁用 tbg/root/panel 同值，
//     TUI 显式黑底要可见）；亮档=前景口径深色 ≥4.6（nord-light/rosepine-light
//     原先是浅色，前景隐身）；
//   ④ 亮档 ansi16bg：TUI 背景填充浅彩表（渲染端 idx_to_rgb_bg 已支持，
//     修「亮主题 TUI 黑底白字 1.05~1.55:1」）；
//   ⑤ group24 派生：26 套非门面从各套 ac 色相起环（6 hue×3 明度 + 6 灰），
//     替换共用 tailwind 表（on panel ≥3.0 验收）；
//   ⑥ _meta.ansi_check 口径修正（亮档 0 必判）+ generated 日期。
// 用法：node scripts/_audit/fix_batch3_ansi.js   （随后跑 gen_theme_rust.js）
// ═════════════════════════════════════════════════════════════════════
const fs = require('fs');
const path = require('path');
const ROOT = path.join(__dirname, '..', '..');
const P = path.join(ROOT, 'scripts', '_ansi-group.json');

const doc = JSON.parse(fs.readFileSync(P, 'utf8'));
const ui = {};
for (const f of ['theme-tokens.json', '_radix-themes.json', '_extra-themes.json']) {
  Object.assign(ui, JSON.parse(fs.readFileSync(path.join(ROOT, 'scripts', f), 'utf8')));
}
// theme-tokens 的 gruvbox-* 键对应 palettes 的 gruvbox-material-*（id 迁移遗留）
if (ui['gruvbox-dark'] && !ui['gruvbox-material-dark']) ui['gruvbox-material-dark'] = ui['gruvbox-dark'];
if (ui['gruvbox-light'] && !ui['gruvbox-material-light']) ui['gruvbox-material-light'] = ui['gruvbox-light'];

// 28 套 id 清单从 palettes.rs 读（json 里另有 meat-* 迁移档，不在选择器内）
const ids = [...new Set(
  fs.readFileSync(path.join(ROOT, 'src', 'theme', 'palettes.rs'), 'utf8')
    .match(/id: "([^"]+)"/g).map(s => s.slice(5, -1))
)].filter(id => doc[id]);

// ── 颜色工具（与 gen_ansi_group.js 同式）─────────────────────────────
const hx2rgb = h => { h = h.replace('#', ''); return [0, 2, 4].map(i => parseInt(h.slice(i, i + 2), 16)); };
const rgb2hx = (r, g, b) => '#' + [r, g, b].map(v => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, '0')).join('').toUpperCase();
const lum = h => { const [r, g, b] = hx2rgb(h).map(v => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
const cr = (a, b) => { const [l1, l2] = [lum(a), lum(b)].sort((x, y) => y - x); return (l1 + 0.05) / (l2 + 0.05); };
function rgb2hsl(h) { let [r, g, b] = hx2rgb(h).map(v => v / 255); const mx = Math.max(r, g, b), mn = Math.min(r, g, b); let hh = 0, s = 0; const l = (mx + mn) / 2; if (mx !== mn) { const d = mx - mn; s = l > 0.5 ? d / (2 - mx - mn) : d / (mx + mn); if (mx === r) hh = (g - b) / d + (g < b ? 6 : 0); else if (mx === g) hh = (b - r) / d + 2; else hh = (r - g) / d + 4; hh /= 6; } return [hh, s, l]; }
function hsl2hx(h, s, l) { if (s < 1e-6) { const v = Math.round(l * 255); return rgb2hx(v, v, v); } const q = l < 0.5 ? l * (1 + s) : l + s - l * s, p = 2 * l - q; const f = t => { if (t < 0) t += 1; if (t > 1) t -= 1; if (t < 1 / 6) return p + (q - p) * 6 * t; if (t < 0.5) return q; if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6; return p; }; return rgb2hx(f(h + 1 / 3) * 255, f(h) * 255, f(h - 1 / 3) * 255); }
const hueDeg = h => rgb2hsl(h)[0] * 360;

/// 保 H/S、沿明度收敛到 target（darken=true 向 0.02 压、false 向 0.98 提）
function fitL(hex, bg, target, darken) {
  if (cr(hex, bg) >= target) return hex;
  const [h, s, l0] = rgb2hsl(hex);
  for (let i = 1; i <= 1024; i++) {
    const t = i / 1024;
    const nl = darken ? l0 * (1 - t) : l0 + (1 - l0) * t;
    const cand = hsl2hx(h, Math.min(s, 0.9), nl);
    if (cr(cand, bg) >= target) return cand;
  }
  return darken ? hsl2hx(h, s, 0.02) : hsl2hx(h, s, 0.98);
}
/// 保 H/S、朝「远离底」方向收敛（底暗提亮、底亮压暗）
function fitAway(hex, bg, target) { return fitL(hex, bg, target, lum(bg) >= 0.5); }

// ── ① 门面手工值（方案里已验证对比度的草案）─────────────────────────
const FACADE = {
  'mterm-dark': { 6: '#45C8E0', 14: '#86DCEC', 0: '#0F1013', 4: '#5A6CF7' },
  'mterm-light': { 6: '#19617C', 14: '#12546E', 4: '#44599B' },
};
// 派生亮档（无官方发布文件）→ 彩色槽锚定配对暗档的 H/S
const ANCHOR_LIGHT = { 'nord-light': 'nord-dark', 'graphite-light': 'graphite-dark' };
// 明暗配对（锚定 + group24 家族起相用）
const PAIR = {
  'mterm-dark': 'mterm-light', 'graphite-dark': 'graphite-light', 'onedark-dark': 'onedark-light',
  'nord-dark': 'nord-light', 'catppuccin-dark': 'catppuccin-light', 'dracula-dark': 'dracula-light',
  'rosepine-dark': 'rosepine-light', 'everforest-dark': 'everforest-light', 'kanagawa-wave': 'kanagawa-lotus',
  'gruvbox-material-dark': 'gruvbox-material-light', 'ayu-mirage': 'ayu-light', 'flexoki-dark': 'flexoki-light',
  'solarized-dark': 'solarized-light', 'darcula-dark': 'darcula-light',
};

const stats = [];
for (const id of ids) {
  const t = doc[id];
  const u = ui[id];
  if (!u) { stats.push([id, '!! 缺 UI 槽位，跳过']); continue; }
  const tbg = u.tbg, panel = u.panel, dark = t.m === '暗';
  const a = t.ansi16.map(x => x.toUpperCase());
  const before = [...a];
  const note = [];
  const facade = FACADE[id] || null;

  // ② 彩色槽压齐 4.6（门面槽位/派生锚定在后面单独覆盖）
  for (const i of [1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14]) {
    const bg = dark ? tbg : tbg; // 终端底
    if (ANCHOR_LIGHT[id]) {
      // 派生亮档：H/S 取配对暗档同槽，L 从自身起收敛
      const src = doc[ANCHOR_LIGHT[id]].ansi16[i].toUpperCase();
      const [h, s] = rgb2hsl(src);
      const [, , l0] = rgb2hsl(a[i]);
      const cand = fitL(hsl2hx(h, Math.min(s, 0.55), l0), tbg, 4.6, true);
      if (cand !== a[i]) { a[i] = cand; note.push(`a${i}锚定`); }
    } else {
      const fitted = fitAway(a[i], tbg, 4.6);
      if (fitted !== a[i]) { a[i] = fitted; note.push(`a${i}压齐`); }
    }
  }

  // ① 门面手工值（覆盖压齐结果 —— a6/a14/a0 已按「选区可辨」验证过；
  //    a4 的靛蓝草案若对比不足，兜底保色相收敛，不再回退到上游蓝）
  if (facade) {
    for (const [k, v] of Object.entries(facade)) a[+k] = v;
    for (const i of [1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14]) {
      a[i] = fitAway(a[i], tbg, 4.6);
    }
    note.push('门面解耦');
  }

  // ③ slot0 口径
  if (facade && facade[0]) {
    // 已由手工值处理
  } else if (dark) {
    // 暗档：必须比 tbg 深一档（禁同值）；已比 tbg 显著深的保持
    if (lum(a[0]) >= lum(tbg) * 0.9) {
      const [h, s, l0] = rgb2hsl(tbg);
      a[0] = hsl2hx(h, s, Math.max(0.02, l0 * 0.7));
      note.push('a0独立黑');
    }
  } else {
    // 亮档：前景口径深色 ≥4.6
    const fitted = fitL(a[0], tbg, 4.6, true);
    if (fitted !== a[0]) { a[0] = fitted; note.push('a0前景口径'); }
  }

  // 亮档 7/8/15 补验（历史上 fix 过，理论上不动；不达标才动）
  if (!dark) {
    for (const i of [7, 8, 15]) a[i] = fitL(a[i], tbg, 4.6, true);
  }
  // 暗档 8 补验（#ansi-dark-grey 已修过，兜底）
  if (dark) a[8] = fitAway(a[8], tbg, 4.6);

  t.ansi16 = a;

  // ④ 亮档 ansi16bg：TUI 背景填充浅彩表
  if (!dark && !t.ansi16bg) {
    const bg16 = [];
    for (let i = 0; i < 16; i++) {
      const [h, s] = rgb2hsl(a[i]);
      if (i === 15) bg16.push('#FFFFFF');
      else if (i === 7 || i === 0) bg16.push(hsl2hx(h, Math.min(s, 0.08), 0.945));
      else bg16.push(hsl2hx(h, Math.min(s * 0.35, 0.25), 0.87));
    }
    t.ansi16bg = bg16;
    note.push('ansi16bg');
  }

  // ⑤ group24 派生（门面保持手设）
  if (!facade) {
    const g = [];
    const h0 = hueDeg(u.ac) / 360;
    const lTiers = dark ? [0.70, 0.60, 0.50] : [0.42, 0.50, 0.58];
    for (let k = 0; k < 6; k++) {
      for (const L of lTiers) {
        g.push(hsl2hx((h0 + k / 6) % 1, 0.62, L));
      }
    }
    // 灰 6 档：t3 → t1 间均布（保底按 t3 色相）
    const [gh, gs] = rgb2hsl(u.t3);
    const l3 = rgb2hsl(u.t3)[2], l1 = rgb2hsl(u.t1)[2];
    for (let k = 0; k < 6; k++) {
      const L = l3 + (l1 - l3) * (k / 5);
      g.push(hsl2hx(gh, Math.min(gs, 0.12), L));
    }
    // on panel ≥3.0 收敛（暗档提亮、亮档压深）
    t.group24 = g.map(c => fitAway(c, panel, 3.05));
    note.push('group24派生');
  }

  if (note.length) stats.push([id, note.join(' ') + `（改动 ${(t.ansi16.filter((v, i) => v !== before[i])).length} 槽）`]);
}

// ⑥ 元数据
doc._meta.generated = '2026-10-21';
doc._meta.ansi_check = '12 彩色槽(1-6,9-14) 全部明暗档 ≥4.5:1（#ansi-batch3 A 案：上游原味让位可读性）；亮色档 0/7/8/15 与暗色档 8 同判 ≥4.5；暗色档 0/7/15 按终端惯例只记录；暗色档 0 须深于 tbg（TUI 显式黑底可见）；亮档 ansi16bg 供 TUI 背景填充';

// ── 验收 ────────────────────────────────────────────────────────────
let fails = 0;
const fail = msg => { console.log('  FAIL ' + msg); fails++; };
const darkIds = ids.filter(i => doc[i].m === '暗');
const lightIds = ids.filter(i => doc[i].m === '亮');
for (const id of ids) {
  const t = doc[id], u = ui[id], tbg = u.tbg, dark = t.m === '暗';
  for (const i of [1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14]) {
    const c = cr(t.ansi16[i], tbg);
    if (c < 4.5) fail(`${id} a${i} ${t.ansi16[i]} = ${c.toFixed(2)}`);
  }
  if (!dark) for (const i of [0, 7, 8, 15]) {
    const c = cr(t.ansi16[i], tbg);
    if (c < 4.5) fail(`${id} 亮档 a${i} ${t.ansi16[i]} = ${c.toFixed(2)}`);
  }
  if (dark) {
    if (cr(t.ansi16[8], tbg) < 4.5) fail(`${id} 暗档 a8 <4.5`);
    if (lum(t.ansi16[0]) >= lum(tbg) * 0.9) fail(`${id} 暗档 a0 不比 tbg 深（TUI 黑底不可见）`);
    if (t.ansi16[0].toUpperCase() === tbg.toUpperCase()) fail(`${id} a0 == tbg`);
  }
  for (const g of t.group24) if (cr(g, u.panel) < 3.0) fail(`${id} group24 ${g} on panel <3.0`);
  if (!dark && !t.ansi16bg) fail(`${id} 亮档缺 ansi16bg`);
}
// 门面解耦判据：a6 与 ac 色相差 ≥15°，a6/a14 对选区合成底 ≥3:1
for (const fid of ['mterm-dark', 'mterm-light']) {
  const t = doc[fid], u = ui[fid];
  const mix = (() => { const [r, g, b] = hx2rgb(u.ac); const [br, bg2, bb] = hx2rgb(u.tbg); return rgb2hx(br + (r - br) * 0.4, bg2 + (g - bg2) * 0.4, bb + (b - bb) * 0.4); })();
  const dh = Math.abs(hueDeg(t.ansi16[6]) - hueDeg(u.ac));
  if (Math.min(dh, 360 - dh) < 15) fail(`${fid} a6 与 ac 色相差 ${Math.min(dh, 360 - dh).toFixed(0)}° <15°`);
  for (const i of [6, 14]) if (cr(t.ansi16[i], mix) < 3.0) fail(`${fid} a${i} 对选区合成底 ${cr(t.ansi16[i], mix).toFixed(2)} <3.0`);
  if (t.ansi16[0].toUpperCase() === u.tbg.toUpperCase() || t.ansi16[0].toUpperCase() === (u.panel || '').toUpperCase()) fail(`${fid} a0 与界面底同值`);
}
// group24 同明暗组内不逐位相同
for (const group of [darkIds, lightIds]) {
  const seen = new Map();
  for (const id of group) {
    const key = doc[id].group24.join();
    if (seen.has(key)) fail(`group24 逐位相同: ${id} ~ ${seen.get(key)}`);
    seen.set(key, id);
  }
}

fs.writeFileSync(P, JSON.stringify(doc, null, 1) + '\n');
console.log('── 变更明细 ──');
for (const [id, s] of stats) console.log(' ' + id.padEnd(22), s);
console.log(fails === 0 ? '── 验收全过 ──' : `── 验收失败 ${fails} 项 ──`);
process.exit(fails === 0 ? 0 : 1);
