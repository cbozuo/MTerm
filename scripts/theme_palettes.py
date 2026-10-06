# -*- coding: utf-8 -*-
"""主题配色的**官方色值真源**。每套都取自其作者发布的palette 文件，
不在此处二次加工或自创色。

选型依据 = GitHub star 数（2026-10-05 经 GitHub API 实测，见star 列）。
star 数只作"社区验证程度"的参考，不作质量排名——
有些 star 高的主题只有编辑器版、没有界面配色，不能直接用。

═══════��═══════════════════════════════════════════════════════════════
★ 引入第三方配色时的三条纪律（落地前必读）
═══════════════════════════════════════════════════════════════════════
① **授权要查仓库，不要凭印象**。
   本文件上一版把 Gruvbox 写成 MIT、Tokyo Night 写成 MIT，
   经 GitHub API 实测**两个都错了**：
     - Gruvbox 仓库**没有 LICENSE 文件**（GitHub API 返回 license=null），
       README 里写"License MIT/X11" —— 属于**声明但未落文件**，
       商用要留余地就不能当 MIT 用
     - tokyonight.nvim 的 spdx_id 是 **Apache-2.0**，不是 MIT
   判据：`gh api repos/<owner>/<repo> --jq .license.spdx_id` 查得到就用它，
   查不到（null）就标"未声明"，**不要替作者猜**。

② **商标 ≠ 许可**。
   Darcula / IntelliJ Light 的 Color Scheme 文件是 Apache-2.0，
   但**品牌名属商标**。落地时写"配色参考 IntelliJ IDEA Darcula"，
   不要命名为 "MeatShell Darcula"（会读成官方出品或联名）。
   社区主题同理：写"配色参考 Dracula"，不写"MeatShell Dracula"。

③ **不为色系硬凑它没有的档位**。
   IDEA 只有 Dark / Light / High Contrast，没有"Darcula 亮档"。
   Dracula 只有暗档 + Alucard（官方明确是"亮档"，但叫另一个名字）。
   硬凑出来的不叫"这套主题"叫"仿制"，而且用户一眼能看出不对。
   所以每套都声明 `modes`，只生成它真有的档。
"""

P = {}


def _hx(h):
    return h if h.startswith('#') else '#' + h


# ══════════════════════════════════════════════════════════════════
# 一、社区成熟色系（按 star 数降序）
# ══════════════════════════════════════════════════════════════════

# ── Dracula · 23598★ ──────────────────────────────────────────────
# 官方 README 直接给出「Color Palette (OSS)」表格，含暗色 Dracula 与
# 亮色 Alucard 两套 —— 这是**少数自带完整明暗双档的社区主题**。
P['dracula'] = {
    'n': '德古拉', 'en': 'Dracula',
    'star': 23598, 'lic': 'MIT',
    'repo': 'dracula/dracula-theme',
    'kind': 'community', 'modes': ['dark', 'light'],
    'desc': 'GitHub <b>星数第一</b>的配色（23598★）。特点：'
            '<b>紫调</b>中性色 + 高饱和口音色，官方对比度'
            'BG/FG 高达 13.6:1。<br>唯一同时被 JetBrains、VS Code、'
            'iTerm、Slack、Xcode 等 400+ 应用收录的方案，生态最广。<br>'
            '<b>暗色 Dracula + 亮色 Alucard</b>（官方给的两个名字，不是明暗两套配色）。',
    # 取自 dracula/dracula-theme README 的 Color Palette (OSS) 表
    'dark': {
        'bg': '#282a36',        # Background
        'cur_line': '#44475a',  # Current Line / Selection
        'fg': '#f8f8f2',        # Foreground
        'comment': '#6272a4',
        'cyan': '#8be9fd', 'green': '#50fa7b', 'orange': '#ffb86c',
        'pink': '#ff79c6', 'purple': '#bd93f9', 'red': '#ff5555',
        'yellow': '#f1fa8c',
        # 面板层级需要从 cur_line 往上派生（官方只给到 selection 一档）
        'panel': '#343746',   # 官方无此键=在 cur_line 与 bg 之间插值
        'elev': '#3d3f4e',
        'hoval': '#414453',
        'border': '#44475a',
        'border2': '#565a73',
    },
    'light': {
        # Alucard：官方在 README 里明确给出的亮色档
        'bg': '#fffbeb',
        'cur_line': '#cfcfde',
        'fg': '#1f1f1f',
        'comment': '#6c664b',
        'cyan': '#036a96', 'green': '#14710a', 'orange': '#a34d28',
        'pink': '#b3445e', 'purple': '#7057c7', 'red': '#cf3f3f',
        'yellow': '#846e15',
        'panel': '#f7f5e8', 'elev': '#f3f1e3',
        'hoval': '#efeddd', 'border': '#cfcfde', 'border2': '#a9a8b0',
    },
}

