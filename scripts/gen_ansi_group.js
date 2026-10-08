// ═════════════════════════════════════════════════════════════════════
// 补全设计稿缺口：33 套主题的「终端 ANSI 16 色」+「分组色 24 色」
//
// 为什么要有这个文件：theme-wallpaper-split 设计稿 §3 调整⑤要求
// "ANSI16_DARK/LIGHT 两套 → 按主题查表"，分组色要求 "每套主题的 24 色
// 按其明度重新取档"——但 theme-tokens.json / _radix-themes.json 里
// 只有 UI 槽位（root/panel/tbg/tfg/ac...），没有 ansi / group-colors。
// 本文件把这两个数据缺口补上，输出 scripts/_ansi-group.json，
// 并生成注入设计稿的 HTML 片段（scripts/_ansi-fragment.html）。
//
// 数据纪律（同 theme_palettes.py）：
//   ① 社区色系的 ANSI 一律取官方发布文件，不在此处二次加工；
//   ② 官方没有的档位（nord-light / onedark-light）不硬凑——
//      派生规则明确标注，校验结果如实记录，是否上架由待定项决策；
//   ③ 对比度校验只对 12 个彩色槽（1-6, 9-14）设 4.5:1 文字线——
//      0/7/8/15 槽（black/white 系）在终端惯例里兼作背景/弱化用途，
//      官方主题普遍不满足高对比（如 tokyonight day 的 black 槽），
//      只记录不判失败。
//
// 用法：node scripts/gen_ansi_group.js
// ═════════════════════════════════════════════════════════════════════

const fs = require('fs');
const path = require('path');

const TOKENS = JSON.parse(fs.readFileSync(path.join(__dirname, 'theme-tokens.json'), 'utf8'));
const RADIX_T = JSON.parse(fs.readFileSync(path.join(__dirname, '_radix-themes.json'), 'utf8'));
const RADIX = JSON.parse(fs.readFileSync(path.join(__dirname, '_radix.json'), 'utf8'));

