// 4 阶段断言的数据预验证：mterm 两套 × §6.3/6.4 计划断言
// 用法: node scripts/check_mterm_assertions.js
const fs = require('fs');
const path = require('path');
const src = fs.readFileSync(path.join(__dirname, '..', 'src', 'theme', 'palettes.rs'), 'utf8');
const blocks = src.split(/\n    Palette \{/).slice(1);
const parse = (b) => {
  const id = /id:\s*"([^"]+)"/.exec(b)[1];
  const one = (k) => parseInt(new RegExp(k + ':\\s*0x([0-9A-Fa-f]+)').exec(b)[1], 16);
  const many = (k) => new RegExp(k + ':\\s*\\[([^\\]]+)\\]').exec(b)[1]
    .split(',').map((s) => parseInt(s.trim().replace('0x', ''), 16));
  return {
    id,
    u: { panel: one('panel'), t2: one('t2'), t3: one('t3'), tbg: one('tbg'), tab: one('tab'), ac: one('ac') , ok: one('ok'), wr: one('wr'), dg: one('dg') },
    g24: many('group24'),
    ansi: many('ansi16'),
    ch: many('channel'),
  };
};
const all = blocks.map(parse);
const lum = (c) => { const f = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
  return 0.2126 * f((c >> 16) & 255) + 0.7152 * f((c >> 8) & 255) + 0.0722 * f(c & 255); };
const cr = (a, b) => { const l1 = lum(a), l2 = lum(b); return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05); };
const hue = (c) => { const r = ((c >> 16) & 255) / 255, g = ((c >> 8) & 255) / 255, b = (c & 255) / 255;
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn; if (!d) return 0;
  let h = mx === r ? ((g - b) / d) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
  h *= 60; if (h < 0) h += 360; return h; };
const dh = (a, b) => { const d = Math.abs(hue(a) - hue(b)); return Math.min(d, 360 - d); };

for (const t of all.filter((x) => x.id.startsWith('mterm'))) {
  console.log('=== ' + t.id + ' ===');
  console.log('  t3 on panel =', cr(t.u.t3, t.u.panel).toFixed(2), '(≥3.0? )');
  console.log('  t2==t3?', t.u.t2 === t.u.t3, '| panel==tbg?', t.u.panel === t.u.tbg);
  const map = [['ac↔a6', t.u.ac, t.ansi[6]], ['ok↔a2', t.u.ok, t.ansi[2]], ['wr↔a3', t.u.wr, t.ansi[3]], ['dg↔a1', t.u.dg, t.ansi[1]]];
  console.log('  语义映射 Δhue:', map.map(([n, a, b]) => n + '=' + dh(a, b).toFixed(0) + '°').join(' '));
  const dup = all.filter((x) => x.id !== t.id && JSON.stringify(x.g24) === JSON.stringify(t.g24)).map((x) => x.id);
  console.log('  g24 与其他套逐位相同:', dup.length ? dup.join(',') : '无 ✓');
  const low = [1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14].filter((i) => cr(t.ansi[i], t.u.tbg) < 4.5);
  console.log('  彩槽 <4.5 on tbg:', low.length ? low.join(',') : '无 ✓');
  const chBad = t.ch.filter((c) => cr(c, t.u.tab) < 3.0 || cr(c, t.u.tbg) < 3.0).length;
  console.log('  频道色双侧 <3.0 个数:', chBad);
}