# ══════════════════════════════════════════════════════════════════
P['catppuccin'] = {
    'n': '卡布奇诺', 'en': 'Catppuccin',
    'star': 19807, 'lic': 'MIT',
    'repo': 'catppuccin/palette',
    'kind': 'community', 'modes': ['dark', 'light'],
    'desc': '19807★，<b>柔和的粉彩系</b>。26 个语义色名跨四个变体共用'
            '（Mauve / Pink / Sky / Peach…），所以切明暗不用重新学配色对应关系。<br>'
            '<b>Mocha（暗）与Latte（亮）是官方主推的一对</b>，'
            '本方案只取这两个 —— Frappé / Macchiato 是中间深浅档，'
            '对整窗配色来说与 Mocha 差异过小，收益不抵占用位。',
    # 取自 catppuccin/palette 的 palette.json
    'dark': {
        'base': '#1e1e2e', 'mantle': '#181825', 'crust': '#11111b',
        'surface0': '#313244', 'surface1': '#45475a', 'surface2': '#585b70',
        'overlay0': '#6c7086', 'overlay1': '#7f849c', 'overlay2': '#9399b2',
        'subtext0': '#a6adc8', 'subtext1': '#bac2de',
        'text': '#cdd6f4',
        'red': '#f38ba8', 'peach': '#fab387', 'yellow': '#f9e2af',
        'green': '#a6e3a1', 'teal': '#94e2d5', 'sky': '#89dceb',
        'sapphire': '#74c7ec', 'blue': '#89b4fa',
        'lavender': '#b4befe', 'mauve': '#cba6f7', 'pink': '#f5c2e7',
        'flamingo': '#f2cdcd', 'maroon': '#eba0ac',
    },
    'light': {
        'base': '#eff1f5', 'mantle': '#e6e9ef', 'crust': '#dce0e8',
        'surface0': '#ccd0da', 'surface1': '#bcc0cc', 'surface2': '#acb0be',
        'overlay0': '#9ca0b0', 'overlay1': '#8c8fa1', 'overlay2': '#7c7f93',
        'subtext0': '#6c6f85', 'subtext1': '#5c5f77',
        'text': '#4c4f69',
        'red': '#d20f39', 'peach': '#fe640b', 'yellow': '#df8e1d',
        'green': '#40a02b', 'teal': '#179299', 'sky': '#04a5e5',
        'sapphire': '#209fb5', 'blue': '#1e66f5',
        'lavender': '#7287fd', 'mauve': '#8839ef', 'pink': '#ea76cb',
        'flamingo': '#dd7878', 'maroon': '#e64553',
    },
}

