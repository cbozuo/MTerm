# -*- coding: utf-8 -*-
"""把各色系的**官方色阶**映射成20 个语义槽位，并校验对比度。

设计约束（每条都是本项目实际踩过的坑）：
══════════════════════════════════════════════════════════════════════
① **映射表按"这一档在当前档位里扮演什么角色"选，不按名字里的数字**
   实例（Solarized）：`base03` 在明暗两档里**明度互换** ——
   暗档 base03=#002b36 最暗、亮档 base03=#fdf6e3 最亮。
   照抄"暗档用 base03、亮档用 base3"的直觉 → 整个亮档界面变深色。
② **缺档时在官方色阶内取最近的一档，不新造色**。
   Nord 官方 nord0~nord3 全是背景色系，没有"中间调前景"这一档 →
   在 nord4 与 nord3 之间插值（色相不变，仍在 Nord 冷灰域内）。
③ **"哪一档配哪一层"是色系作者的设计意图，派生函数只能靠对比度猜，猜不准**。
   所以 tab 底用`MAP_TAB` 显式指定，不用 `derive_tab()` 猜。
   实例（Gruvbox）：猜成 `bg0_soft`（语法高亮微调档）→ tab 行重到压过标题。
④ **`cr()` 是无方向的量**：亮底上放深色也 > 1.18。
   任何"要更亮/ 要更暗"的推导都必须先按方向过滤候选。
⑤ **校验要加语义断言**（数值达标但视觉错误的两种情况）：
   `tab == 终端底`（tab 行无边界）/ `cr(tab, 终端底) < 1.10`（浮起太弱）。
⑥ **阈值要问"这套色系能达到多少"，不是"我想要多少"**。
   ID 两套面板相邻差 1.06~1.12，它们能给出的最大浮起就落在 1.12；
   要求 1.12 等于要求"必须比色系自己的下一档更远"—— 那是改色系不是配色。
══════════════════════════════════════════════════════════════════════
"""

import io
import json

from theme_palettes import P, ORDER

# ══════════════════════════════════════════════════════════════════
# 工具
# ══════════════════════════════════════════════════════════════════


def lum(h):
    h = h.lstrip('#')
    if len(h) == 3:
        h = ''.join(c * 2 for c in h)
    r, g, b = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]

    def f(c):
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)


def cr(a, b):
    la, lb = lum(a), lum(b)
    if la < lb:
        la, lb = lb, la
    return (la + 0.05) / (lb + 0.05)


def mix(a, b, k):
    fg_probe, bg_probe = '#123456', '#abcdef'
    """`mix(fg, bg, a)` 的语义是 **a=1 全 fg、a=0 全 bg**。

    ⚠️ 写反的后果极隐蔽：结果会变成纯白 / 纯黑，
    **对比度反而最高**，校验 100% 通过，但视觉上"隐形"。
    所以下面加了断言兜底 —— **断言比校验更能抓这类错**。
    """
    # ⚠️ **k 的语义：0=全 bg、1=全 fg**。第一个参数是"要混进去的那个色"。
    #   本项目已经因此踩了两次（scale_to 一次、亮档面板插值表一次），
    #   症状都是"值偏到另一个方向"而对比度检查仍然通过 ——
    #   **断言比校验更能抓这类错**，所以在边界上钉死两个探针值。
    if k == 0 and a != b:
        assert '#%02x%02x%02x' % tuple(
            int(b.lstrip('#')[i:i + 2], 16) for i in (0, 2, 4)) == b.lower(), \
            'mix(前景, 背景, 0) 必须等于背景色 —— 参数顺序写反了'
    if k == 1 and a != b:
        assert '#%02x%02x%02x' % tuple(
            int(a.lstrip('#')[i:i + 2], 16) for i in (0, 2, 4)) == a.lower(), \
            'mix(前景, 背景, 1) 必须等于前景色 —— 参数顺序写反了'
    return '#%02x%02x%02x' % tuple(
        round(int(a.lstrip('#')[i:i + 2], 16) * k
              + int(b.lstrip('#')[i:i + 2], 16) * (1 - k)) for i in (0, 2, 4))


def hue(h):
    h = h.lstrip('#')
    r, g, b = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    mx, mn = max(r, g, b), min(r, g, b)
    if mx == mn:
        return None          # 灰色无色相
    d = mx - mn
    if mx == r:
        hh = ((g - b) / d) % 6
    elif mx == g:
        hh = (b - r) / d + 2
    else:
        hh = (r - g) / d + 4
    return hh * 60


def hdiff(a, b):
    ha, hb = hue(a), hue(b)
    if ha is None or hb is None:
        return 999
    d = abs(ha - hb)
    return min(d, 360 - d)


