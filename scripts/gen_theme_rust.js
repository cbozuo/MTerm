// ═════════════════════════════════════════════════════════════════════
// 生成 src/theme/palettes.rs：37 套主题色表（Rust 常量）
//
// 输入（真源，均可由各自生成器重跑）：
//   theme-tokens.json + _radix-themes.json + _extra-themes.json  → UI 20 槽
//   _ansi-group.json（gen_ansi_group.js 产出）                    → ANSI16/分组色24/频道色4
//
// ⚠️ 生成文件，勿手改。用法：node scripts/gen_theme_rust.js
// ═════════════════════════════════════════════════════════════════════
const fs = require('fs');
const path = require('path');

const TOKENS = JSON.parse(fs.readFileSync(path.join(__dirname, 'theme-tokens.json'), 'utf8'));
const RADIX_T = JSON.parse(fs.readFileSync(path.join(__dirname, '_radix-themes.json'), 'utf8'));
const EXTRA = JSON.parse(fs.readFileSync(path.join(__dirname, '_extra-themes.json'), 'utf8'));
const AG = JSON.parse(fs.readFileSync(path.join(__dirname, '_ansi-group.json'), 'utf8'));
const ALL_UI = { ...TOKENS, ...RADIX_T, ...EXTRA };

// 与 gen_ansi_group.js 的 GROUPS_DEF/VARIANTS 一致（品牌族 github/idea/islands
// 紧随 radix 之后，终端族在后 —— 即选择器里的族顺序）
// （2026-10-07 用户决策：迁移保留档 meat-dark/light 不进选择器——真源数据仍在
//  _ansi-group.json，如需恢复在 ORDER 尾部加回即可）
const ORDER = [
  'graphite-dark', 'graphite-light', 'slate-dark', 'slate-light', 'mauve-dark', 'mauve-light',
  'sage-dark', 'sage-light', 'sand-dark', 'sand-light',
  'github-dark', 'github-light', 'idea-dark', 'idea-light',
  'islands-dark',
  'dracula-dark', 'catppuccin-dark', 'catppuccin-light', 'solarized-dark', 'solarized-light',
  'kanagawa-wave', 'kanagawa-lotus', 'ayu-dark', 'ayu-mirage', 'ayu-light',
  'gruvbox-material-dark', 'gruvbox-material-light',
  'tokyonight-dark', 'tokyonight-light', 'nord-dark', 'nord-light', 'everforest-dark',
  'rosepine-dark', 'rosepine-light', 'onedark-dark', 'flexoki-dark', 'flexoki-light',
];

const hx = (s) => '0x' + s.replace('#', '').toUpperCase();
const arr = (a) => '[' + a.map(hx).join(', ') + ']';

const out = [];
out.push('// ═════════════════════════════════════════════════════════════════════');
out.push('// 37 套主题色表（★生成文件，勿手改★）——scripts/gen_theme_rust.js 产出');
out.push('// 真源：scripts/theme-tokens.json + _radix-themes.json + _extra-themes.json');
out.push('//       + _islands-dark.json（MTerm 套的上游原样副本，仅取证）+ _ansi-group.json');
out.push('//       （ANSI16 / 分组色24 / 频道色4，含来源与校验）');
out.push('// 全部色值为官方发布原样（个别派生档在 _ansi-group.json 的 src 字段标注）。');
out.push('// ═════════════════════════════════════════════════════════════════════');
out.push('');
out.push('/// 单套主题的全部色槽。色值为 0xRRGGBB。');
out.push("pub struct Palette {");
out.push("    pub id: &'static str,");
out.push('    /// 中文名（不含明暗后缀，如「石墨」）');
out.push("    pub zh: &'static str,");
out.push('    pub dark: bool,');
out.push('    /// radix | brand | terminal');
out.push("    pub kind: &'static str,");
out.push('    pub root: u32, pub panel: u32, pub palt: u32, pub elev: u32,');
out.push('    pub hov: u32, pub act: u32, pub tab: u32, pub line: u32, pub lstr: u32,');
out.push('    pub t1: u32, pub t2: u32, pub t3: u32,');
out.push('    pub tbg: u32, pub tfg: u32,');
out.push('    pub ac: u32, pub ac2: u32, pub ok: u32, pub wr: u32, pub dg: u32, pub dg_s: u32,');
out.push('    /// 频道色 4 槽（该套的 tab 底与选中底双 ≥3.0，见 _ansi-group.json）');
out.push('    pub channel: [u32; 4],');
out.push('    /// 分组色 24 色（on panel ≥3.0）');
out.push('    pub group24: [u32; 24],');
out.push('    /// 终端 ANSI 16 色（前景用）');
out.push('    pub ansi16: [u32; 16],');
out.push('    /// 亮色 TUI 背景填充表（官方发布即有；当前 37 套均为 None）');
out.push('    pub ansi16bg: Option<[u32; 16]>,');
out.push('}');
out.push('');
out.push('/// 9 个强调色预设（§4-L2/§6 已定：全部出自某套主题自己的 accent）');
out.push("pub static ACCENT_PRESETS: &[(&str, &str, u32)] = &[");
out.push('    ("classic-blue", "经典蓝", 0x0090FF), // Radix 石墨');
out.push('    ("cyan", "青", 0x00A2C7),           // Radix 石板');
out.push('    ("iris", "靛紫", 0x5B5BD6),         // Radix 藕荷');
out.push('    ("jade", "翡翠", 0x29A383),         // Radix 青苔');
out.push('    ("amber", "琥珀", 0xFFC53D),        // Radix 暖砂');
out.push('    ("tomato", "番茄", 0xE54D2E),       // Radix 橄榄（已删族，色值仍为其 accent）');
out.push('    ("neon-blue", "霓蓝", 0x7AA2F7),    // Tokyo Night');
out.push('    ("frost-cyan", "霜青", 0x88C0D0),   // Nord');
out.push('    ("dusk-purple", "暮紫", 0xBD93F9),  // Dracula');
out.push('];');
out.push('');
out.push('/// tab 关闭键 hover 色（tab-row-idia 稿：全局常量，非按主题；');
out.push('/// 不复用旧 #e81123——那是「红底白字」方案的底色，作前景亮色档只有 3.59:1）');
out.push('pub const DANGER_STRONG: u32 = 0xDC4B44;');
out.push('/// 频道暂停态的暖色竖条（状态条左缘；不用暖底色——一转暖就成「另一个面板」）');
out.push('pub const CHANNEL_PAUSE_BAR: u32 = 0xE2A84A;');
out.push('');
out.push('/// 全部 37 套（选择器实际只按明暗分两组，组内即本表顺序）；kind 即族：');
out.push('/// radix 10 / brand 2（GitHub）/ idea 2 / mterm 1 / community 22');
out.push('pub static PALETTES: &[Palette] = &[');