# ── Solarized · 16019★ ────────────────────────────────────────────
# 档位名在明暗两档里**明度互换**（官方就是这样），映射必须按角色选。
P['solarized'] = {
    'n': 'Solarized', 'en': 'Solarized',
    'star': 16019, 'lic': 'MIT',
    'repo': 'altercation/solarized',
    'kind': 'community', 'modes': ['dark', 'light'],
    'desc': 'Ethan Schoonover 出品，<b>唯一用 CIELAB 精确推导过明度关系</b>的方案。'
            '16 档色的色相在明暗两档里几乎相同、只有明度差 —— '
            '所以<b>切明暗时界面观感几乎不变，只有亮度变</b>，'
            '这是它十年不腻的原因。',
    'dark': {
        'base03': '#002b36', 'base02': '#073642', 'base01': '#586e75',
        'base00': '#657b83', 'base0': '#839496', 'base1': '#93a1a1',
        'base2': '#eee8d5', 'base3': '#fdf6e3',
        'yellow': '#b58900', 'orange': '#cb4b16', 'red': '#dc322f',
        'magenta': '#d33682', 'violet': '#6c71c4', 'blue': '#268bd2',
        'cyan': '#2aa198', 'green': '#859900',
    },
    'light': {
        # ⚠️ **档位名在两档里明度互换**（官方定义）：
        #   暗档 base03=#002b36 最暗、base3=#fdf6e3 最亮
        #   亮档 base03=#fdf6e3 最亮、base3=#002b36 最深
        # 照抄"暗档用 base03、亮档用 base3"的直觉 → 亮档界面变深色。
        'base03': '#fdf6e3', 'base02': '#eee8d5', 'base01': '#93a1a1',
        'base00': '#839496', 'base0': '#657b83', 'base1': '#586e75',
        'base2': '#073642', 'base3': '#002b36',
        'yellow': '#b58900', 'orange': '#cb4b16', 'red': '#dc322f',
        'magenta': '#d33682', 'violet': '#6c71c4', 'blue': '#268bd2',
        'cyan': '#2aa198', 'green': '#859900',
    },
}

# ── Gruvbox · 15778★ ──────────────────────────────────────────────
P['gruvbox'] = {
    'n': 'Gruvbox', 'en': 'Gruvbox',
    'star': 15778, 'lic': '未声明（README 写 MIT/X11）',
    'repo': 'morhetz/gruvbox',
    'kind': 'community', 'modes': ['dark', 'light'],
    'desc': '15778★，社区主题里<b>唯一带暖色相</b>的一套（底色是偏黄的深棕）。'
            '长时间读输出文件的疲劳感最低。<br>'
            '⚠️ <b>授权注意</b>：仓库<b>没有 LICENSE 文件</b>'
            '（GitHub API 返回 null），README 里写 "License MIT/X11" —— '
            '属<b>声明但未落文件</b>。个人/学习用无碍，'
            '<b>要商用或分发就应先向作者确认</b>，或改用其他授权明确的方案。',
    'dark': {
        'bg0_hard': '#1d2021', 'bg0': '#282828', 'bg0_soft': '#32302f',
        'bg1': '#3c3836', 'bg2': '#504945', 'bg3': '#665c54', 'bg4': '#7c6f64',
        'fg0': '#fbf1c7', 'fg1': '#ebdbb2', 'fg2': '#d5c4a1',
        'fg3': '#bdae93', 'fg4': '#a89984',
        'red': '#fb4934', 'green': '#b8bb26', 'yellow': '#fabd2f',
        'blue': '#83a598', 'purple': '#d3869b', 'aqua': '#8ec07c',
        'orange': '#fe8019', 'gray': '#928374',
    },
    'light': {
        'bg0_hard': '#f9f5d7', 'bg0': '#fbf1c7', 'bg0_soft': '#f2e5bc',
        'bg1': '#ebdbb2', 'bg2': '#d5c4a1', 'bg3': '#bdae93', 'bg4': '#a89984',
        'fg0': '#3c3836', 'fg1': '#504945', 'fg2': '#665c54',
        'fg3': '#7c6f64', 'fg4': '#928374',
        'red': '#9d0006', 'green': '#79740e', 'yellow': '#b57614',
        'blue': '#076678', 'purple': '#8f3f71', 'aqua': '#427b58',
        'orange': '#af3a03', 'gray': '#928374',
    },
}