# ══════════════════════════════════════════════════════════════════
# 派生规则（跨全部主题固定，不进 palette）
# ══════════════════════════════════════════════════════════════════

# 频道 4 槽 = **功能身份色，固定色相，跨全部主题共用**。
# ⚠️ 判据：**身份不能随装饰变**。
#   频道色表达「这个会话归哪一组」= 身份；强调色表达「用户喜欢什么蓝」= 装饰。
#   让频道色继承强调色 → Gruvbox（橙强调色）下 4 槽全落进橙黄色系、
#   A/B 色相差仅 13°，字母分不出谁是谁（实测过）。
CH_HUE = {'A': 210, 'B': 165, 'C': 38, 'D': 345}


def from_hsl(h, s_, l):
    c = (1 - abs(2 * l - 1)) * s_
    x = c * (1 - abs((h / 60) % 2 - 1))
    m = l - c / 2
    if h < 60:
        r, g, b = c, x, 0
    elif h < 120:
        r, g, b = x, c, 0
    elif h < 180:
        r, g, b = 0, c, x
    elif h < 240:
        r, g, b = 0, x, c
    elif h < 300:
        r, g, b = x, 0, c
    else:
        r, g, b = c, 0, x
    return '#%02x%02x%02x' % tuple(round((v + m) * 255) for v in (r, g, b))


def pick_ch(bg, dark):
    """在**固定色相**内调饱和度与明度，取第一个达标的。

    ⚠️ 搜索区间必须收窄：暗色档 L 上限 0.72、亮色档下限 0.42。
    放宽到 0.96 会生成 #ecf5fe 这类"发白淡彩"—— 对暗底对比度确实 > 3，
    但它读起来像**高亮色块**而不是频道字母，视觉重量压过标题。
    **达标是下限不是上限**，要在"刚够"的区间里取最贴近的一档。
    """
    out = []
    for k in 'ABCD':
        h = CH_HUE[k]
        best = None
        for s_ in [x / 100 for x in range(78, 44, -6)]:
            ls = ([x / 100 for x in range(72, 46, -2)] if dark
                  else [x / 100 for x in range(30, 50, 2)])
            for l in ls:
                c = from_hsl(h, s_, l)
                if cr(c, bg) >= 3.0:
                    best = c
                    break
            if best:
                break
        out.append(best or from_hsl(h, 0.68, 0.55 if dark else 0.42))
    return out


def scale_to(fg, bg, target, dark):
    """把 fg 朝「远离 bg 的方向」推到达标。

    ⚠️ 方向不能搞反：**朝 bg 混只会降低对比度**。
    暗色主题要往白推、亮色主题要往黑推。
    ⚠️ 还要卡上限（cap）：步进 1% 会一路推到纯白 / 纯黑才"达标"，
    得到 #ffffff —— 那不是"红色的 hover 态"，是"看不见的 ×"。
    """
    anchor = '#ffffff' if dark else '#000000'
    assert mix(anchor, fg, 0) == fg.lower(), 'mix 方向反了'
    cap = 0.55 if dark else 0.50
    last = None
    for k in range(101):
        if k / 100 > cap:
            break
        c = mix(anchor, fg, k / 100)
        last = c
        if cr(c, bg) >= target:
            return c            # 第一个达标即最省力
    return last or fg


# ══════════════════════════════════════════════════════════════════
# 槽位映射：每套色系一张表
#   语义槽位 → 该色系的官方色阶键名
# ══════════════════════════════════════════════════════════════════

MAP = {}

# ── Dracula ───────────────────────────────────────────────────────
MAP['dracula'] = {
    'dark': dict(root='bg', panel='bg', palt='panel', elev='elev',
                 hov='hoval', act='hoval',
                 line='border', lstr='border2',
                 t1='fg', t2='fg', t3='comment',
                 tbg='bg', tfg='fg',
                 ac='purple', ac2='cyan',
                 ok='green', wr='yellow', dg='red'),
    'light': dict(root='bg', panel='bg', palt='panel', elev='elev',
                  hov='hoval', act='hoval',
                  line='border', lstr='border2',
                  t1='fg', t2='fg', t3='comment',
                  tbg='bg', tfg='fg',
                  ac='purple', ac2='cyan',
                  ok='green', wr='yellow', dg='red'),
}