for (const id of ORDER) {
  const ag = AG[id];
  if (!ag) { console.error('缺 _ansi-group:', id); process.exit(1); }
  const ui = ALL_UI[id];
  if (!ui) { console.error('缺 UI 槽:', id); process.exit(1); }
  const dark = ui.m === '暗' || ui.dark === true;
  const kind = ui.kind || ui.group || 'terminal';
  const name = ui.f;
  const g = (k) => hx(ui[k]);
  out.push('    // ' + name + '·' + ui.m + (ag.src ? ' —— ' + ag.src.replace(/<\/?code>/g, '') : ''));
  out.push('    Palette {');
  out.push('        id: "' + id + '", zh: "' + name + '", dark: ' + dark + ', kind: "' + kind + '",');
  out.push('        root: ' + g('root') + ', panel: ' + g('panel') + ', palt: ' + g('palt') + ', elev: ' + g('elev') + ',');
  out.push('        hov: ' + g('hov') + ', act: ' + g('act') + ', tab: ' + g('tab') + ', line: ' + g('line') + ', lstr: ' + g('lstr') + ',');
  out.push('        t1: ' + g('t1') + ', t2: ' + g('t2') + ', t3: ' + g('t3') + ',');
  out.push('        tbg: ' + g('tbg') + ', tfg: ' + g('tfg') + ',');
  out.push('        ac: ' + g('ac') + ', ac2: ' + g('ac2') + ', ok: ' + g('ok') + ', wr: ' + g('wr') + ', dg: ' + g('dg') + ', dg_s: ' + g('dgS') + ',');
  out.push('        channel: ' + arr(ag.channel) + ',');
  out.push('        group24: ' + arr(ag.group24) + ',');
  out.push('        ansi16: ' + arr(ag.ansi16) + ',');
  out.push('        ansi16bg: ' + (ag.ansi16bg ? 'Some(' + arr(ag.ansi16bg) + ')' : 'None') + ',');
  out.push('    },');
}
out.push('];');
out.push('');
out.push('/// 按 id 查表（线性 37 项，调用点均为低频切主题路径）');
out.push("pub fn find(id: &str) -> Option<&'static Palette> {");
out.push('    PALETTES.iter().find(|p| p.id == id)');
out.push('}');
out.push('');
out.push('/// 默认主题（已确认决策：新老用户一律用新底色——graphite 族）');
out.push('pub const DEFAULT_DARK: &str = "graphite-dark";');
out.push('pub const DEFAULT_LIGHT: &str = "graphite-light";');

fs.writeFileSync(path.join(__dirname, '..', 'src', 'theme', 'palettes.rs'), out.join('\n') + '\n', 'utf8');
console.log('src/theme/palettes.rs 生成：', out.length, '行，', ORDER.length, '套');
const bad = ORDER.filter(id => !AG[id] || !ALL_UI[id]);
if (bad.length) { console.error('缺数据:', bad); process.exit(1); }
