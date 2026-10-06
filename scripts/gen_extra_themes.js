// ═════════════════════════════════════════════════════════════════════
// 主题重构（2026-10-07 第二轮）：删 8 套 + 增 11 套 → 36 预设
//
// 删除（8）：gruvbox-dark/light（授权未声明）、dracula-light/Alucard（官方
//   明说不是亮色档，挂名困惑）、onedark-light（纯自造，违反"不硬凑档位"）、
//   darcula（与 idea-dark 重叠，New UI 是现代默认）、olive-dark/light
//   （与 sage/sand 色温重叠）、everforest-light（官方灰阶撑不起正文，待定②收口）
// 新增（11）：kanagawa-wave/lotus、github-dark/light、ayu-dark/mirage/light、
//   flexoki-dark/light、gruvbox-material-dark/light —— 全部 MIT 实测确认
//   （2026-10-07 gh api：kanagawa 6420★ / primer 413★ / ayu 857★ /
//   flexoki 3686★ / gruvbox-material 2649★）
//
// 本文件产出 scripts/_extra-themes.json（11 套新主题的 UI 语义槽位），
// 结构与 theme-tokens.json 同构。槽位组装规则：
//   ① 官方调色板有明确键 → 直接取官方值（ANSI、fg/bg/accent、色相）
//   ② 官方只有 bg/fg 两档（终端色系常态）→ 面板阶在官方 bg 阶内取档，
//      并在 src 注明；不引入官方不存在的色相
// ANSI 16 色不在此文件（gen_ansi_group.js 的 OFFICIAL_ANSI 统一管理）。
// ═════════════════════════════════════════════════════════════════════

const T = {};

// ── 神奈川 Kanagawa（rebelot/kanagawa.nvim，MIT，6420★）───────────────
// 槽位取自 lua/kanagawa/colors.lua 官方 palette（wave/lotus 逐键）
T['kanagawa-wave'] = {
  f: '神奈川', en: 'Kanagawa', m: '暗', kind: 'community', group: 'terminal',
  star: 6420, lic: 'MIT', repo: 'rebelot/kanagawa.nvim',
  desc: '灵感取自葛饰北斋《神奈川冲浪里》：<b>墨蓝底 + 浮世绘秋色</b>。低饱和高内聚，'
      + '是 nvim 生态近年 top 级配色。官方 wave（暗）/ lotus（亮）双档。',
  src: '官方 palette wave 逐键：sumiInk0-5 面板阶 / fujiWhite-fujiGray 文本 / crystalBlue 强调',
  root: '#16161D', panel: '#1F1F28', palt: '#2A2A37', elev: '#363646',
  hov: '#2A2A37', act: '#363646', tab: '#181820',
  line: '#2A2A37', lstr: '#363646',
  t1: '#DCD7BA', t2: '#727169', t3: '#727169',
  tbg: '#1F1F28', tfg: '#DCD7BA',
  ac: '#7E9CD8', ac2: '#7FB4CA',
  ok: '#98BB6C', wr: '#FF9E3B', dg: '#E82424', dgS: '#C34043',
  ch: ['#7FB4CA', '#98BB6C', '#FF9E3B', '#D27E99'],
};
T['kanagawa-lotus'] = {
  f: '神奈川', en: 'Kanagawa', m: '亮', kind: 'community', group: 'terminal',
  star: 6420, lic: 'MIT', repo: 'rebelot/kanagawa.nvim',
  desc: '神奈川的官方亮色档 <b>lotus</b>：和纸米黄底 + 靛蓝秋色，与 wave 同源的日式低饱和。',
  src: '官方 palette lotus 逐键：lotusWhite 阶 / lotusInk 文本 / lotusBlue4 强调',
  root: '#f2ecbc', panel: '#e5ddb0', palt: '#dcd5ac', elev: '#d5cea3',
  hov: '#dcd5ac', act: '#d5cea3', tab: '#e7dba0',
  line: '#a09cac', lstr: '#c9cbd1',
  t1: '#545464', t2: '#716e61', t3: '#8a8980',
  tbg: '#f2ecbc', tfg: '#545464',
  ac: '#4d699b', ac2: '#6693bf',
  ok: '#6f894e', wr: '#de9800', dg: '#c84053', dgS: '#d7474b',
  ch: ['#6693bf', '#6f894e', '#de9800', '#b35b79'],
};