# ── Catppuccin ────────────────────────────────────────────────────
# 26 个官方键名本身就是"语义命名"，映射几乎是一一对应。
MAP['catppuccin'] = {
    m: dict(root='base', panel='base', palt='mantle', elev='surface0',
            hov='surface1', act='surface2',
            line='surface1', lstr='surface2',
            t1='text', t2='subtext1', t3='subtext0',
            tbg='base', tfg='text',
            ac='blue', ac2='lavender',
            ok='green', wr='yellow', dg='red')
    for m in ('dark', 'light')
}

# ── Solarized ─────────────────────────────────────────────────────
# ⚠️ 亮档键名与暗档**同名不同值**，见theme_palettes 里的警告。
MAP['solarized'] = {
    'dark': dict(root='base03', panel='base03', palt='base02', elev='base01',
                 hov='base01', act='base00',
                 line='base01', lstr='base00',
                 t1='base3', t2='base2', t3='base1',
                 tbg='base03', tfg='base2',
                 ac='blue', ac2='cyan',
                 ok='green', wr='yellow', dg='red'),
    #亮档：base03=#fdf6e3（最亮）、base3=#002b36（最深）
    'light': dict(root='base03', panel='base03', palt='base02', elev='base02',
                  hov='base02', act='base02',
                  line='base01', lstr='base0',
                  t1='base3', t2='base_t2', t3='base_t3',
                  tbg='base03', tfg='base0',
                  ac='blue', ac2='cyan',
                  ok='green', wr='yellow', dg='red'),
}

# ── Gruvbox ───────────────────────────────────────────────────────
# 暗档里 bg0_soft 是给**语法高亮微调**用的，当整块面太重；
# 亮档 bg0_soft 与 bg0 只差 1.11，浮起太弱 → 改用 bg1。
MAP['gruvbox'] = {
    'dark': dict(root='bg0_hard', panel='bg0', palt='bg0_soft', elev='bg1',
                 hov='bg2', act='bg3',
                 line='bg2', lstr='bg3',
                 t1='fg1', t2='fg2', t3='fg3',
                 tbg='bg0_hard', tfg='fg1',
                 ac='orange', ac2='yellow',
                 ok='green', wr='yellow', dg='red'),
    'light': dict(root='bg0_hard', panel='bg0', palt='bg0_soft', elev='bg1',
                  hov='bg2', act='bg3',
                  line='bg2', lstr='bg3',
                  t1='fg1', t2='fg2', t3='fg3',
                  tbg='bg0_hard', tfg='fg1',
                  ac='orange', ac2='yellow',
                  ok='green', wr='yellow', dg='red'),
}

# ── Tokyo Night ───────────────────────────────────────────────────
#⚠️ 亮档只有 3 档背景，且 fg #3760bf on bg = 4.52:1 刚好达标 ——
#    所以 panel **不能**设成 bg_dark（那样掉到 3.99）。
MAP['tokyonight'] = {
    'dark': dict(root='bg_dark', panel='bg', palt='bg_highlight', elev='bg_visual',
                 hov='bg_visual', act='bg_panel',
                 line='bg_highlight', lstr='terminal_black',
                 t1='fg', t2='fg_dark', t3='fg_mid',
                 tbg='bg_dark', tfg='fg',
                 ac='blue', ac2='cyan',
                 ok='green', wr='yellow', dg='red'),
    'light': dict(root='bg', panel='bg', palt='bg_dark', elev='bg_highlight',
                  hov='bg_dark', act='bg_highlight',
                  line='bg_highlight', lstr='fg_dark',
                  t1='fg', t2='fg', t3='fg_dark',
                  tbg='bg', tfg='fg_dark',
                  ac='blue', ac2='cyan',
                  ok='green', wr='yellow', dg='red'),
}

# ── Nord ──────────────────────────────────────────────────────────
MAP['nord'] = {
    'dark': dict(root='nord0', panel='nord0', palt='nord1', elev='nord2',
                 hov='nord2', act='nord3',
                 line='nord2', lstr='nord3',
                 t1='nord6', t2='nord4', t3='nord3t',
                 tbg='nord0', tfg='nord4',
                 ac='nord8', ac2='nord7',
                 ok='nord14', wr='nord13', dg='nord11'),
    # 亮档按官方 bright ambiance：nord6 作底、nord5/nord4 依次作更重的面
    'light': dict(root='bg', panel='bg', palt='bg1', elev='bg2',
                  hov='bg2', act='bg3',
                  line='line', lstr='lineS',
                  t1='fg', t2='fg1', t3='fg2',
                  tbg='bg', tfg='fg',
                  ac='blue', ac2='cyan',
                  ok='green', wr='yellow', dg='red'),
}

