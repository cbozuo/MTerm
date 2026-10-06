# -*- coding: utf-8 -*-
"""合并「社区终端色系」与「现代 UI 色板」两族 → 一份统一数据。

两族并存的理由（这是本方案的一个关键判断）：
──────────────────────────────────────────────
社区终端色系（Nord / Dracula / Gruvbox…）是**给终端输出配色**的，
把它们直接铺满整个界面会掉一个档次 ——
  ① 它们没有"三级面板底"，终端里只有"底 + 文字"
  ② 层次靠色相偏移而非明度分层→ 面板之间色差只有 0.02~0.03，糊
  ③ 硬对比是为长时间读输出服务的，界面需要的是**柔和分层**

Radix Colors 相反：它是**为界面做的**（shadcn/ui 的底座），
  ① 12 档等距阶梯，每档明确标注用途
  ② 对比度按 **APCA 算法**标（比 WCAG 更贴近人眼）
  ③ 有 **P3 色域**支持

→ **两类都要，但在选单里分组**，让用户知道自己看到的是哪一类。
  判据：**色系要按"它服务什么"选，不是按"它多好看"选。**
  把终端配色当界面主题用，就像把工程测量尺当装饰用 —— 不是不行，是不对口。
"""

import io
import json

COMMUNITY = json.load(io.open('theme-tokens.json', encoding='utf-8'))
RADIX = json.load(io.open('_radix-themes.json', encoding='utf-8'))

# ── 顺序：现代 UI 色板打头（用户找"现代"时第一眼看到）────────
GROUPS = [
    ('radix',   '现代 UI 色板',
     'Radix Colors 体系 · 为界面设计，明度等距 12 档。'
     '面板层级清晰、柔和分层，是四者里最"现代"的一类。'),
    ('terminal', '经典终端配色',
     '终端社区最常用的几套 · 为长时间读输出设计，'
     '硬对比、护眼。深色档尤其适合暗环境。'),
    ('idea',    'JetBrains 官方',
     'IDEA 的三个默认 UI 主题 · 商标属 JetBrains，'
     '落地写"配色参考"不要冠产品名。'),
]

# 社区那 9 套按 star 降序
TERM_ORDER = [
    ('dracula', 'dracula-dark', 'dracula-light'),
    ('catppuccin', 'catppuccin-dark', 'catppuccin-light'),
    ('solarized', 'solarized-dark', 'solarized-light'),
    ('gruvbox', 'gruvbox-dark', 'gruvbox-light'),
    ('tokyonight', 'tokyonight-dark', 'tokyonight-light'),
    ('nord', 'nord-dark', 'nord-light'),
    ('everforest', 'everforest-dark', 'everforest-light'),
    ('rosepine', 'rosepine-dark', 'rosepine-light'),
    ('onedark', 'onedark-dark', 'onedark-light'),
]
RADIX_ORDER = [
    ('graphite-dark', 'graphite-light'),
    ('slate-dark', 'slate-light'),
    ('mauve-dark', 'mauve-light'),
    ('sage-dark', 'sage-light'),
    ('sand-dark', 'sand-light'),
    ('olive-dark', 'olive-light'),
]
IDEA_ORDER = ['idea-dark', 'idea-light', 'darcula']


def build():
    out = {}
    for mid in RADIX_ORDER:
        for k in mid:            # mid = (dark_id, light_id)
            t = dict(RADIX[k])
            t['group'] = 'radix'
            out[k] = t
    for _fam, d, l in TERM_ORDER:
        for k in (d, l):
            t = dict(COMMUNITY[k])
            t['group'] = 'terminal'
            out[k] = t
    for k in IDEA_ORDER:
        t = dict(COMMUNITY[k])
        t['group'] = 'idea'
        out[k] = t
    return out


def ordered(out):
    """返回 (group_id, group_name, group_desc, [变体id...]) 列表。"""
    res = []
    for gid, gname, gdesc in GROUPS:
        ids = []
        if gid == 'radix':
            for d, l in RADIX_ORDER:
                ids += [d, l]
        elif gid == 'terminal':
            for _f, d, l in TERM_ORDER:
                ids += [d, l]
        else:
            ids = IDEA_ORDER[:]
        res.append((gid, gname, gdesc, ids))
    return res


if __name__ == '__main__':
    out = build()
    print('共 %d 个变体' % len(out))
    for gid, name, desc, ids in ordered(out):
        print('\n【%s】%d 个' % (name, len(ids)))
        for i in ids:
            t = out[i]
            star = ('%d★' % t['star']) if t.get('star') else 'JetBrains'
            print('   %-18s %-4s %-9s %s'
                  % (i, t['m'], t['ac'], star))
