// ═════════════════════════════════════════════════════════════════════
// 生成 src/theme/palettes.rs：28 套主题色表（Rust 常量）
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

// 28 套的**选择器内顺序**（暗/亮两组各自按本序过滤；门面在前）。
// #theme-dedupe 2026-10-15：38→28 精简、明暗配对补齐后的定稿顺序
// （与 gen_ansi_group.js 的 GROUPS_DEF 是同一批套的两处清单，改一处必须同步另一处）。
// （迁移保留档 meat-dark/light 不进选择器——真源数据仍在 _ansi-group.json，
//  如需恢复在 ORDER 尾部加回即可）
const ORDER = [
  'mterm-dark', 'mterm-light', 'onedark-dark', 'onedark-light',
  'graphite-dark', 'graphite-light', 'nord-dark', 'nord-light',
  'catppuccin-dark', 'catppuccin-light', 'dracula-dark', 'dracula-light',
  'rosepine-dark', 'rosepine-light', 'everforest-dark', 'everforest-light',
  'kanagawa-wave', 'kanagawa-lotus', 'gruvbox-material-dark', 'gruvbox-material-light',
  'ayu-mirage', 'ayu-light', 'flexoki-dark', 'flexoki-light',
  'solarized-dark', 'solarized-light', 'darcula-dark', 'darcula-light',
];

const hx = (s) => '0x' + s.replace('#', '').toUpperCase();
const arr = (a) => '[' + a.map(hx).join(', ') + ']';