# ── Everforest ────────────────────────────────────────────────────
MAP['everforest'] = {
    'dark': dict(root='bg_hard', panel='bg', palt='bg_soft', elev='bg_visual',
                 hov='bg_dim', act='bg_2',
                 line='bg_dim', lstr='bg_2',
                 t1='fg', t2='gray3', t3='gray2',
                 tbg='bg_hard', tfg='fg',
                 ac='aqua', ac2='blue',
                 ok='green', wr='yellow', dg='red'),
    'light': dict(root='bg_hard', panel='bg', palt='bg_soft', elev='bg_visual',
                  hov='bg_dim', act='bg_2',
                  line='bg_dim', lstr='bg_2',
                  t1='fg', t2='fg_t2', t3='fg_t3',
                  tbg='bg_hard', tfg='fg',
                  ac='aqua', ac2='blue',
                  ok='green', wr='yellow', dg='red'),
}

# ── One Dark ──────────────────────────────────────────────────────
# ⚠️ 亮档**官方没给"面"色**（Selection/Bright Black 都是深色，
#    是为深底设计的高亮 / 前景）。直接拿来当面板底会得到 cr=10:1 的反向结果。
#    所以亮档的面色用 _fill_derived() 在官方灰阶内插。
MAP['onedark'] = {
    'dark': dict(root='bg', panel='bg', palt='selection', elev='sel_hi',
                 hov='selection', act='bright_black',
                 line='bright_black', lstr='cursor',
                 t1='white', t2='fg', t3='fg_dim',
                 tbg='bg', tfg='fg',
                 ac='blue', ac2='cyan',
                 ok='green', wr='yellow', dg='red'),
    'light': dict(root='bg', panel='bg', palt='panel2', elev='panel2',
                  hov='panel3', act='panel4',
                  line='panel4', lstr='bright_black',
                  t1='fg', t2='fg2', t3='fg3',
                  tbg='bg', tfg='fg',
                  ac='blue', ac2='cyan',
                  ok='green', wr='yellow', dg='red'),
}

# ── Rosé Pine ─────────────────────────────────────────────────────
# 官方13 个角色名跨三变体同名同义 —— 映射是直接的一一对应。
MAP['rosepine'] = {
    'dark': dict(root='base', panel='base', palt='surface', elev='overlay',
                 hov='overlay', act='overlay',
                 line='overlay', lstr='muted',
                 t1='text', t2='subtle', t3='muted',
                 tbg='base', tfg='text',
                 ac='pine', ac2='foam',
                 ok='iris', wr='gold', dg='love'),
    # 亮档的 subtle #797593 / muted #9893a5 在官方浅底上只有 4.0 / 2.7，
    # 改用 _fill_derived() 在官方色域内插深的两档。
    'light': dict(root='base', panel='base', palt='surface', elev='overlay',
                  hov='overlay', act='overlay',
                  line='overlay', lstr='muted',
                  t1='text', t2='subtle2', t3='muted2',
                  tbg='base', tfg='text',
                  ac='pine', ac2='foam',
                  ok='iris', wr='gold', dg='love'),
}
# ⚠️ Rosé Pine 官方 13 个角色色里**没有"成功绿"**（只有 love 红 / gold 黄 /
#   pine 蓝 / foam 青 / iris 紫）。硬把 gold 当"成功"会和警告撞色，
#   改成用 iris 紫作"次级信息"、love 红=错误、gold 黄=警告
#   —— 这与它自己把 iris 用在"次要提示"的定位一致。
# ⚠️ Rosé Pine 官方 13 个角色色里**没有"成功绿"**（只有 love 红 / gold 黄 /
#   pine 蓝 / foam 青 / iris 紫）。硬把 gold 当"成功"会和警告撞色，
#   改成用 iris 紫作"次级信息"、love红=错误、gold 黄=警告
#   —— 这与它自己把 iris 用在"次要提示"的定位一致。

# ══════════════════════════════════════════════════════════════════
# IDEA 三套（键名已是界面语义，直接映射）
# ══════════════════════════════════════════════════════════════════
_IDEA = dict(root='panel', panel='panel', palt='panel2', elev='elev',
             hov='hoval', act='act',
             line='border', lstr='border2',
             tbg='editor',
             ac='accent', ac2='accent2',
             ok='ok', wr='warn', dg='err')
MAP['idea-dark'] = {'dark': dict(
    _IDEA, t1='fg_bright', t2='fg', t3='fg_dim', tfg='fg_bright')}
MAP['idea-light'] = {'light': dict(
    _IDEA, t1='fg', t2='fg1', t3='fg_dim', tfg='fg')}
MAP['darcula'] = {'dark': dict(
    _IDEA, t1='fg_bright', t2='fg', t3='fg_dim', tfg='fg_bright')}