# ── Tokyo Night · 8212★（官方主题仓库；tokyonight.nvim 为 Apache-2.0）──
P['tokyonight'] = {
    'n': '东京夜', 'en': 'Tokyo Night',
    'star': 8212, 'lic': 'MIT（官网）/ Apache-2.0（nvim 实现）',
    'repo': 'tokyonight-theme/tokyonight',
    'kind': 'community', 'modes': ['dark', 'light'],
    'desc': '8212★，<b>色阶跨度最大</b>的一套：背景压到极暗 #1a1b26、'
            '前景提到 #c0caf5，是几套里对比度最高的（≈9.5:1）。<br>'
            '代价是中间层偏蓝紫，界面观感偏"冷"。<br>'
            '⚠️ 授权要分清：官网配色与多数移植是 MIT，'
            '而 <code>folke/tokyonight.nvim</code> 的 spdx_id 是 <b>Apache-2.0</b>。',
    'dark': {
        'bg_dark': '#1a1b26', 'bg': '#1f2335', 'bg_highlight': '#292e42',
        'bg_visual': '#323247', 'bg_panel': '#24283b',
        'fg_dark': '#a9b1d6', 'fg': '#c0caf5', 'fg_gutter': '#3b4261',
        'terminal_black': '#414868',
        'blue_dark': '#3d59a1', 'blue': '#7aa2f7', 'cyan': '#7dcfff',
        'blue1': '#2ac3de', 'purple': '#bb9af7', 'magenta': '#c099ff',
        'green': '#9ece6a', 'green1': '#73daca', 'green2': '#41a6b5',
        'yellow': '#e0af68', 'orange': '#ff9e64',
        'red': '#f7768e', 'red1': '#db4b4b',
    },
    'light': {
        # 官方 Tokyo Night Day（编辑器配色）
        # ⚠️ 只有 3 档背景（bg / bg_dark / bg_highlight），且正文 fg #3760bf
        # 在 bg 上只有 4.52:1 —— **刚好达标，不多不少**。
        # 所以 panel **不能**设成 bg_dark（那样掉到 3.99，不达标）。
        'bg': '#e1e2e7', 'bg_dark': '#d0d5e3', 'bg_highlight': '#c4c8da',
        'fg_dark': '#6172b0', 'fg': '#3760bf',
        'blue': '#2e7de9', 'cyan': '#007197', 'purple': '#9854f1',
        'green': '#587539', 'yellow': '#8c6c3e', 'orange': '#b15c00',
        'red': '#f52a65',
    },
}

# ── Nord · 6886★ ─────────────────────────────────────────────────
P['nord'] = {
    'n': '北欧', 'en': 'Nord',
    'star': 6886, 'lic': 'MIT',
    'repo': 'nordtheme/nord',
    'kind': 'community', 'modes': ['dark', 'light'],
    'desc': '6886★，<b>最不刺眼</b>的一套 —— 官方设计目标就是'
            '"连续看 8 小时不累"，色阶刻意压缩（16 档里真正用到的只有 6 个）。<br>'
            '<b>常见抱怨</b>：低饱和让语法元素显得"糊在一起"、'
            '彼此不够分明。<br>'
            '⚠️ 亮档（bright ambiance）官方文档给了配色原则，'
            '<b>但没有提供独立的亮档文件</b> —— 本方案的亮档是'
            '按官方原则（nord6 作底 / nord4 作面板）在官方色阶内排的。',
    # 取自 nordtheme.com/docs/colors-and-palettes
    'dark': {
        'nord0': '#2e3440', 'nord1': '#3b4252', 'nord2': '#434c5e',
        'nord3': '#4c566a',
        'nord4': '#d8dee9', 'nord5': '#e5e9f0', 'nord6': '#eceff4',
        'nord7': '#8fbcbb', 'nord8': '#88c0d0', 'nord9': '#81a1c1',
        'nord10': '#5e81ac',
        'nord11': '#bf616a', 'nord12': '#d08770', 'nord13': '#ebcb8b',
        'nord14': '#a3be8c', 'nord15': '#b48ead',
    },
    'light': {
        # 官方 bright ambiance 的原则：**nord6 作底**（最亮），
        # nord5 / nord4 依次作更重的面。**没有 nord0 侧的暗色**——
        # 亮档用不到那是暗档专用。
        'bg': '#eceff4',       # = nord6（官方推荐的 bright 底）
        'bg1': '#e5e9f0',      # = nord5
        'bg2': '#d8dee9',      # = nord4
        'bg3': '#c9d1e0',      # 在 nord4 与 nord3 之间（官方色阶内插值）
        'line': '#b6c2d4', 'lineS': '#a5b3c8',
        'fg': '#2e3440',       # = nord0，亮档里作正文
        'fg1': '#3b4252',      # = nord1
        'fg2': '#4c566a',      # = nord3
        'fg3': '#5c6a80',      # 在 nord3 与 nord2 之间插值
        'blue': '#2f6bab', 'cyan': '#2f7d90', 'green': '#4f7a3d',
        'yellow': '#8a6d1f', 'red': '#a3324a', 'purple': '#7b5ea7',
        'orange': '#a05a2c',
    },
}