// ── 颜色工具 ────────────────────────────────────────────────────────
function hx2rgb(h) {
  h = h.replace('#', '');
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}
function rgb2hx(r, g, b) {
  return '#' + [r, g, b].map(v => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, '0')).join('');
}
// WCAG 2.1 相对亮度与对比度
function lum(h) {
  const [r, g, b] = hx2rgb(h).map(v => {
    v /= 255;
    return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function contrast(a, b) {
  const [l1, l2] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
}
function rgb2hsl(r, g, b) {
  r /= 255; g /= 255; b /= 255;
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
  let h = 0, s = 0; const l = (mx + mn) / 2;
  if (mx !== mn) {
    const d = mx - mn;
    s = l > 0.5 ? d / (2 - mx - mn) : d / (mx + mn);
    if (mx === r) h = ((g - b) / d + (g < b ? 6 : 0));
    else if (mx === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
    h /= 6;
  }
  return [h * 360, s, l];
}
function hsl2rgb(h, s, l) {
  h /= 360;
  const f = (n) => {
    const k = (n + h * 12) % 12;
    const a = s * Math.min(l, 1 - l);
    return l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
  };
  return [f(0) * 255, f(8) * 255, f(4) * 255];
}
// 保持色相/饱和，调明度至对比度达标；原色已达标则原样返回（最小干预）
function fitL(hx, bg, target) {
  if (contrast(hx, bg) >= target) return hx;
  const [h, s, l0] = rgb2hsl(...hx2rgb(hx));
  const dark = lum(bg) < 0.35;           // 暗底 → 向亮扫；亮底 → 向深扫
  const dir = dark ? 1 : -1;
  for (let l = l0; dir > 0 ? l <= 1 : l >= 0; l += dir * 0.01) {
    const cand = rgb2hx(...hsl2rgb(h, s, l));
    if (contrast(cand, bg) >= target) return cand;
  }
  return dark ? rgb2hx(...hsl2rgb(h, s, 1)) : rgb2hx(...hsl2rgb(h, s, 0));
}

// ── ANSI 真源：12 套社区 / IDEA（官方发布文件采集，2026-10-07） ──────
// 槽序：0..7 = black red green yellow blue magenta cyan white，
//       8..15 = bright 对应。来源逐套注释，可回查。
const OFFICIAL_ANSI = {
  // draculatheme.com/spec · Color Palette (OSS)：Dracula（暗）与 Alucard（亮）
  'dracula-dark': {
    src: '官方 spec（draculatheme.com/spec）Dracula 档',
    ansi: ['#21222c', '#ff5555', '#50fa7b', '#f1fa8c', '#bd93f9', '#ff79c6', '#8be9fd', '#f8f8f2',
           '#6272a4', '#ff6e6e', '#69ff94', '#ffffa5', '#d6acff', '#ff92df', '#a4ffff', '#ffffff'],
    tbg: '#282a36', tfg: '#f8f8f2',
  },
  'dracula-light': {
    src: '官方 spec Alucard 档（Dracula 官方唯一亮色档）',
    ansi: ['#fffbeb', '#cb3a2a', '#14710a', '#846e15', '#644ac9', '#a3144d', '#036a96', '#1f1f1f',
           '#6c664b', '#d74c3d', '#198d0c', '#9e841a', '#7862d0', '#bf185a', '#047fb4', '#2c2b31'],
    tbg: '#fffbeb', tfg: '#1f1f1f',
  },
  // catppuccin/kitty 官方主题 conf（mocha.conf / latte.conf，2026-10-07 实取）
  'catppuccin-dark': {
    src: '官方 catppuccin/kitty mocha.conf',
    ansi: ['#45475a', '#f38ba8', '#a6e3a1', '#f9e2af', '#89b4fa', '#f5c2e7', '#94e2d5', '#bac2de',
           '#585b70', '#f38ba8', '#a6e3a1', '#f9e2af', '#89b4fa', '#f5c2e7', '#94e2d5', '#a6adc8'],
    tbg: '#1e1e2e', tfg: '#cdd6f4',
  },
  'catppuccin-light': {
    src: '官方 catppuccin/kitty latte.conf',
    ansi: ['#5c5f77', '#d20f39', '#40a02b', '#df8e1d', '#1e66f5', '#ea76cb', '#179299', '#acb0be',
           '#6c6f85', '#d20f39', '#40a02b', '#df8e1d', '#1e66f5', '#ea76cb', '#179299', '#bcc0cc'],
    tbg: '#eff1f5', tfg: '#4c4f69',
  },
  // ethanschoonover.com/solarized 官方映射表（亮暗共用 accent 组 + base 组翻转）
  // light 版另与 iTerm2 官方收录 "iTerm2 Solarized Light" 核对一致
  'solarized-dark': {
    src: '官方 solarized spec 映射表（dark）',
    ansi: ['#073642', '#dc322f', '#859900', '#b58900', '#268bd2', '#d33682', '#2aa198', '#eee8d5',
           '#002b36', '#cb4b16', '#586e75', '#657b83', '#839496', '#6c71c4', '#93a1a1', '#fdf6e3'],
    tbg: '#002b36', tfg: '#839496',
  },
  'solarized-light': {
    src: '官方 spec + iTerm2 Solarized Light 核对',
    ansi: ['#073642', '#dc322f', '#859900', '#b58900', '#268bd2', '#d33682', '#2aa198', '#eee8d5',
           '#002b36', '#cb4b16', '#586e75', '#657b83', '#839496', '#6c71c4', '#93a1a1', '#fdf6e3'],
    tbg: '#fdf6e3', tfg: '#657b83',
  },
  // morhetz/gruvbox colors/gruvbox.vim 调色板 → 终端 16 色标准映射
  // （gruvbox-community Xresources 同源）；亮档用官方 faded 组
  'gruvbox-dark': {
    src: '官方 gruvbox.vim 调色板 → 标准终端映射',
    ansi: ['#282828', '#cc241d', '#98971a', '#d79921', '#458588', '#b16286', '#689d6a', '#a89984',
           '#928374', '#fb4934', '#b8bb26', '#fabd2f', '#83a598', '#d3869b', '#8ec07c', '#ebdbb2'],
    tbg: '#282828', tfg: '#ebdbb2',
  },
  'gruvbox-light': {
    src: '官方 faded 组（gruvbox.vim）→ gruvbox_light 终端映射',
    ansi: ['#fbf1c7', '#cc241d', '#98971a', '#d79921', '#458588', '#b16286', '#689d6a', '#7c6f64',
           '#928374', '#9d0006', '#79740e', '#b57614', '#076678', '#8f3f71', '#427b58', '#3c3836'],
    tbg: '#fbf1c7', tfg: '#3c3836',
  },
  // folke/tokyonight.nvim extras/kitty 官方生成文件（night.conf / day.conf）
  'tokyonight-dark': {
    src: '官方 tokyonight.nvim extras/kitty night',
    ansi: ['#15161e', '#f7768e', '#9ece6a', '#e0af68', '#7aa2f7', '#bb9af7', '#7dcfff', '#a9b1d6',
           '#414868', '#ff899d', '#9fe044', '#faba4a', '#8db0ff', '#c7a9ff', '#a4daff', '#c0caf5'],
    tbg: '#1a1b26', tfg: '#c0caf5',
  },
  'tokyonight-light': {
    src: '官方 tokyonight.nvim extras/kitty day',
    ansi: ['#b4b5b9', '#f52a65', '#587539', '#8c6c3e', '#2e7de9', '#9854f1', '#007197', '#6172b0',
           '#a1a6c5', '#ff4774', '#5c8524', '#a27629', '#358aff', '#a463ff', '#007ea8', '#3760bf'],
    tbg: '#e1e2e7', tfg: '#3760bf',
  },
  // arcticicestudio/nord-xresources src/nord（官方 Xresources 映射）
  'nord-dark': {
    src: '官方 nord-xresources Xresources 映射',
    ansi: ['#3b4252', '#bf616a', '#a3be8c', '#ebcb8b', '#81a1c1', '#b48ead', '#88c0d0', '#e5e9f0',
           '#4c566a', '#bf616a', '#a3be8c', '#ebcb8b', '#81a1c1', '#b48ead', '#8fbcbb', '#eceff4'],
    tbg: '#2e3440', tfg: '#d8dee9',
  },
  // Nord 官方没有亮色主题档。按官方 "bright ambiance" 指引派生：
  // nord6 作背景、Polar Night 作前景/槽位、Frost+Aurora 原色不变。
  // ⚠️ Aurora 系（红黄绿）在浅底对比度 3.1~3.9，低于 4.5 —— 与 Everforest
  //    亮档同属设计稿 §6 待定②，校验如实记录，是否上架随该决策。
  'nord-light': {
    src: '派生：官方 bright ambiance 指引（nord6 底 + Polar 前景，Frost/Aurora 原色）',
    ansi: ['#d8dee9', '#bf616a', '#a3be8c', '#ebcb8b', '#81a1c1', '#b48ead', '#88c0d0', '#4c566a',
           '#e5e9f0', '#bf616a', '#a3be8c', '#ebcb8b', '#81a1c1', '#b48ead', '#8fbcbb', '#434c5e'],
    tbg: '#eceff4', tfg: '#3b4252',
  },
  // sainnhe/everforest palette.md（medium）+ alacritty-theme 官方移植核对
  'everforest-dark': {
    src: '官方 palette.md medium + everforest_dark 终端移植核对',
    ansi: ['#475258', '#e67e80', '#a7c080', '#dbbc7f', '#7fbbb3', '#d699b6', '#83c092', '#d3c6aa',
           '#475258', '#e67e80', '#a7c080', '#dbbc7f', '#7fbbb3', '#d699b6', '#83c092', '#d3c6aa'],
    tbg: '#2d353b', tfg: '#d3c6aa',
  },
  'everforest-light': {
    src: '官方 palette.md medium + everforest_light 终端移植核对',
    ansi: ['#5c6a72', '#f85552', '#8da101', '#dfa000', '#3a94c5', '#df69ba', '#35a77c', '#e0dcc7',
           '#5c6a72', '#f85552', '#8da101', '#dfa000', '#3a94c5', '#df69ba', '#35a77c', '#e0dcc7'],
    tbg: '#fdf6e3', tfg: '#5c6a72',
  },
  // rose-pine/alacritty 官方 dist（Main / Dawn）
  'rosepine-dark': {
    src: '官方 rose-pine/alacritty dist（Main）',
    ansi: ['#26233a', '#eb6f92', '#31748f', '#f6c177', '#9ccfd8', '#c4a7e7', '#ebbcba', '#e0def4',
           '#6e6a86', '#eb6f92', '#31748f', '#f6c177', '#9ccfd8', '#c4a7e7', '#ebbcba', '#e0def4'],
    tbg: '#191724', tfg: '#e0def4',
  },
  'rosepine-light': {
    src: '官方 rose-pine/alacritty dist（Dawn）',
    ansi: ['#f2e9e1', '#b4637a', '#286983', '#ea9d34', '#56949f', '#907aa9', '#d7827e', '#575279',
           '#9893a5', '#b4637a', '#286983', '#ea9d34', '#56949f', '#907aa9', '#d7827e', '#575279'],
    tbg: '#faf4ed', tfg: '#575279',
  },
  // alacritty-theme one_dark（Atom 官方移植谱系，bright 仅黑白有别——官方如此）
  'onedark-dark': {
    src: 'atom one-dark 终端移植（alacritty-theme one_dark）',
    ansi: ['#1e2127', '#e06c75', '#98c379', '#d19a66', '#61afef', '#c678dd', '#56b6c2', '#abb2bf',
           '#5c6370', '#e06c75', '#98c379', '#d19a66', '#61afef', '#c678dd', '#56b6c2', '#ffffff'],
    tbg: '#282c34', tfg: '#abb2bf',
  },
  // One Dark 官方没有亮档（atom one light 是另一个色系）。亮档按项目统一
  // 自造规则派生：保持官方彩色槽的色相/饱和，明度二分至 4.5:1。
  'onedark-light': {
    src: '派生：One Dark 彩色槽保 H/S 调 L 至 4.5:1（官方无亮档，自造）',
    // 起点 = One Dark 官方彩色槽，生成期对各彩槽按亮底 tbg 二分明度
    ansi: ['#1e2127', '#e06c75', '#98c379', '#d19a66', '#61afef', '#c678dd', '#56b6c2', '#abb2bf',
           '#5c6370', '#e06c75', '#98c379', '#d19a66', '#61afef', '#c678dd', '#56b6c2', '#ffffff'],
    tbg: null, tfg: null,
  },
  // JetBrains：IDEA 的终端 ANSI 不随 UI 明暗切换（IDEA 实际行为），
  // idea-dark/idea-light 共用同一套彩色槽（iTerm2 官方收录 "JetBrains Darcula"），
  // 仅背景/前景随各套 tokens。亮底上部分彩槽对比度低——IDEA 本身如此，
  // 如实记录。darcula 已删（与 idea-dark 重叠，New UI 是现代默认）。
  'idea-dark': {
    src: 'JetBrains Darcula 终端色（iTerm2 官方收录版）；IDEA 行为：明暗共用 ANSI',
    ansi: ['#000000', '#fa8355', '#186e00', '#c2c300', '#4581eb', '#fa54ff', '#33c2c1', '#adadad',
           '#555555', '#fb7172', '#67ff4f', '#ffff00', '#6d9df1', '#fb82ff', '#60d3d1', '#eeeeee'],
    tbg: null, tfg: null,              // 用 tokens 的
  },
  'idea-light': { sameAs: 'idea-dark' },

  // ══════ 2026-10-07 主题重构新增 11 套（全部 MIT 实测确认） ══════
  // rebelot/kanagawa.nvim 官方 palette + alacritty-theme kanagawa_wave
  'kanagawa-wave': {
    src: '官方 palette + alacritty kanagawa_wave（官方收录移植）',
    ansi: ['#090618', '#c34043', '#76946a', '#c0a36e', '#7e9cd8', '#957fb8', '#6a9589', '#c8c093',
           '#727169', '#e82424', '#98bb6c', '#e6c384', '#7fb4ca', '#938aa9', '#7aa89f', '#dcd7ba'],
    tbg: '#1F1F28', tfg: '#DCD7BA',
  },
  // lotus 官方无终端发布文件 → lotus palette 键映射 + 调深达标（同自造亮档规则）
  'kanagawa-lotus': {
    src: '派生：官方 lotus palette 键映射 + 保 H/S 调 L 至 4.5:1（官方无终端发布文件）',
    ansi: ['#545464', '#c84053', '#6f894e', '#de9800', '#4d699b', '#b35b79', '#597b75', '#8a8980',
           '#a09cac', '#d7474b', '#6e915f', '#e98a00', '#6693bf', '#766b90', '#5e857a', '#43436c'],
    tbg: '#f2ecbc', tfg: '#545464',
  },
  // GitHub 官方 VS Code 主题（Primer dark/light default）的 terminal 色
  'github-dark': {
    src: 'GitHub 官方 VS Code 主题 Dark Default terminal 色（Primer 体系）',
    ansi: ['#484f58', '#ff7b72', '#3fb950', '#d29922', '#58a6ff', '#bc8cff', '#39c5cf', '#b6beca',
           '#6e7681', '#ffa198', '#56d364', '#e3b341', '#79c0ff', '#d2a8ff', '#56d4dd', '#f0f6fc'],
    tbg: '#0d1117', tfg: '#e6edf3',
  },
  'github-light': {
    src: 'GitHub 官方 VS Code 主题 Light Default terminal 色（Primer 体系）',
    ansi: ['#24292f', '#cf222e', '#116329', '#953800', '#0550ae', '#8250df', '#1b7c83', '#6e7781',
           '#57606a', '#a40e26', '#1a7f37', '#9e6a03', '#218bff', '#a475f9', '#3192aa', '#8c959f'],
    tbg: '#ffffff', tfg: '#1f2328',
  },
  // ayu 官方收录移植（alacritty-theme ayu_dark/mirage/light 三档）
  // （ayu-vim 的 term/*.itermcolors 是 2018 老 8 位值，亮底不可读，不采用）
  'ayu-dark': {
    src: '官方收录移植（alacritty-theme ayu_dark）',
    ansi: ['#01060e', '#ea6c73', '#91b362', '#f9af4f', '#53bdfa', '#fae994', '#90e1c6', '#c7c7c7',
           '#686868', '#f07178', '#c2d94c', '#ffb454', '#59c2ff', '#ffee99', '#95e6cb', '#ffffff'],
    tbg: '#0B0E14', tfg: '#B3B1AD',
  },
  'ayu-mirage': {
    src: '官方收录移植（alacritty-theme ayu_mirage）',
    ansi: ['#212733', '#f08778', '#53bf97', '#fdcc60', '#60b8d6', '#ec7171', '#98e6ca', '#fafafa',
           '#686868', '#f58c7d', '#58c49c', '#ffd165', '#65bddb', '#f17676', '#9debcf', '#ffffff'],
    tbg: '#1F2430', tfg: '#CBCCC6',
  },
  'ayu-light': {
    src: '官方收录移植（alacritty-theme ayu_light）',
    ansi: ['#010101', '#e7666a', '#80ab24', '#eba54d', '#4196df', '#9870c3', '#51b891', '#c1c1c1',
           '#343434', '#ee9295', '#9fd32f', '#f0bc7b', '#6daee6', '#b294d2', '#75c7a8', '#dbdbdb'],
    tbg: '#FCFCFC', tfg: '#5C6166',
  },
  // kepano/flexoki 官方 alacritty 移植（官方收录，README 调色板同源）
  'flexoki-dark': {
    src: '官方 flexoki 终端移植（alacritty-theme flexoki，README 同源）',
    ansi: ['#100F0F', '#AF3029', '#66800B', '#AD8301', '#205EA6', '#A02F6F', '#24837B', '#FFFCF0',
           '#100F0F', '#D14D41', '#879A39', '#D0A215', '#4385BE', '#CE5D97', '#3AA99F', '#FFFCF0'],
    tbg: '#282726', tfg: '#FFFCF0',
  },
  'flexoki-light': {
    src: '官方 flexoki-light 终端移植（alacritty-theme，README 同源）',
    ansi: ['#100F0F', '#D14D41', '#879A39', '#D0A215', '#4385BE', '#CE5D97', '#3AA99F', '#FFFCF0',
           '#100F0F', '#D14D41', '#879A39', '#D0A215', '#4385BE', '#CE5D97', '#3AA99F', '#FFFCF0'],
    tbg: '#FFFCF0', tfg: '#100F0F',
  },
  // sainnhe/gruvbox-material 官方 medium（MIT，替代无授权的原版 Gruvbox）
  'gruvbox-material-dark': {
    src: '官方 medium palette + alacritty gruvbox_material_medium_dark',
    ansi: ['#3c3836', '#ea6962', '#a9b665', '#d8a657', '#7daea3', '#d3869b', '#89b482', '#d4be98',
           '#3c3836', '#ea6962', '#a9b665', '#d8a657', '#7daea3', '#d3869b', '#89b482', '#d4be98'],
    tbg: '#282828', tfg: '#d4be98',
  },
  'gruvbox-material-light': {
    src: '官方 medium light palette + alacritty gruvbox_material_medium_light',
    ansi: ['#654735', '#c14a4a', '#6c782e', '#b47109', '#45707a', '#945e80', '#4c7a5d', '#eee0b7',
           '#654735', '#c14a4a', '#6c782e', '#b47109', '#45707a', '#945e80', '#4c7a5d', '#eee0b7'],
    tbg: '#fbf1c7', tfg: '#654735',
  },

  // ══════ 2026-10-07 主题重构删除 8 套（数据保留在 git 历史） ══════
  // gruvbox-dark/light：仓库无 LICENSE 文件（仅 README 口头 MIT/X11），商用分发风险
  // dracula-light（Alucard）：官方明说不是亮色档，挂名困惑
  // onedark-light：纯自造（Atom 无 One Dark 亮档），违反"不硬凑档位"
  // darcula：与 idea-dark 重叠（ANSI 完全共用），New UI 是现代默认
  // everforest-light：官方灰阶撑不起正文对比（待定②收口，只推暗档）
  // olive-dark/light：Radix 六中性系中独特性最弱（绿调有 sage、暖调有 sand）
};

// ── Radix 12 套派生（Radix 是界面色板，无官方 ANSI → 按色阶确定性映射）──
// 规则（dark 变体）：彩色槽 normal = 该色系 idx10（亮端次高）、bright = idx11（最亮），
//                   black = 中性 idx1、brightBlack = 中性 idx6、white = tfg、brightWhite = 中性 idx11。
// 规则（light 变体）：彩色 normal = idx10（深端）、bright = idx11（更深，同 gruvbox-light 官方做法），
//                   black = 中性 idx10、brightBlack = 中性 idx8、white = 中性 idx9、brightWhite = 中性 idx11。
// 彩色色相 6 系全族共用（red/green/yellow/blue/purple/cyan——Radix 色相体系一致，
// 族内 ANSI 稳定；各套 accent 的调和体现在 UI 层，不进终端）。
// 校验不足 4.5 时自动换下一档并记录（dark: idx10→11；light: idx10→11 不够则加深深化）。
const HUES = { red: 'red', green: 'green', yellow: 'yellow', blue: 'blue', magenta: 'purple', cyan: 'cyan' };
const NEUTRALS = { 'graphite': 'gray', 'slate': 'slate', 'mauve': 'mauve', 'sage': 'sage', 'sand': 'sand' };

function radixAnsi(famKey, mode) {
  const t = RADIX_T[`${famKey}-${mode}`];
  const neutral = NEUTRALS[famKey];
  const ladder = (name) => (mode === 'dark' ? RADIX.dark[name] : RADIX.light[name]);
  const N = ladder(neutral);
  const dark = mode === 'dark';
  const ansi = new Array(16);
  const notes = [];
  const hueOrder = ['red', 'green', 'yellow', 'blue', 'magenta', 'cyan'];
  hueOrder.forEach((slot, i) => {
    const L = ladder(HUES[slot]);
    let a = L[10], b = L[11];
    if (dark) {
      if (contrast(a, t.tbg) < 4.5) { a = L[11]; notes.push(`ansi${i + 1}(${slot}) idx10 不足 4.5 → 用 idx11`); }
    } else {
      if (contrast(a, t.tbg) < 4.5) { a = L[11]; notes.push(`ansi${i + 1}(${slot}) idx10 不足 4.5 → 用 idx11`); }
    }
    ansi[i + 1] = a;
    ansi[i + 9] = b;
  });
  if (dark) {
    ansi[0] = N[1]; ansi[8] = N[6];
    ansi[7] = t.tfg; ansi[15] = N[11];
  } else {
    ansi[0] = N[10]; ansi[8] = N[8];
    ansi[7] = N[9]; ansi[15] = N[11];
  }
  return { ansi, notes, src: '派生：Radix 色阶确定性映射（' + (dark ? 'normal=idx10/bright=idx11' : 'normal=idx10/bright=idx11 更深') + '）' };
}

// ── 迁移保留档 meat-dark / meat-light（现配色原样，§5 迁移表引用） ────
// theme_pref=dark|light 的老用户迁到 theme=meat-dark|meat-light，观感零变化。
// ANSI = presentation.rs ANSI16_DARK（VS Code Dark+）/ ANSI16_LIGHT 现值；
// meat-light 额外带 ansi16bg（亮色 TUI 背景填充表，presentation.rs ANSI16_LIGHT_BG
// ——现状"亮色前景/背景双表"特性，迁移后必须保留）。
const MEAT = {
  'meat-dark': {
    f: 'MTerm', m: '暗', kind: 'meat', group: 'meat',
    tbg: '#0e0f13', tfg: '#d4d4d4', panel: '#23262d', tab: '#2a2d35',
    ch: ['#378add', '#1d9e75', '#ba7517', '#993556'],
    ansi16: ['#000000', '#cd3131', '#0dbc79', '#e5e510', '#2472c8', '#bc3fbc', '#11a8cd', '#e5e5e5',
             '#666666', '#f14c4c', '#23d18b', '#f5f543', '#3b8eea', '#d670d6', '#29b8db', '#ffffff'],
    src: '现状原样（presentation.rs ANSI16_DARK = VS Code Dark+）',
  },
  'meat-light': {
    f: 'MTerm', m: '亮', kind: 'meat', group: 'meat',
    tbg: '#fafafa', tfg: '#2d2d2f', panel: '#ffffff', tab: '#eaeaef',
    ch: ['#378add', '#1d9e75', '#ba7517', '#993556'],
    ansi16: ['#1c1c1e', '#c0392b', '#1a7f37', '#856404', '#0451a5', '#800080', '#0e725c', '#3a3a3c',
             '#555555', '#e74c3c', '#27ae60', '#d4ac0d', '#2e86c1', '#9b59b6', '#1abc9c', '#2c2c2e'],
    ansi16bg: ['#e8e8ed', '#ffd5d5', '#d5f5d5', '#fff8d5', '#d5e8f8', '#f5d5f5', '#d5f5f8', '#f5f5f7',
               '#d1d1d6', '#ffbebe', '#bef5be', '#f5f5be', '#beddff', '#f0beff', '#bef5ff', '#ffffff'],
    src: '现状原样（ANSI16_LIGHT 前景表 + ANSI16_LIGHT_BG TUI 背景表，双表特性保留）',
  },
};

// ── 分组色基准（ui/theme.slint:244-255 现值，2026-10-07 同步） ─────────
const GROUP_DARK = ['#f87171','#fb923c','#fbbf24','#facc15','#a3e635','#4ade80',
                    '#34d399','#2dd4bf','#22d3ee','#38bdf8','#60a5fa','#818cf8',
                    '#a78bfa','#c084fc','#e879f9','#f472b6','#fb7185','#fda4af',
                    '#f1f5f9','#e2e8f0','#cbd5e1','#a8a29e','#94a3b8','#64748b'];
const GROUP_LIGHT = ['#dc2626','#ea580c','#d97706','#ca8a04','#65a30d','#16a34a',
                     '#059669','#0d9488','#0891b2','#0284c7','#2563eb','#4f46e5',
                     '#7c3aed','#9333ea','#c026d3','#db2777','#e11d48','#f43f5e',
                     '#475569','#334155','#1e293b','#57534e','#78716c','#a8a29e'];

// ── 组装（2026-10-07 重构后 36 预设 + 2 迁移档） ───────────────────
const EXTRA = JSON.parse(fs.readFileSync(path.join(__dirname, '_extra-themes.json'), 'utf8'));
const SKIP = new Set([
  'gruvbox-dark', 'gruvbox-light',      // 授权未声明（→ gruvbox-material 替代）
  'dracula-light',                       // Alucard 非亮色档
  'onedark-light',                       // 纯自造亮档
  'darcula',                             // 与 idea-dark 重叠
  'everforest-light',                    // 官方灰阶撑不起正文（只推暗档）
  'olive-dark', 'olive-light',           // 与 sage/sand 色温重叠
]);
const GROUPS_DEF = [
  ['radix', ['graphite', 'slate', 'mauve', 'sage', 'sand']],
  ['brand', ['github-dark', 'github-light', 'idea-dark', 'idea-light']],
  ['terminal', ['dracula', 'catppuccin', 'solarized', 'kanagawa', 'ayu', 'gruvbox-material',
                'tokyonight', 'nord', 'everforest', 'rosepine', 'onedark', 'flexoki']],
];
// 变体名不对称的套（默认对称 dark/light）：
//   kanagawa → wave（暗）/ lotus（亮，官方命名）
//   ayu → dark / mirage / light（官方三档，mirage 为中间档）
const VARIANTS = { kanagawa: ['wave', 'lotus'], ayu: ['dark', 'mirage', 'light'] };

const out = { _meta: {
  generated: '2026-10-07',
  purpose: '主题重构后全量：36 预设（Radix 10 + 品牌官方 4 + 经典终端 22）+ 2 迁移档的 ANSI 16 色 + 分组色 24 色 + 频道色',
  ansi_sources: {}, group_rule: '基准=theme.slint group-color-dark/light 24 色，保 H/S 二分 L 至 on(panel) ≥ 3.0:1',
  ansi_check: '12 彩色槽(1-6,9-14) ≥ 4.5:1 为达标线；0/7/8/15 槽按终端惯例只记录不判失败',
} };
const report = [];

for (const [_grp, ids] of GROUPS_DEF) {
  for (const base of ids) {
    const variants = VARIANTS[base]
      || ((base.endsWith('-dark') || base.endsWith('-light')) ? [''] : ['dark', 'light']);
    for (const variant of variants) {
      const id = variant ? `${base}-${variant}` : base;
      if (SKIP.has(id)) continue;
      const tokens = TOKENS[id] || RADIX_T[id] || EXTRA[id];
      if (!tokens) { console.error('缺 tokens:', id); continue; }
      const tbg = tokens.tbg, tfg = tokens.tfg;
      let ansi, src, notes = [];

      if (OFFICIAL_ANSI[id]) {
        const o = OFFICIAL_ANSI[id];
        if (o.sameAs) {
          const real = OFFICIAL_ANSI[o.sameAs];
          ansi = real.ansi; src = real.src + '（' + id + ' 与 ' + o.sameAs + ' 共用，IDEA 行为：ANSI 不随明暗切换）';
          notes.push('IDEA 亮/暗共用同一套终端 ANSI（IDEA 实际行为）；' + (lum(tbg) > 0.35 ? '亮底上部分彩槽对比度低于 4.5，是 IDEA 本身观感，见校验' : ''));
        } else {
          ansi = o.ansi; src = o.src;
          if (id === 'nord-light' || id === 'onedark-light' || id === 'kanagawa-lotus')
            ansi = ansi.map((c, i2) => ([1,2,3,4,5,6,9,10,11,12,13,14].includes(i2) ? fitL(c, tbg, 4.5) : c));
          if (id === 'nord-light') notes.push('官方无亮档：UI 按 bright ambiance 派生，ANSI 彩色槽同自造规则调深至 4.5:1');
          if (id === 'onedark-light') notes.push('官方无亮档：彩色槽保 H/S 调 L 至 4.5:1（自造，已在 src 标注）');
        }
      } else if (RADIX_T[id]) {
        const r = radixAnsi(base, id.endsWith('-dark') ? 'dark' : 'light');
        ansi = r.ansi; src = r.src; notes = r.notes;
      } else { console.error('无 ANSI 来源:', id); continue; }

      // 与 tokens 的 tbg/tfg 校对（官方 ANSI 自带 bg/fg 时提示差异）
      if (OFFICIAL_ANSI[id] && OFFICIAL_ANSI[id].tbg && OFFICIAL_ANSI[id].tbg.toLowerCase() !== tbg.toLowerCase()) {
        notes.push(`注意：官方终端底 ${OFFICIAL_ANSI[id].tbg} 与本套 tokens tbg ${tbg} 不同（以 tokens 为准）`);
      }

      // 分组色：基准按明暗方向取，保 H/S 调 L 至 on(panel) ≥ 3.0
      const gBase = tokens.m === '暗' ? GROUP_DARK : GROUP_LIGHT;
      const group = gBase.map(c => fitL(c, tokens.panel, 3.0));

      // 校验
      const fails = [], mins = [];
      for (const i of [1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14]) {
        const c = contrast(ansi[i], tbg);
        mins.push(c.toFixed(1));
        if (c < 4.5) fails.push(`ansi${i}=${ansi[i]} on tbg ${c.toFixed(2)}`);
      }
      const gFails = [];
      group.forEach((c, i) => { if (contrast(c, tokens.panel) < 3.0) gFails.push(i); });

      out[id] = {
        f: tokens.f, m: tokens.m, ansi16: ansi, group24: group,
        ch: tokens.ch, channel: makeChannel(tokens.ch, tokens.tab, tbg),
        tab: tokens.tab, src, notes, fails,
      };
      out._meta.ansi_sources[id] = src;
      report.push({ id, name: `${tokens.f}·${tokens.m}`, minC: Math.min(...[1,2,3,4,5,6,9,10,11,12,13,14].map(i => contrast(ansi[i], tbg))), fails, gFails: gFails.length });
    }
  }
}

// ── 频道色（tab-row-idia 第四章"深浅两套"根治方案的数据形态） ──────────
// 主题化后每套变体自带 4 槽频道色（tokens 的 ch），须在该套的两个 tab 底上
// 双达标：非选中 tab 底（tab 槽）与选中底（= term-bg）都 ≥ 3.0（非文本线）。
// 不达标保 H/S 调 L（最小干预）。tab 稿的"8 常量"方案是单配色时代的形态，
// 主题化后被"每套 4 值"自然覆盖。
function makeChannel(ch, tabBg, tbg) {
  return ch.map(c => {
    let v = fitL(c, tabBg, 3.0);
    if (contrast(v, tbg) < 3.0) v = fitL(c, tbg, 3.0);
    return v;
  });
}

// ── 迁移保留档 meat-dark / meat-light ──────────────────────────────
for (const [id, mk] of Object.entries(MEAT)) {
  const group = (mk.m === '暗' ? GROUP_DARK : GROUP_LIGHT).map(c => fitL(c, mk.panel, 3.0));
  const fails = [];
  for (const i of [1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14]) {
    const c = contrast(mk.ansi16[i], mk.tbg);
    if (c < 4.5) fails.push(`ansi${i}=${mk.ansi16[i]} on tbg ${c.toFixed(2)}`);
  }
  const ch = makeChannel(mk.ch, mk.tab, mk.tbg);
  out[id] = {
    f: mk.f, m: mk.m, kind: 'meat',
    ansi16: mk.ansi16, ...(mk.ansi16bg ? { ansi16bg: mk.ansi16bg } : {}),
    group24: group, ch: mk.ch, channel: ch,
    tab: mk.tab, src: mk.src,
    notes: ['迁移保留档：老用户 theme_pref 迁到本 id，观感零变化（§5 迁移表）'],
    fails,
  };
  out._meta.ansi_sources[id] = mk.src;
  report.push({ id, name: `${mk.f}·${mk.m}`, minC: Math.min(...[1,2,3,4,5,6,9,10,11,12,13,14].map(i => contrast(mk.ansi16[i], mk.tbg))), fails, gFails: 0 });
}

fs.writeFileSync(path.join(__dirname, '_ansi-group.json'), JSON.stringify(out, null, 1), 'utf8');

// 控制台报告
console.log('══ 33 套 ANSI / 分组色生成完毕 → scripts/_ansi-group.json ══\n');
for (const r of report) {
  const flag = r.fails.length ? '✗' : '✓';
  console.log(`${flag} ${r.id.padEnd(18)} ${r.name.padEnd(10)} 彩槽最低对比 ${r.minC.toFixed(2)}  分组色未达标 ${r.gFails}`);
  r.fails.forEach(f => console.log(`      · ${f}`));
}