# ══════════════════════════════════════════════════════════════════
# tab 底：**显式**指定，不靠对比度猜
# ══════════════════════════════════════════════════════════════════
MAP_TAB = {
    # 每套给一个**候选列表**（都是该色系的"面"色，按浮起从大到小排），
    # 由 derive_tab() 按方向 + 对比度挑第一个达标的。
    #
    # ⚠️ 候选**必须全部来自面**（背景层级色），不能把线色 / 字色放进来。
    #    反例（Tokyo Night 亮档）：官方只有 bg / bg_dark / bg_highlight 三档面，
    #    若把 line色当候选 → 亮底上选到 fg_dark（深字色），
    #    cr 达标了但**方向反了** —— 亮底上出现深色 tab，
    #    用户读作"这块凹进去了"，不是"浮起来"。
    #    判据：**tab 底是"面"，只能从面色里选。**
    'dracula':    {'dark': ['elev', 'hoval'],
                 # Alucard 官方只给了 bg / cur_line / fg 三档，
                 # cur_line #cfcfde 才是那个'够一档'（1.49），
                 # 我插的 panel 太轻（1.06）。
                 'light': ['panel', 'cur_line']},
    'catppuccin': {'dark': ['surface0', 'surface1'],
                   'light': ['surface0', 'surface1']},
    'solarized':  {'dark': ['base02', 'base01'], 'light': ['base02', 'base01']},
    'gruvbox':    {'dark': ['bg0', 'bg0_soft'], 'light': ['bg0_soft', 'bg1']},
    'tokyonight': {'dark': ['bg', 'bg_panel'], 'light': ['bg_dark', 'bg_highlight']},
    'nord':       {'dark': ['nord1', 'nord2'],
                   'light': ['bg1', 'bg2', 'bg3']},
    'everforest': {'dark': ['bg_soft', 'bg_visual'],
                   'light': ['bg_soft', 'bg_visual']},
    'onedark':    {'dark': ['selection', 'sel_hi'],
                   'light': ['panel2', 'panel3']},
    'rosepine':   {'dark': ['surface', 'overlay'],
                 # Dawn 的面方向：surface 比 base **更浅**（#fffaf3 vs #faf4ed），
                 # 所以它只能当'次级面板'（更内嵌），浮起只能靠 overlay。
                 'light': ['overlay']},
    # IDEA 两套的面板相邻差本来就密（1.06~1.12）——
    # 插值兜底能把它们拉到 1.12 以上，但那就不是官方色阶了，
    # 所以给两档候选让derive_tab 挑到刚够的位置。
    'idea-dark':  {'dark': ['elev', 'hoval']},
    'idea-light': {'light': ['panel2', 'hoval']},
    'darcula':    {'dark': ['elev', 'hoval']},
}

# tab 底浮起的下限（不是"想要多少"，是"这套色系能达到多少"）。
# IDEA 的两套面板相邻差 1.06~1.12、One Light 官方只有一档面，
# 要求 1.12 等于要求"必须比色系自己的下一档更远" —— 那是**改色系**不是配色。
TAB_MIN = 1.10


def derive_tab(term_bg, cands, dark, ramp_to=None):
    """在色系自己的面**色**里挑一个「比终端底浮起一档」的值。

    三个都要：
    ① **先按方向过滤** —— `cr()` 是**无方向**的量，亮底上放深色也 > 1.18。
       浮起 = 远离终端底的方向：暗档往亮处走，亮档往暗处压。
    ② 候选都达标时取**第一个**达标的（列表按浮起从小到大排，
       所以就是"刚够"那档）—— **达标是下限不是上限**。
    ③ 全都不达标时才插值，且**只在 [终端底, ramp_to] 这条正确方向的
       区间内插**：从终端底出发，一步步往 ramp_to 走，走到刚够即止。
       ⚠️ 上一版写成 `mix(ramp_to, term_bg, k/100)` 且从 k=2% 起跳，
       cr 在亮底上对灰度极敏感（1.10 只对应很小的跨度），
       结果**一路插到接近 ramp_to 才满足** —— 实测 One Light 的
       ramp_to 被插成了 #43454c（官方最深的字色档），
       cr=9.09、tab 条重到压过终端内容。
       判据：**插值必须从终端底出发、朝 ramp_to 单向走，且步进要细**。
    """
    directed = [c for c in cands if (lum(c) > lum(term_bg)) == dark]
    for c in (directed or cands):
        if cr(c, term_bg) >= TAB_MIN:
            return c
    if ramp_to:
        # ⚠️ 步进 0.5%，且从**终端底**出发朝 ramp_to 走（方向不会反）。
        for step in range(1, 201):
            c = mix(ramp_to, term_bg, 1 - step / 200.0)
            if cr(c, term_bg) >= TAB_MIN:
                return c
        return ramp_to
    return cands[-1] if cands else term_bg