const out = [];
out.push('// ═════════════════════════════════════════════════════════════════════');
out.push('// 28 套主题色表（★生成文件，勿手改★）——scripts/gen_theme_rust.js 产出');
out.push('// 真源：scripts/theme-tokens.json + _radix-themes.json + _extra-themes.json');
out.push('//       + _islands-dark.json（MTerm 套的上游原样副本，仅取证）+ _ansi-group.json');
out.push('//       （ANSI16 / 分组色24 / 频道色4，含来源与校验）');
out.push('// 色值来源：上游原值 / 各套的 src 标注；#theme-dedupe 定稿修正见 _ansi-group.json');
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
out.push('    /// 亮色 TUI 背景填充表（官方发布即有；当前 28 套均为 None）');
out.push('    pub ansi16bg: Option<[u32; 16]>,');
out.push('}');
out.push('');
// 强调色预设网格：**两处副本约定** —— 本表（palettes.rs）与 ui/app.slint 的网格
// 字面量需同源，改动请同步两处。（2026-10-15 #theme-dedupe-P：46 色实为 47 项、
// 副本实为 app.slint 而非 interface_panel.slint，注释随实修正。）
out.push('/// 强调色预设网格（47 色）。高保真稿 §3.8「点缀色」弹框的取值表：');
out.push('/// 点预设即写入 custom hex（预设不再是独立一档，与「自定义」合并为两态），');
out.push('/// 故本表只作**色值/名称的单一出处**，实际生效走 `AccentCfg::Custom` + 3:1 守卫。');
out.push('/// ui/app.slint 的网格字面量需与本表同源，改动请同步两处。');
out.push("pub static ACCENT_PRESETS: &[(&str, &str, u32)] = &[");
for (const [k, n, v] of [
  ['mterm-cyan', 'MTerm 青', 0x1FC8B4], ['lime', '青柠', 0xC8FF00], ['tea', '茶色', 0xFFC100],
  ['amethyst', '紫水晶色', 0xAB47BC], ['aqua', '水蓝色', 0x5CCFE6], ['aquamarine', '海蓝宝石色', 0x64FFDD],
  ['ultraviolet', '仿紫色', 0x651FFF], ['toxic', '绝命毒药色', 0x368A3F], ['magenta', '洋红', 0xE1306C],
  ['carbon', '碳素色', 0x424242], ['coffee', '咖啡色', 0x795548], ['coral', '珊瑚色', 0xFF7F6B],
  ['orange', '橙色', 0xF0862C], ['chartreuse', '蟹青色', 0xFFE831], ['royal', '宝蓝色', 0x2979F5],
  ['azure', '鲜蓝色', 0x73D8FF], ['deep-rose', '深红色', 0xE916B5], ['gold', '金色', 0xFFC069],
  ['lead', '铅黑色', 0x616161], ['indigo', '靛蓝色', 0x2F51B5], ['lavender', '薰衣草色', 0x7E57C2],
  ['white-glow', '白光色', 0xFAFAFA], ['fresh-green', '鲜绿色', 0x7CB342], ['tangerine', '橘黄色', 0xFFA565],
  ['pale-purple', '淡紫色', 0xDFBFFF], ['mint', '薄荷色', 0x95E0C1], ['neon', '霓虹色', 0x1D49B1],
  ['seastar', '海星色', 0x546E7A], ['amber', '琥珀橙', 0xFF9B0B], ['night', '夜色', 0x676E9A],
  ['moss', '苔藓色', 0xB1C784], ['plant-red', '植物红', 0xB71C1C], ['rose', '玫瑰色', 0xF279B9],
  ['salmon', '鲑鱼色', 0xF29E7B], ['sand', '沙色', 0xD9AE91], ['gray', '灰色', 0x9E9E9E],
  ['sky', '天空色', 0x64FFFF], ['slate', '石板色', 0x607D8B], ['steel', '钢色', 0xB9B119],
  ['grass-pink', '草绿粉', 0xFF40CC], ['teal', '青绿色', 0x089B8B], ['tomato', '番茄色', 0xF44333],
  ['turquoise', '绿松石色', 0x88CBBC], ['water', '水色', 0x42A5F5], ['yellow-green', '黄绿色', 0xA5FF80],
  ['brand-blue', '品牌蓝', 0x0969DA], ['jade', '翡翠绿', 0x29A383],
]) out.push(`    ("${k}", "${n}", 0x${v.toString(16).toUpperCase().padStart(6, '0')}),`);
out.push('];');
out.push('');
out.push('/// tab 关闭键 hover 色（tab-row-idia 稿：全局常量，非按主题；');
out.push('/// 不复用旧 #e81123——那是「红底白字」方案的底色，作前景亮色档只有 3.59:1）');
out.push('pub const DANGER_STRONG: u32 = 0xDC4B44;');
out.push('/// 频道暂停态的暖色竖条（状态条左缘；不用暖底色——一转暖就成「另一个面板」）');
out.push('pub const CHANNEL_PAUSE_BAR: u32 = 0xE2A84A;');
out.push('');
out.push('/// 全部 28 套（选择器实际只按明暗分两组，组内即本表顺序）；kind 即族：');
out.push('/// radix 2（石墨）/ idea 1（Darcula · 暗）/ mterm 2（MTerm · 暗 / 亮）/ community 23');
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
out.push('/// 按 id 查表（线性 28 项，调用点均为低频切主题路径）');
out.push("pub fn find(id: &str) -> Option<&'static Palette> {");
out.push('    PALETTES.iter().find(|p| p.id == id)');
out.push('}');
out.push('');
out.push('/// 默认主题（#theme-dedupe 2026-10-15 定稿：门面 MTerm 两套；新老用户一律新底色）');
out.push('pub const DEFAULT_DARK: &str = "mterm-dark";');
out.push('pub const DEFAULT_LIGHT: &str = "mterm-light";');

fs.writeFileSync(path.join(__dirname, '..', 'src', 'theme', 'palettes.rs'), out.join('\n') + '\n', 'utf8');
console.log('src/theme/palettes.rs 生成：', out.length, '行，', ORDER.length, '套');
const bad = ORDER.filter(id => !AG[id] || !ALL_UI[id]);
if (bad.length) { console.error('缺数据:', bad); process.exit(1); }