// ── GitHub（Primer 官方，MIT，primitives 413★ · 官方出品）──────────────
// UI 语义槽位取 Primer dark.default / light.default（canvas/fg/accent 语义 token 值）
T['github-dark'] = {
  f: 'GitHub', en: 'GitHub', m: '暗', kind: 'brand', group: 'brand',
  star: 0, lic: 'MIT', repo: 'primer/primitives',
  desc: '<b>程序员每天在看的界面</b>。UI 取 Primer 官方 dark.default 语义 token，'
      + '终端取 GitHub 官方 VS Code 主题的 terminal 色。认知度即熟悉度。',
  src: 'Primer dark.default：canvas.default #0d1117 / canvas.subtle #161b22 / fg.default #e6edf3 / accent #2f81f7',
  root: '#0d1117', panel: '#161b22', palt: '#21262d', elev: '#2d333b',
  hov: '#293038', act: '#32383e', tab: '#161b22',
  line: '#30363d', lstr: '#444c56',
  t1: '#e6edf3', t2: '#9198a1', t3: '#6e7681',
  tbg: '#0d1117', tfg: '#e6edf3',
  ac: '#2f81f7', ac2: '#1f6feb',
  ok: '#3fb950', wr: '#d29922', dg: '#f85149', dgS: '#da3633',
  ch: ['#58a6ff', '#3fb950', '#d29922', '#bc8cff'],
};
T['github-light'] = {
  f: 'GitHub', en: 'GitHub', m: '亮', kind: 'brand', group: 'brand',
  star: 0, lic: 'MIT', repo: 'primer/primitives',
  desc: 'GitHub 官方亮色（light.default）：白纸底 + 高饱和语义色，与暗档同一套 token 体系。',
  src: 'Primer light.default：canvas #ffffff / canvas.subtle #f6f8fa / fg #1f2328 / accent #0969da',
  root: '#ffffff', panel: '#f6f8fa', palt: '#eff2f5', elev: '#e7ebef',
  hov: '#f3f4f6', act: '#e7ebef', tab: '#f6f8fa',
  line: '#d0d7de', lstr: '#afb8c1',
  t1: '#1f2328', t2: '#656d76', t3: '#6e7781',
  tbg: '#ffffff', tfg: '#1f2328',
  ac: '#0969da', ac2: '#0550ae',
  ok: '#1a7f37', wr: '#9a6700', dg: '#cf222e', dgS: '#a40e26',
  ch: ['#0969da', '#1a7f37', '#9a6700', '#8250df'],
};