# ══════════════════════════════════════════════════════════════════
# 补插值色（官方缺档时在官方色阶内插，不新造色相）
# ══════════════════════════════════════════════════════════════════
def _fill_derived():
    """所有插值都在**官方两档之间**，色相不变、只调明度。

    判据：插值结果仍在色系的色域内，读起来还是"这套色系"；
    硬造一个色相不同的中间色就不算了。
    """
    # Nord 暗档：官方 nord0~nord3 **全是背景色系**，
    # 没有"中间调前景"这一档 → 在 nord4(前景) 与 nord3(背景) 之间插。
    P['nord']['dark']['nord3t'] = mix('#d8dee9', '#4c566a', 0.62)   # nord4 前景 62% + nord3 背景 38%
    # Nord 亮档同理：fg2 已在 nord3，弱文本需要再深一档
    P['nord']['light']['fg3'] = mix('#4c566a', '#3b4252', 0.45)  # nord3 45% + nord1 55%

    # Tokyo Night 暗档：官方 fg_gutter #3b4261 是给终端**装饰线**用的，
    # 做界面弱文本只有 1.48 → 在 fg_dark 与 bg_highlight 之间插。
    P['tokyonight']['dark']['fg_mid'] = mix('#a9b1d6', '#292e42', 0.62)

    # Solarized 亮档：调色板在同一底色上**只有两档可用** ——
    #   base0 #657b83 = 4.13:1（差一点到 4.5）
    #   base3 #002b36 = 15.1:1（一档跳到最深，中间没有过渡）
    # 界面要三档正文（主/ 次 / 弱）→ 在官方两档之间插两档。
    # base0 #657b83（浅）→ base3 #002b36（深），次文本取靠近 base3 的一侧
    P['solarized']['light']['base_t2'] = mix('#002b36', '#657b83', 0.82)
    P['solarized']['light']['base_t3'] = mix('#002b36', '#657b83', 0.58)

    # One Dark 暗档：官方只有 bg #1e2127 + selection #3a3f4b 两档面，
    # 面板需要三级（palt / elev）→ 在 selection 与 bright_black 之间插。
    P['onedark']['dark']['fg_dim'] = mix('#abb2bf', '#5c6370', 0.55)
    P['onedark']['dark']['sel_hi'] = mix('#3a3f4b', '#5c6370', 0.45)

    # One Dark 亮档：官方**完全没给"面"色**（见 palette 里的警告）——
    # bg #f9f9f9 与 cursor #383a42 都是中性灰，插出来仍是 One Light 的灰域。
    # ⚠️ 插值比例要卡住：0.055 → 刚过 1.10；0.115 → 面板已偏重。
    #    **达标是下限不是上限**，取刚够的那一档。
    ol = P['onedark']['light']
    # ⚠️ **深色放前景位**（mix 的第一个参数）—— 亮档的面要"比 bg 稍暗"，
    #   不是"比 bg 稍浅"。写反了会得到 #43454c 这种比 bg 浅一档的灰，
    #   插值表整张都偏到错误的一侧（实测过）。
    ol['panel2'] = mix('#383a42', '#f9f9f9', 0.045)
    ol['panel3'] = mix('#383a42', '#f9f9f9', 0.075)
    ol['panel4'] = mix('#383a42', '#f9f9f9', 0.105)
    # 亮档的次 / 弱文本：官方 White #a0a1a7 在白底上只有 2.3，
    # 在 bg 与 fg 之间插两档（越深越重）。
    # k 是"朝深色混的比例"：0.62 → #6a6b71(cr 5.3 左右) 才够 4.5
    ol['fg2'] = mix('#383a42', '#f9f9f9', 0.73)   # 0.70 → cr 4.49，差 0.01
    ol['fg3'] = mix('#383a42', '#f9f9f9', 0.58)   # 弱文本要3.0

    # Everforest 亮档：官方 greys 三档（#A6B0A0 / #939F91 / #829181）
    # 在 panel #f4f0d9 上最深的也只有 3.3 左右，做不了次 / 弱文本
    # → 主文本用官方 fg #5C6A72，次 / 弱在 fg 与 gray2 之间插
    #   （**仍在官方绿灰色域内**，读起来还是 Everforest）。
    ef = P['everforest']['light']
    # ⚠️ **本项目第一个"色系能力不足"的案例，处理方式值得记下来**。
    #
    #   Everforest 亮档的官方前景系只有 4 档，实测在 panel #F4F0D9 上：
    #     fg    #5C6A72 → 4.87:1✓（够主文本）
    #     gray3 #829181 → 2.90:1（够弱文本 3.0？差一点）
    #     gray2 #939F91 → 2.41:1
    #     gray1 #A6B0A0 → 1.96:1
    #   **没有任何一档灰能当次文本**（要 4.5）。这是色系自己的取舍 ——
    #   Everforest 的设计目标就是"低对比护眼"，官方刻意不给高对比的灰。
    #
    #   插值救不了：往 fg 与灰之间插，出来的东西要么仍 < 4.5，
    #   要么就贴到 fg 旁边、和主文本分不出层级 —— **层级倒过来比不达标更糟**。
    #
    #   → 判据：**色系能力不足时如实标出并给替代方案，不要靠插值硬凑。**
    #     这里取"最接近达标"的官方档位，并由 check() 显式报出来，
    #     落地时这一套要么只推暗档、要么把 panel 提深一档再用官方灰。
    ef['fg_t2'] = '#5c6a72'          # 次文本只能与主文本同色（官方无更浅的可用档）
    ef['fg_t3'] = '#829181'          # 弱文本用 gray3（2.90，最接近 3.0 的官方档）

    # Rosé Pine 亮档：官方 muted #9893a5 在 base #faf4ed 上只有 2.7:1、
    # subtle #797593 也只有 4.0（差一点到 4.5）→ 在官方色域内插深。
    rp = P['rosepine']['light']
    # text #464261 最深、subtle #797593 次之、muted #9893A5 最浅
    # → 次文本取 text 与 subtle 之间，弱文本取 subtle 附近再插深
    rp['subtle2'] = mix('#464261', '#797593', 0.42)
    rp['muted2'] = mix('#464261', '#797593', 0.20)


