// #theme-dedupe 阶段 1 一次性迁移脚本
// ① 28 套目标态：u(20 槽) + ch(4) 写入三个 tokens json；zh/kind 微调
// ② 下线 12 套 + github-light 的数据从 json 删除（生成端由 GROUPS_DEF/ORDER 控制）
// ③ 输出 REVISED_ANSI(16 套定稿 ANSI) 与 FINAL_GROUP24(5 套) 的代码段 → output/
// 用法: node scripts/migrate_stage1_data.js
const fs = require('fs');
const path = require('path');
const D = require('../output/theme-dedupe-target.json');
const FILES = ['theme-tokens.json', '_extra-themes.json', '_radix-themes.json'];
const docs = {};
for (const f of FILES) {
  const raw = fs.readFileSync(path.join(__dirname, f), 'utf8');
  docs[f] = { raw, obj: JSON.parse(raw) };
}
const U2J = { dg_s: 'dgS' };
const log = [];

// ── ① 28 套 u + ch 全量写入（目标 u = { ...t.u, ...opt 的 20 槽 }；ch 直接取 t.ch）──
for (const t of D.keep) {
  let file = null;
  for (const f of FILES) if (docs[f].obj[t.id]) { file = f; break; }
  if (!file) { log.push('!! MISS ' + t.id); continue; }
  const o = docs[file].obj[t.id];
  const changes = [];
  const optU = {};
  for (const k of Object.keys(t.u)) if (t.opt && t.opt[k] !== undefined) optU[k] = t.opt[k];
  const tU = { ...t.u, ...optU };
  for (const [k, v] of Object.entries(tU)) {
    const jk = U2J[k] || k;
    const vU = v.toUpperCase();
    if ((o[jk] || '').toUpperCase() !== vU) { changes.push(`${jk}:${o[jk]}→${vU}`); o[jk] = vU; }
  }
  if (JSON.stringify(o.ch.map(x => x.toUpperCase())) !== JSON.stringify(t.ch.map(x => x.toUpperCase()))) {
    changes.push('ch:[' + o.ch.join(',') + ']→[' + t.ch.join(',') + ']');
    o.ch = [...t.ch];
  }
  if ((t.id === 'onedark-dark' || t.id === 'onedark-light') && o.f !== '原子') {
    changes.push(`f:${o.f}→原子`);
    o.f = '原子';
  }
  if (t.id === 'darcula-light' && o.kind !== 'community') {
    changes.push(`kind:${o.kind}→community`);
    o.kind = 'community';
  }
  log.push((changes.length ? `改 ${String(changes.length).padStart(2)} 项  ` : '不变     ') + t.id.padEnd(22) + '@' + file.padEnd(18) +
    (changes.length ? ' [' + changes.slice(0, 3).join('; ') + (changes.length > 3 ? ` …+${changes.length - 3}` : '') + ']' : ''));
}

// ── ② 下线数据删除（12 套 + github-light；数据在 git 历史可查）──
const DROP = ['slate-dark', 'slate-light', 'mauve-dark', 'mauve-light', 'sage-dark', 'sage-light',
  'sand-dark', 'sand-light', 'tokyonight-dark', 'tokyonight-light', 'ayu-dark', 'github-dark', 'github-light'];
for (const id of DROP) {
  for (const f of FILES) {
    if (docs[f].obj[id]) { delete docs[f].obj[id]; log.push('删数据 ' + id + ' @' + f); }
  }
}

// ── 写回 ──
for (const f of FILES) {
  const out = JSON.stringify(docs[f].obj, null, 1).replace(/\n/g, '\r\n');
  if (out !== docs[f].raw) { fs.writeFileSync(path.join(__dirname, f), out, 'utf8'); log.push('写回 ' + f + ' (delta ' + (out.length - docs[f].raw.length) + 'B)'); }
  else log.push('无变化 ' + f);
}

// ── ③ 生成 REVISED_ANSI / FINAL_GROUP24 段 ──
// REVISED = 需与上游原值解耦的定稿：现有套的算子修正 + 新增套定稿（带完整 src）
// 基准取 **git HEAD 版** 的 _ansi-group.json（38 套旧结构）：用于计算"修正了哪些槽"
// 与干净 src。不能读当前版 —— 它已被生成器更新为定稿，幂等重跑会丢修正列表。
let AGC = {};
try {
  AGC = JSON.parse(require('child_process').execSync('git show HEAD:scripts/_ansi-group.json', { maxBuffer: 16 * 1024 * 1024 }).toString());
} catch (e) { console.warn('!! 无法读取 HEAD 版 _ansi-group.json（修正列表将降级为空）：', e.message); }
const RENAME_OLD = { 'mterm-dark': 'islands-dark', 'mterm-light': 'islands-light', 'darcula-dark': 'idea-dark', 'darcula-light': 'idea-light', 'onedark-light': 'github-light' };
const REVISED = ['mterm-dark', 'catppuccin-light', 'rosepine-light', 'everforest-dark', 'gruvbox-material-dark',
  'gruvbox-material-light', 'ayu-light', 'flexoki-dark', 'flexoki-light', 'solarized-dark', 'darcula-dark',
  'solarized-light', 'darcula-light', 'dracula-light', 'everforest-light', 'onedark-light'];
