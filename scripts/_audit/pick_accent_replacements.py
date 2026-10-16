# -*- coding: utf-8 -*-
"""(#accent-preset-swap 2026-10-16) 给强调色预设网格挑两块「主流、与现有 48块
不冲突」的替代色，替掉用户红框圈出的「白光色 #FAFAFA」「MTerm 青 #1FC8B4」。

判据（都按 WCAG 相对亮度算，与项目内其它取色脚本同式）：
  ① minΔE(CIE76, Lab) —— 与网格里**其余** 46 块的最小色差，>=12 才算「不冲突」；
  ② 亮度不落在两端极值（别再是 98% 纯白 / 26% 近黑那种没法当强调色的）；
  ③ 深底上可读：对比度 >= 4.5:1（否则终端/按钮上看不清），浅底上不刺眼；
  ④ 明度与「白光色」原位（网格倒数第 4 列附近都是浅/中性档）形成层次。
输出候选表 + 排序，最后人工从「主流色」里挑。
"""
import colorsys
import re

SRC = 'ui/app.slint'
BLK = re.search(r'for p\[i\] in \[(.*?)\] : Rectangle',
                 open(SRC, encoding='utf-8').read(), re.S).group(1)
GRID = re.findall(r'\{ n: "([^"]+)", h: "(#[0-9A-Fa-f]{6})" \}', BLK)

DROP = {'白光色', 'MTerm 青'}          # 红框圈出的两块


def rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def lin(c):
    c /= 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def lum(c):
    return 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])


def cr(a, b):
    la, lb = lum(a), lum(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def lab(c):
    r, g, b = (lin(v) for v in c)
    x = (r * 0.4124 + g * 0.3576 + b * 0.1805) / 0.95047
    y = r * 0.2126 + g * 0.7152 + b * 0.0722
    z = (r * 0.0193 + g * 0.1192 + b * 0.9505) / 1.08883

    def f(t):
        return t ** (1 / 3) if t > 0.008856 else 7.787 * t + 16 / 116
    fx, fy, fz = f(x), f(y), f(z)
    return (116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz))


def dE(h1, h2):
    a, b = lab(rgb(h1)), lab(rgb(h2))
    return sum((x - y) ** 2 for x, y in zip(a, b)) ** 0.5


keep = [(n, h) for n, h in GRID if n not in DROP]

# 主流品牌色候选（都是有出处的官方主色，不是自造色）
CANDIDATES = [
    ('Discord 蓝紫', '#5865F2'),
    ('Stripe 紫',    '#635BFF'),
    ('Slack 紫',     '#4A154B'),
    ('GitHub 绿',    '#2DA44E'),
    ('GitHub 蓝',    '#0969DA'),
    ('Twitter 蓝',   '#1D9BF0'),
    ('Netflix 红',   '#E50914'),
    ('Figma 紫',     '#A259FF'),
    ('Figma 橙',     '#F24E1E'),
    ('Spotify 绿',   '#1DB954'),
    ('Notion 棕',    '#9F6B53'),
    ('Vercel 黑',    '#000000'),
    ('Spotify 黑',   '#191414'),
    ('Duolingo 绿',  '#58CC02'),
    ('Zoom 蓝',      '#2D8CFF'),
    ('Shopify 绿',   '#95BF47'),
    ('Linear 靛',    '#5E6AD2'),
    ('Supabase 青',  '#24B47E'),
    ('Twitch 紫',    '#9146FF'),
    ('红帽红',       '#EE0000'),
    ('Intel 蓝',     '#0071C5'),
    ('Chrome 黄',    '#F4B400'),
    ('Firefox 橙',   '#FF7139'),
    ('Opera 红',     '#FF1B2D'),
    ('Steam 蓝',     '#1B2838'),
    ('DeepMind 蓝',  '#4285F4'),
    ('Claude 陶土',  '#D97757'),
    ('ChatGPT 绿',   '#10A37F'),
    ('Gemini 蓝',    '#8AB4F8'),
    ('Apple 灰',     '#8E8E93'),
    ('石墨蓝',       '#4E6E8E'),
    ('电报蓝',       '#2AABEE'),
    ('Discord 绿',   '#57F287'),
]

print('=== 候选评分（minΔE 越大越不冲突；dark/fg = 深底对比度）===')
rows = []
for name, hx in CANDIDATES:
    m = min(dE(hx, h2) for _, h2 in keep)
    hl = lum(rgb(hx))
    dark_fg = cr(rgb(hx), rgb('#181A1D'))     # mterm-dark 底
    light_fg = cr(rgb(hx), rgb('#FAF8F4'))   # mterm-light 底
    hh, ss, ll = colorsys.rgb_to_hls(*[v / 255 for v in rgb(hx)])
    rows.append((m, name, hx, hh * 360, ss, ll, dark_fg, light_fg))

for m, name, hx, H, S, L, d, l in sorted(rows, reverse=True):
    print(f'  minΔE={m:5.1f}  {name:12s} {hx}  hue={H:5.0f} sat={S * 100:3.0f} lig={L * 100:3.0f}  '
          f'底181A1D={d:5.2f} 底FAF8F4={l:5.2f}')

print()
print('=== 参考：网格里现有块两两 minΔE 的分布 ===')
allmin = []
for i, (_, h1) in enumerate(GRID):
    others = [h2 for j, (_, h2) in enumerate(GRID) if i != j]
    allmin.append(min(dE(h1, h) for h in others))
allmin.sort()
print('  最小 ΔE: p5=%.1f  p25=%.1f  中位=%.1f  最大=%.1f'
      % (allmin[len(allmin) // 20], allmin[len(allmin) // 4],
         allmin[len(allmin) // 2], allmin[-1]))
print('  被替换的两块 minΔE:', ['%s=%.1f' % (n, min(dE(h, x) for _, x in keep))
                            for n, h in GRID if n in DROP])