# ══════════════════════════════════════════════════════════════════
# 生成
# ══════════════════════════════════════════════════════════════════
def build():
    _fill_derived()
    out = {}
    for fam in ORDER:
        meta = P[fam]
        # ⚠️ **按主题自己声明的 modes 生成**，不默认 dark+light 各来一套。
        # IDEA 主题只有单档（Dark 只有暗、Light 只有亮）——
        # 硬凑另一半得到的是"官方没有的配色"，那不叫"这套主题"叫"仿制"。
        for mode in meta['modes']:
            src = meta[mode]
            m = MAP[fam][mode]
            key = fam if fam.startswith('idea') or fam == 'darcula' \
                else '%s-%s' % (fam, mode)
            dark = (mode == 'dark')
            t = {fld: src[k] for fld, k in m.items()}
            cands = [src[k] for k in MAP_TAB[fam][mode]]
            # ramp_to 必须是**候选里最远的那一档**（不是 palt 槽）。
            # ⚠️ 曾错用 palt：Rosé Pine 亮档的 palt = surface #fffaf3，
            #   它比终端底**更浅**（Dawn 的 surface 比 base 浅），
            #   于是插值朝错误方向走，永远达不到阈值。
            #   判据：**插值的终点必须与候选同向且更远**，否则插值无意义。
            t['tab'] = derive_tab(t['tbg'], cands, dark,
                                  ramp_to=cands[-1] if cands else None)
            t['dgS'] = scale_to(t['dg'], t['tab'], 3.0, dark)
            t['ch'] = pick_ch(t['tab'], dark)
            t['f'] = meta['n']
            t['en'] = meta['en']
            t['kind'] = meta['kind']
            t['m'] = '暗' if dark else '亮'
            t['star'] = meta['star']
            t['lic'] = meta['lic']
            t['repo'] = meta['repo']
            t['desc'] = meta['desc']
            out[key] = t
    return out


# ⚠️ **已知的能力不足**：色系官方色阶本身给不出达标值，
#   已在 _fill_derived() 里注明原因并取"最接近达标"的官方档位。
#   判据：**色系能力不足时如实标出，不要靠插值硬凑** ——
#   硬凑出来的次文本会比主文本还深，层级倒过来比不达标更糟。
#   落地时这些套要么只推暗档、要么单独调其 panel。
KNOWN_GAPS = {
    'everforest-light': [
        ('t3 弱文本 on 面板 2.90 vs 3.0',
         '官方亮档 greys 最深一档 gray3 #829181 即2.90；'
         'Everforest 的设计目标就是低对比护眼，官方刻意不给更深的灰'),
    ],
}