const NEW_SRC = {
  'dracula-light': 'Dracula 官方 Alucard 色板重制（#theme-dedupe 定稿：明暗配对补齐，亮底对比度校正）',
  'everforest-light': 'Everforest 官方亮档重制（#theme-dedupe 定稿：明暗配对补齐，亮底对比度校正）',
  'onedark-light': '官方 One Light 全套（Atom One Light，nathanbuchar/atom-one-dark-terminal 同源收录）；#theme-dedupe 重着色定稿',
};
const hex = (h) => "'" + h.toUpperCase() + "'";   // 供插入 gen_ansi_group.js 的 JS 段（字符串形式 '#RRGGBB'）
const ansiLines = ['// ── 定稿 ANSI：与「上游原值」解耦的 28 套（#theme-dedupe 2026-10-15 阶段一）──',
  '// 两类来源：① 现有套经 O3（bright 档位移）/ O5（亮档彩槽对比度）修正后的定稿；',
  '//           ② 新增亮档（dracula-light / everforest-light / onedark-light）的自有定稿。',
  '// src 即最终溯源（含修正记录）。未列于此的套走上游原值路径（OFFICIAL/OWN/RADIX 派生）。',
  'const REVISED_ANSI = {'];
for (const id of REVISED) {
  const t = D.keep.find(x => x.id === id);
  const targetAnsi = (t.opt && t.opt.ansi) ? t.opt.ansi : t.ansi;
  const old = AGC[RENAME_OLD[id] || id];
  const cur = old ? old.ansi16.map(x => x.toUpperCase()) : null;
  const tgt = targetAnsi.map(x => x.toUpperCase());
  let src;
  if (old && old.src) {
    const slots = [];
    for (let i = 0; i < 16; i++) if (cur[i] !== tgt[i]) slots.push('a' + i);
    src = old.src + (slots.length ? '；#theme-dedupe 定稿（修正 ' + slots.join('/') + '）' : '；#theme-dedupe 定稿');
  } else {
    src = NEW_SRC[id];
  }
  const rows = [];
  for (let i = 0; i < 16; i += 4) rows.push('    ' + targetAnsi.slice(i, i + 4).map(hex).join(', ') + ',');
  ansiLines.push(`  '${id}': {`);
  ansiLines.push(`    src: ${JSON.stringify(src)},`);
  ansiLines.push('    ansi: [');
  ansiLines.push(rows.join('\n'));
  ansiLines.push('    ],');
  ansiLines.push('  },');
}
ansiLines.push('};');

const g24Lines = ['// ── 门面/重制套的分组色 24（#theme-dedupe O4 定稿）──',
  '// mterm 两套 = 门面专属设计（以自身 ANSI 色相为锚点，24 色无重复、全部 ≥3.0:1 on panel）；',
  '// 其余三套 = 亮档重制同批设计。未列于此的套走 GROUP_DARK/LIGHT + fitL 原路径。',
  'const FINAL_GROUP24 = {'];
for (const id of ['mterm-dark', 'mterm-light', 'onedark-light', 'solarized-light', 'darcula-light', 'dracula-light', 'everforest-light']) {
  const t = D.keep.find(x => x.id === id);
  const g24 = (t.opt && t.opt.g24) ? t.opt.g24 : t.g24;
  const rows = [];
  for (let i = 0; i < 24; i += 4) rows.push('    ' + g24.slice(i, i + 4).map(hex).join(', ') + ',');
  g24Lines.push(`  '${id}': [`);
  g24Lines.push(rows.join('\n'));
  g24Lines.push('  ],');
}
g24Lines.push('};');

fs.writeFileSync(path.join(__dirname, '..', 'output', 'revised_ansi.txt'), ansiLines.join('\n'), 'utf8');
fs.writeFileSync(path.join(__dirname, '..', 'output', 'final_group24.txt'), g24Lines.join('\n'), 'utf8');
log.push('输出 output/revised_ansi.txt (' + REVISED.length + ' 套) 与 output/final_group24.txt (5 套)');
console.log(log.join('\n'));