// ── Ayu（ayu-theme，MIT，857★）───────────────────────────────────────
// bg/fg/accent 取官方三档；面板阶在官方 bg 阶内组装（src 注明）
T['ayu-dark'] = {
  f: 'Ayu', en: 'Ayu', m: '暗', kind: 'community', group: 'terminal',
  star: 857, lic: 'MIT', repo: 'ayu-theme/ayu-colors',
  desc: 'VS Code 百万安装量的网红配色：<b>深蓝黑底 + 橘金强调</b>。官方 dark / mirage / light 三档，'
      + 'mirage 是官方独有的中间档。',
  src: '官方 dark：bg #0B0E14 / fg #BFBDB6 / accent #FFB454；面板阶在官方 bg 阶内组装',
  root: '#0B0E14', panel: '#0F131A', palt: '#131721', elev: '#171C24',
  hov: '#131721', act: '#171C24', tab: '#0F131A',
  line: '#171B24', lstr: '#262D3A',
  t1: '#BFBDB6', t2: '#8A9199', t3: '#6C7380',
  tbg: '#0B0E14', tfg: '#B3B1AD',
  ac: '#FFB454', ac2: '#F29668',
  ok: '#AAD94C', wr: '#E6B450', dg: '#F07178', dgS: '#E2655F',
  ch: ['#59C2FF', '#AAD94C', '#FFB454', '#D2A6FF'],
};
T['ayu-mirage'] = {
  f: 'Ayu', en: 'Ayu', m: '暗', kind: 'community', group: 'terminal',
  star: 857, lic: 'MIT', repo: 'ayu-theme/ayu-colors',
  desc: '官方独有的<b>中间档</b>：比 dark 亮、比 light 深，蓝灰底 + 金强调，'
      + '暗环境里不刺眼、亮环境里不发散。',
  src: '官方 mirage：bg #1F2430 / fg #D9D7CE / accent #FFCC66；面板阶在官方 bg 阶内组装',
  root: '#1F2430', panel: '#242936', palt: '#2A3140', elev: '#30354A',
  hov: '#2A3140', act: '#30354A', tab: '#242936',
  line: '#31405A', lstr: '#3A4151',
  t1: '#D9D7CE', t2: '#8A9199', t3: '#5C6773',
  tbg: '#1F2430', tfg: '#CBCCC6',
  ac: '#FFCC66', ac2: '#FFD173',
  ok: '#58C49C', wr: '#FFD173', dg: '#F08778', dgS: '#F58C7D',
  ch: ['#65BDDB', '#58C49C', '#FFD173', '#D2A6FF'],
};
T['ayu-light'] = {
  f: 'Ayu', en: 'Ayu', m: '亮', kind: 'community', group: 'terminal',
  star: 857, lic: 'MIT', repo: 'ayu-theme/ayu-colors',
  desc: 'Ayu 的官方亮档：暖白底 + 橘强调，同一套橘金语言的亮色表达。',
  src: '官方 light：bg #FCFCFC / fg #5C6670 / accent #FF9940；面板阶在官方 bg 阶内组装',
  root: '#FCFCFC', panel: '#F1F2F4', palt: '#E7E9EB', elev: '#DDE1E5',
  hov: '#E7E9EB', act: '#DDE1E5', tab: '#F1F2F4',
  line: '#E2E5E9', lstr: '#CFD4DA',
  t1: '#5C6670', t2: '#8A9199', t3: '#A6ABB5',
  tbg: '#FCFCFC', tfg: '#5C6166',
  ac: '#FF9940', ac2: '#F98D3D',
  ok: '#86B300', wr: '#F2AE49', dg: '#E7666A', dgS: '#ED666D',
  ch: ['#4196DF', '#86B300', '#FF9940', '#A37ACC'],
};

// ── 墨纸 Flexoki（kepano，MIT，3686★）───────────────────────────────
// 官方 -600/-400 双档色相 + 黑/纸双 6 阶，槽位全为官方键
T['flexoki-dark'] = {
  f: '墨纸', en: 'Flexoki', m: '暗', kind: 'community', group: 'terminal',
  star: 3686, lic: 'MIT', repo: 'kepano/flexoki',
  desc: 'Obsidian 作者 Steph Ango 的纸墨配色：<b>近黑底 + 纸白文字 + 高饱和色相</b>，'
      + '官方直接发布终端 ANSI 映射，dark/light 双档。',
  src: '官方 -900~-600 黑阶 / tx 阶 / 色相 -400 档（README 调色板表逐键）',
  root: '#100F0F', panel: '#1C1B1A', palt: '#282726', elev: '#343331',
  hov: '#282726', act: '#343331', tab: '#1C1B1A',
  line: '#403E3C', lstr: '#575653',
  t1: '#CECDC3', t2: '#878580', t3: '#6F6E69',
  tbg: '#282726', tfg: '#FFFCF0',
  ac: '#DA702C', ac2: '#BC5215',
  ok: '#879A39', wr: '#D0A215', dg: '#D14D41', dgS: '#AF3029',
  ch: ['#4385BE', '#3AA99F', '#D0A215', '#CE5D97'],
};
T['flexoki-light'] = {
  f: '墨纸', en: 'Flexoki', m: '亮', kind: 'community', group: 'terminal',
  star: 3686, lic: 'MIT', repo: 'kepano/flexoki',
  desc: '墨纸的官方亮档：纸色底（#FFFCF0，不是纯白）+ 墨黑文字，-600 深档色相，'
      + '长时间阅读的纸感取向。',
  src: '官方 paper 阶 / tx 阶 / 色相 -600 档（README 调色板表逐键）',
  root: '#FFFCF0', panel: '#F2F0E5', palt: '#E6E4D9', elev: '#DAD8CE',
  hov: '#E6E4D9', act: '#DAD8CE', tab: '#F2F0E5',
  line: '#CECDC3', lstr: '#A6A69C',
  t1: '#100F0F', t2: '#575653', t3: '#6F6E69',
  tbg: '#FFFCF0', tfg: '#100F0F',
  ac: '#BC5215', ac2: '#205EA6',
  ok: '#66800B', wr: '#AD8301', dg: '#AF3029', dgS: '#942822',
  ch: ['#205EA6', '#66800B', '#AD8301', '#A02F6F'],
};