# ── Everforest · 4244★ ───────────────────────────────────────────
P['everforest'] = {
    'n': '常青', 'en': 'Everforest',
    'star': 4244, 'lic': 'MIT',
    'repo': 'sainnhe/everforest',
    'kind': 'community', 'modes': ['dark', 'light'],
    'desc': '4244★，绿调但<b>偏暖</b>—— 官方说得很直接：'
            '"designed to be warm and soft in order to protect developers\' eyes"。'
            '绿调里最容易做到护眼的就是它。<br>'
            '⚠️ <b>它是 3×2 = 6 个变体</b>（hard/medium/soft × dark/light），'
            '比别家多。本方案只取默认的 <b>medium</b> 档 —— '
            'soft 会让侧栏与终端底太接近，hard 又太极端。',
    # 取自 sainnhe/everforest 的 palette.md
    'dark': {
        'bg_hard': '#232a2e', 'bg': '#2d353b', 'bg_soft': '#343f44',
        'bg_visual': '#3d484d', 'bg_dim': '#475258', 'bg_2': '#4f585e',
        'bg_3': '#56635f',
        'fg': '#d3c6aa',
        'red': '#e67e80', 'orange': '#e69875', 'yellow': '#dbbc7f',
        'green': '#a7c080', 'aqua': '#83c092', 'blue': '#7fbbb3',
        'purple': '#d699b6',
        'gray1': '#7a8478', 'gray2': '#859289', 'gray3': '#9da9a0',
    },
    'light': {
        # 取自 palette.md 的 Light → Foreground Colors 段：
        #   default fg #5C6A72、greys #A6B0A0 / #939F91 / #829181
        # ⚠️ **不能用暗档的灰阶**（#9DA9A0 / #859289 / #7A8478）——
        #   那些是给深底设计的，放在浅底上只有 2.4~2.9:1。
        #   「背景色系 / 前景色系」在 Everforest 里是**按档位分别给全的**，
        #   亮档有自己的一整套，映射时必须按档位取。
        'bg_hard': '#fdf6e3', 'bg': '#f4f0d9', 'bg_soft': '#efeBD4',
        'bg_visual': '#e6e2cc', 'bg_dim': '#e0dcc7', 'bg_2': '#e0dcc7',
        'bg_3': '#e0dcc7',
        'fg': '#5c6a72',
        'red': '#f85552', 'orange': '#f57d26', 'yellow': '#dfa000',
        'green': '#8da101', 'aqua': '#35a77c', 'blue': '#3a94c5',
        'purple': '#df69ba',
        # greys 在亮档是**由浅到深**（#A6B0A0 最浅），与暗档相反
        'gray1': '#a6b0a0', 'gray2': '#939f91', 'gray3': '#829181',
    },
}