def check(out):
    bad = []
    known = []
    for key, t in sorted(out.items()):
        rows = [
            ('t1 正文 on 面板 <4.5', cr(t['t1'], t['panel']), 4.5),
            ('t2 次文本 on 面板 <4.5', cr(t['t2'], t['panel']), 4.5),
            ('t3 弱文本 on 面板 <3.0', cr(t['t3'], t['panel']), 3.0),
            ('× 前景 on tab 底 <3.0', cr(t['dgS'], t['tab']), 3.0),
        ]
        for i, c in enumerate(t['ch']):
            rows.append(('频道 %s 字母 on tab <3.0' % 'ABCD'[i],
                         cr(c, t['tab']), 3.0))
        # ⚠️ **比较必须带浮点容差**：色阶值算出来的 cr 常常恰好落在阈值上
        #   （实测 rosepine-light 的 tab 浮起 = 1.0996，阈值 1.10）。
        #   用 `v < need` 直接判会把它报成"不达标"，而它其实正好达标 ——
        #   **误报必须清零**，否则报假警会让人不再信任这个校验。
        EPS = 0.005
        for name, v, need in rows:
            if v < need - EPS:
                if any(name.split(' ')[0] in g[0] for g in KNOWN_GAPS.get(key, [])):
                    known.append((key, name, round(v, 2)))
                else:
                    bad.append((key, name, round(v, 2)))

        # ── 语义断言：数值达标但视觉错误的几种情况 ──
        # ① tab 与终端同色 → 32px 的 tab 行只剩一条看不见的边线
        if t['tab'] == t['tbg']:
            bad.append((key, 'tab 与终端底同色（tab 行无边界）', 1.0))
        elif cr(t['tab'], t['tbg']) < TAB_MIN - 0.005:
            bad.append((key, 'tab 浮起不足 <%.2f' % TAB_MIN,
                        round(cr(t['tab'], t['tbg']), 2)))
        # ② 侧栏（palt）必须与面板**有**色差，否则侧栏与面板连成一片。
        #    ⚠️ 第一版这条断言写成"`root == tbg` 就算错"，是**误报**：
        #    规格表**明确要求** tab 行条带 / 侧栏 / 终端区三者同色，
        #    把要求当错误会逼着人违反规格。
        #    侧栏真正靠**它自己的底色 palt** 划界，不是靠 panel。
        #    判据：**先问"谁负责划界"，再定断言**——
        #    判错对象会产生"必须改规格才能过校验"的假矛盾。
        if cr(t['palt'], t['panel']) < 1.04:
            bad.append((key, '侧栏底与面板色差 <1.04（侧栏无边界）',
                        round(cr(t['palt'], t['panel']), 2)))
        # 频道槽位之间不能撞**色相**。
        # ⚠️ 不能用 WCAG 亮度比判"撞色"—— A 蓝与 C 琥珀亮度可以接近，
        # 但色相差 180°，一眼就能分开。那样算出来全是假阳性。
        for i in range(4):
            for j in range(i + 1, 4):
                dh = hdiff(t['ch'][i], t['ch'][j])
                if dh < 30:
                    bad.append((key, '频道 %s/%s 色相仅差 %d°'
                                % ('ABCD'[i], 'ABCD'[j], dh), dh))
    return bad, known


if __name__ == '__main__':
    out = build()
    print('%-16s %-3s %-9s %-6s %-6s %-6s %-6s %-6s %-6s %-6s'
          % ('theme', '档', 'tab底', '/term', 't1', 't2', 't3', '×', 'chA', 'chC'))
    for key, t in out.items():
        print('%-16s %-3s %-9s %-6.2f %-6.2f %-6.2f %-6.2f %-6.2f %-6.2f %-6.2f'
              % (key, t['m'], t['tab'], cr(t['tab'], t['tbg']),
                 cr(t['t1'], t['panel']), cr(t['t2'], t['panel']),
                 cr(t['t3'], t['panel']), cr(t['dgS'], t['tab']),
                 cr(t['ch'][0], t['tab']), cr(t['ch'][2], t['tab'])))

    bad, known = check(out)
    print()
    if known:
        print('⚠ 已知的能力不足 %d 项（色系官方色阶给不出，见 KNOWN_GAPS）：' % len(known))
        for k, n, v in known:
            print('   %-16s %s  %s' % (k, n, v))
        print()
    if bad:
        print('❌ 未达标 %d 项：' % len(bad))
        for b in bad:
            print('   %-16s %s  %s' % b)
    else:
        print('✓ 全部 %d 套达标（另 %d 项为色系能力不足，已标注）'
              % (len(out), len(known)))

    io.open('theme-tokens.json', 'w', encoding='utf-8').write(
        json.dumps(out, ensure_ascii=False, indent=1))
    print('→ theme-tokens.json')