// ── 岩焙 Gruvbox Material（sainnhe，MIT，2649★）──────────────────────
// 与 Everforest 同作者的 Gruvbox 授权干净版；槽位取 autoload/gruvbox_material.vim
// 官方 medium palette（palette1 bg 阶 + palette2 fg/彩色）
T['gruvbox-material-dark'] = {
  f: '岩焙', en: 'Gruvbox Material', m: '暗', kind: 'community', group: 'terminal',
  star: 2649, lic: 'MIT', repo: 'sainnhe/gruvbox-material',
  desc: '<b>Gruvbox 的授权干净版</b>（原版仓库无 LICENSE 文件）：暖棕底褪掉刺眼的高饱和，'
      + '对比度重新调平。同作者同风格，商用分发无授权疑虑。',
  src: '官方 medium palette：bg0 #282828 / bg1 #32302f / bg3 #45403d / bg5 #5a524c / fg0 #d4be98；灰阶沿用 gruvbox 官方 grey',
  root: '#282828', panel: '#32302f', palt: '#45403d', elev: '#5a524c',
  hov: '#45403d', act: '#5a524c', tab: '#282828',
  line: '#504945', lstr: '#5a524c',
  t1: '#d4be98', t2: '#a89984', t3: '#928374',
  tbg: '#282828', tfg: '#d4be98',
  ac: '#d8a657', ac2: '#e78a4e',
  ok: '#a9b665', wr: '#e78a4e', dg: '#ea6962', dgS: '#ae5858',
  ch: ['#7daea3', '#a9b665', '#d8a657', '#d3869b'],
};
T['gruvbox-material-light'] = {
  f: '岩焙', en: 'Gruvbox Material', m: '亮', kind: 'community', group: 'terminal',
  star: 2649, lic: 'MIT', repo: 'sainnhe/gruvbox-material',
  desc: '岩焙的官方亮档：米黄纸底 + 深暖棕文字，与暗档同一套暖色语言。',
  src: '官方 medium light palette：bg0 #fbf1c7 / bg1 #f4e8be / bg3 #eee0b7 / fg0 #654735',
  root: '#fbf1c7', panel: '#f4e8be', palt: '#eee0b7', elev: '#e5d5ad',
  hov: '#eee0b7', act: '#e5d5ad', tab: '#f4e8be',
  line: '#ddccab', lstr: '#c8b89a',
  t1: '#654735', t2: '#6B5646', t3: '#4f3829',
  tbg: '#fbf1c7', tfg: '#654735',
  ac: '#b47109', ac2: '#c35e0a',
  ok: '#6c782e', wr: '#c35e0a', dg: '#c14a4a', dgS: '#ae5858',
  ch: ['#45707a', '#6c782e', '#b47109', '#945e80'],
};

const fs = require('fs');
const path = require('path');
fs.writeFileSync(path.join(__dirname, '_extra-themes.json'),
  JSON.stringify({
    _meta: {
      generated: '2026-10-07',
      purpose: '主题重构新增 11 套（删 8 增 11 → 36 预设）',
      note: '槽位①=官方键直取 ②=面板阶在官方 bg 阶内组装（各套 src 字段注明）；ANSI 由 gen_ansi_group.js 统一管理',
    },
    ...T,
  }, null, 1), 'utf8');
console.log('11 套新主题 → scripts/_extra-themes.json');
for (const k of Object.keys(T)) console.log(' ·', k, T[k].f + '·' + T[k].m);