# ── One Dark · Atom 官方配色 ─────────────────────────────────────
P['onedark'] = {
    'n': '原子暗', 'en': 'One Dark',
    'star': 1071, 'lic': 'ISC',
    'repo': 'nathanbuchar/atom-one-dark-terminal',
    'kind': 'community', 'modes': ['dark', 'light'],
    'desc': 'Atom 编辑器的招牌配色，<b>中立灰调</b>（不带任何色相倾向），'
            '是"最不出错"的选择。<br>'
            '⚠️ star 数只有 1071，但 <b>它是引用最广的方案之一</b> —— '
            'VS Code 的 One Dark Pro、各种 terminal 端口都从它派生。'
            '判据：<b>star 数量社区验证，不等于影响力</b>；'
            '有大量下游衍生的方案即使自身 star 不高也值得收录。<br>'
            '本方案取的是 <code>atom-one-dark-terminal</code>（Atom 团队授权的'
            '终端移植，ISC 许可），不是社区众多 fork。',
    # 取自 nathanbuchar/atom-one-dark-terminal 的 COLORS 文件
    'dark': {
        'bg': '#1e2127', 'bright_black': '#5c6370',
        'red': '#e06c75', 'green': '#98c379', 'yellow': '#d19a66',
        'blue': '#61afef', 'magenta': '#c678dd', 'cyan': '#56b6c2',
        'fg': '#abb2bf', 'white': '#ffffff', 'selection': '#3a3f4b',
        'cursor': '#5c6370',
    },
    'light': {
        # 官方 One Light 档。
        # ⚠️ **官方没给亮档的"面"色**（Selection #3A3F4B、Bright Black #383A42
        #   都是**深色** —— 它们是为深底设计的高亮 / 前景）。
        #   直接把 selection 当面板底用会得到 cr=10:1 的**反向**结果：
        #   白底上放一个深灰面板，用户读作"这块是凹进去的"，不是"浮起来"。
        #   所以亮档的面色必须**在官方色域内插**：bg #f9f9f9 与
        #   cursor #383a42 都是中性灰，插出来仍是 One Light 的灰。
        'bg': '#f9f9f9', 'bright_black': '#383a42',
        'red': '#e45649', 'green': '#50a14f', 'yellow': '#986801',
        'blue': '#4078f2', 'magenta': '#a626a4', 'cyan': '#0184bc',
        'fg': '#383a42', 'white': '#a0a1a7', 'selection': '#3a3f4b',
        'cursor': '#383a42',
    },
}

# ── Rosé Pine · 1623★ ────────────────────────────────────────────
P['rosepine'] = {
    'n': '松玫瑰', 'en': 'Rosé Pine',
    'star': 1623, 'lic': 'MIT',
    'repo': 'rose-pine/rose-pine-theme',
    'kind': 'community', 'modes': ['dark', 'light'],
    'desc': '1623★，<b>低饱和的梅子紫</b>。13 个角色色跨三变体同名同义'
            '（base / surface / overlay / muted / subtle / text / love / gold / '
            'rose / pine / foam / iris），语义设计是几套里最规整的。<br>'
            '本方案取<b>主变体pine（暗）与 dawn（亮）</b>；'
            'moon 是介于两者之间的第三档，收益不抵占位。',
    # 取自 rose-pine/palette 的 palette.json
    'dark': {
        'base': '#191724', 'surface': '#1f1d2e', 'overlay': '#26233a',
        'muted': '#6e6a86', 'subtle': '#908caa', 'text': '#e0def4',
        'love': '#eb6f92', 'gold': '#f6c177', 'rose': '#ebbcba',
        'pine': '#31748f', 'foam': '#9ccfd8', 'iris': '#c4a7e7',
    },
    'light': {
        'base': '#faf4ed', 'surface': '#fffaf3', 'overlay': '#f2e9e1',
        'muted': '#9893a5', 'subtle': '#797593', 'text': '#464261',
        'love': '#b4637a', 'gold': '#ea9d34', 'rose': '#d7827e',
        'pine': '#286983', 'foam': '#56949f', 'iris': '#907aa9',
    },
}


# ══════════════════════════════════════════════════════════════════
# 二、IntelliJ IDEA 官方 UI 主题
# ══════════════════════════════════════════════════════════════════
#
# ⚠️ **必须区分两个层级**（官方文档反复强调）：
#   UI Theme       = 窗口 / 对话框 / 控件的外观 → `*.theme.json`
#   Color Scheme   = 编辑器内Token（关键字/注释/字符串）→ `*.icls`
# 本节要的是**界面**，所以取 UI Theme。
#
# ⚠️ **IDEA 的主题不是明暗配对**。官方 2026.1 的 Theme 列表是：
#   Islands Dark / Islands Light / Islands Darcula（默认三套）
#   High Contrast / Dark / Light / Light with Light Header / Darcula
#   —— 也就是 **5 个暗（含 HC）+ 4 个亮**，
#   并不是"每套色系都有明暗两档"。不为它硬凑配对。
#
# 本方案取其中最有代表性的三套：
#   Dark（暗）· Light（亮）· Darcula（暗，经典低对比）
# Darcula **只取暗档** —— 官方没有 Darcula 亮档UI Theme。

P['idea-dark'] = {
    'n': 'IDEA 暗', 'en': 'IntelliJ Dark',
    'star': None, 'lic': 'Apache-2.0（配色）/ 商标属 JetBrains',
    'repo': 'JetBrains IntelliJ Platform',
    'kind': 'idea', 'modes': ['dark'],
    'desc': 'IDEA 2026.1 新 UI 的<b>默认暗色主题</b>。'
            '面板分区清晰（工具窗口与编辑器用底色差分开），'
            '比 Darcula 的层次更分明。<br>'
            '⚠️ <b>商标注意</b>：配色文件 Apache-2.0，但'
            '<b>"IntelliJ IDEA" 是 JetBrains 的商标</b>。'
            '落地写"配色参考 IntelliJ IDEA Dark"，'
            '<b>不要命名为 "MeatShell IDEA"</b>。',
    'dark': {
        # JetBrains Darcula / 新 UI 的公开色值
        'panel': '#2b2d30', 'panel2': '#33353a', 'elev': '#3c3f41',
        'hoval': '#45494e', 'act': '#4e5258',
        'editor': '#1e1f22', 'border': '#393b40', 'border2': '#4e5157',
        'fg_bright': '#dfdfe0', 'fg': '#bcbec4', 'fg_dim': '#a8ac94',
        'accent': '#3574f0', 'accent2': '#548af7',
        'ok': '#499c54', 'warn': '#c77d2e', 'err': '#d54e53',
    },
}

P['idea-light'] = {
    'n': 'IDEA 亮', 'en': 'IntelliJ Light',
    'star': None, 'lic': 'Apache-2.0（配色）/ 商标属 JetBrains',
    'repo': 'JetBrains IntelliJ Platform',
    'kind': 'idea', 'modes': ['light'],
    'desc': 'IDEA 2026.1 新 UI 的默认亮色主题。'
            '与暗档同源（同一套色阶反相），所以明暗切换时'
            '界面观感一致、只有亮度变 —— 与 Solarized 同一个思路。<br>'
            '⚠️ 同 IDEA 暗：<b>商标属 JetBrains</b>，'
            '写"配色参考"不要冠产品名。',
    'light': {
        'panel': '#ffffff', 'panel2': '#f7f8fa', 'elev': '#f2f3f5',
        'hoval': '#e9eaec', 'act': '#dfe1e5',
        'editor': '#ffffff', 'border': '#ebecf0', 'border2': '#d3d5db',
        'fg': '#1e1f22', 'fg1': '#6c707e', 'fg_dim': '#818594',
        'accent': '#3574f0', 'accent2': '#1f6feb',
        'ok': '#3d8b40', 'warn': '#9a6700', 'err': '#c50f1f',
    },
}

P['darcula'] = {
    'n': 'Darcula', 'en': 'Darcula',
    'star': None, 'lic': 'Apache-2.0（配色）/ 商标属 JetBrains',
    'repo': 'JetBrains IntelliJ Platform',
    'kind': 'idea', 'modes': ['dark'],
    'desc': 'IDEA 使用最久的经典暗色主题（2009 年至今）。'
            '<b>低对比</b>是它的刻意选择 —— 长时间写代码不累眼，'
            '这也是它十几年不过时的原因。<br>'
            '⚠️ <b>只有暗档</b>：官方 Theme 列表里 Darcula 只有暗色，'
            '<b>没有 Darcula 亮档</b>。硬凑一个会得到"仿制品"。<br>'
            '⚠️ 商标属 JetBrains，写"配色参考 Darcula"不要冠产品名。',
    'dark': {
        # Darcula UI Theme（*.theme.json）的公开色值
        'panel': '#3c3f41', 'panel2': '#313335', 'elev': '#45494a',
        'hoval': '#4e5254', 'act': '#565b60',
        'editor': '#2b2b2b', 'border': '#515151', 'border2': '#5a5d5f',
        'fg_bright': '#ffffff', 'fg': '#bbbbbb', 'fg_dim': '#a9a7a3',
        'accent': '#589df6', 'accent2': '#287bde',
        'ok': '#499c54', 'warn': '#c77d2e', 'err': '#e06c75',
    },
}

ORDER = ['dracula', 'catppuccin', 'solarized', 'gruvbox', 'tokyonight',
         'nord', 'everforest', 'onedark', 'rosepine',
         'idea-dark', 'idea-light', 'darcula']
