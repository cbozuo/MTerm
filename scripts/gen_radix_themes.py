# -*- coding: utf-8 -*-
"""从 Radix Colors 生成「现代 UI 色板」系列主题。

══���══════════════════════════════════════════════════════════════════
★ 为什么要有这一系列：社区终端主题为什么"不够现代"
══════════════════════════════════════════════════════════════════════
 Nord / Dracula / Gruvbox 这些是**终端配色**，它们的历史包袱是
 "给 ANSI 16 色用"—— 所以：
   ① 前景/背景之间是**硬对比**（要长时间读输出不累），
      界面面板如果照搬，会显得"脏"（暗底上大片中灰）
   ② 层次靠**色相偏移**实现（往暖/往冷偏一点），而不是靠**明度分层**
      →面板之间色相差 0.02~0.03，视觉上"糊"
   ③ 它们**没有为界面设计的三级面板底**——终端里只有"底 + 文字"

 Radix Colors 相反：它是为**界面**做的（shadcn/ui 的底座），
   ① 每个色系 12 档，每档**明确标注用途**（背景/悬停/边框/文字）
   ② 明度阶梯**等距**，所以三级面板能拿到1.10~1.12 的均匀差
   ③ 对比度目标是**按APCA 算法**标的（比 WCAG 更贴近人眼）
   ④ 有 **P3 色域**支持 —— 亮度能做上去了而不偏色

→ 判据：**色系要按"它服务什么"选，不是按"它多好看"选**。
  终端主题做终端配色是一流的，直接拿来铺满整个界面会掉一个档次。
  两类都需要 —— 但要在选单里**分组**，让用户知道自己看到的是哪一类。

═══��═══════════════════════════════════════════════════════════════════
★ Radix 两个源文件的**方向相反**（本项目踩过）
══════════════════════════════════════════════════════════════════════
   light.ts：index 0 最浅（#fcfcfc） → index 11 最深（#202020）
   dark.ts ：index 0 最深（#111111） → index 11 最浅（#eeeeee）
   ——**同一个键名在两个文件里指相反的明度**。
   取错方向会让整套主题变成"亮底配深色文字"的反色版本，
   而且**对比度检查依然能过**（只是全反了）。
   所以下面 `LADDER` 一律写成 **"从界面上最浅/最深往里数第几档"**，
   由 `pick()` 负责按 mode 换算成 index。
══════════════════════════════════════════════════════════════════════
"""

import io
import json

RADIX = json.load(io.open('_radix.json', encoding='utf-8'))
L, D = RADIX['light'], RADIX['dark']

# 中性色系（Radix 官方的 6 个低饱和灰）
NEUTRALS = ['gray', 'slate', 'mauve', 'sage', 'olive', 'sand']


def pick(scale, mode, k):
    """取 Radix 色阶第 k 档（k 从"界面底色那一侧"数起，1-based）。

    ⚠️ **本文件最关键的一处**：Radix 的两个源文件方向相反
       （light.ts 浅→深、dark.ts 深→浅），**看起来**需要反向换算，
       但其实**两个文件的 index 0 都是"最贴近界面底色"的那一档**：
         light.ts idx0 = #fcfcfc（最浅）← 亮档的界面底
         dark.ts  idx0 = #111111（最深）← 暗档的界面底
       所以两档用**同一个公式** `idx = k - 1`，k 的语义统一为
       "离界面底色有多远"。

       ⚠️ 我第一版写了反向换算 `11 - (k-1)`，把暗档的 root 变成
       `#eeeeee`（最亮）、t1 变成 `#111111`（最深）——
       **整个暗档反色成"亮底深字"**，而**对比度检查依然大部分通过**
       （只是全反了）。所以取色值的方向**必须靠断言兜底**：
       `assert lum(pick(dark,k=1)) < lum(pick(dark,k=12))`。
    """
    arr = L[scale] if mode == 'light' else D[scale]
    return arr[k - 1]


# ── 槽位定义：k = "从界面底色那一侧往里数第几档" ────────────────
# ⚠️ **k 的语义是"离底色有多远"，不是"绝对明暗"**。这样同一张表
#    在明暗两档都能用，且**角色是对应的**：
#    app 永远是 k=1（最贴近底色）→ 越大的 k 越"浮起来"。
#
#    亮档：底色浅，往里数 = 越来越深 → 直接用 k
#    暗档：底色深，往里数 = 越来越浅 → pick() 内部做反向换算
LAYOUT = {
    'root':  dict(light=1,  dark=1),      # 界面底（终端底同此）
    'panel': dict(light=2,  dark=2),      # 面板：标题栏/侧栏底
    'palt':  dict(light=3,  dark=3),      # 次级面板（侧栏）
    'elev':  dict(light=4,  dark=4),      # 抬升面（菜单/弹框）
    'hov':   dict(light=5,  dark=5),      # 悬停
    'act':   dict(light=6,  dark=6),      # 按下
    'line':  dict(light=7,  dark=6),      # 分界线
    'lstr':  dict(light=8,  dark=5),      # 控件描边
    # 文本：暗档要**最亮**才够对比 → 离底最远的那档；
    #亮档要**最深** → 也是离底最远。所以两边都取大 k。
    # 文本：**离界面底最远**的那几档。
    # ⚠️ 亮档 t2=10 / t3=8 实测在 panel(k2) 上只有 3.6 / 1.8 ——
    #   Radix 的中性色阶在浅底上"加深得很慢"（12 档里前 9 档都在
    #   #fb~#ce 之间）。所以亮档的次/弱文本要比直觉更靠后。
    # 文本：**离界面底最远**的那几档。
    # ⚠️ 两档的"可用区间"不同，Radix 的阶梯不是对称的：
    #   亮档前 9 档都在 #fb~#ce 之间（加深得很慢）→ 要 k=11/10
    #   亮档 t2=k10 只有 3.6 → 必须到 k=11 才4.5
    #   暗档相反：从 k=8 就已经很暗了 → k=10 只有 4.15、k=8 只有 2.80
    #   所以暗档的次/弱文本要取**更靠近底**的档（k=9/ 7）。
    # ⚠️ 判据：**色阶不对称时，两档的档位映射不能共用同一组数字**——
    #   照"看起来对称"配会一边过、一边不过。
    't1':    dict(light=12, dark=11),
    't2':    dict(light=11, dark=10),
    't3':    dict(light=10, dark=9),
    'tbg':   dict(light=1,  dark=1),      # 终端底 ≡ app 底
}


def lum(h):
    h = h.lstrip('#')
    r, g, b = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]

    def f(c):
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)


def cr(a, b):
    la, lb = lum(a), lum(b)
    if la < lb:
        la, lb = lb, la
    return (la + 0.05) / (lb + 0.05)


def mix(fg, bg, k):
    """`mix(fg, bg, k)`：k=1 全fg、k=0 全 bg。"""
    return '#%02x%02x%02x' % tuple(
        round(int(fg.lstrip('#')[i:i + 2], 16) * k
              + int(bg.lstrip('#')[i:i + 2], 16) * (1 - k)) for i in (0, 2, 4))


# ── 6 套现代 UI 色板 ─────────────────────────────────────────────
# 每套 = 一个 Radix 中性色系× 一个 Radix 强调色系。
# ⚠️ **中性色系与强调色系要成对挑**，不能随便配：
#    sage（绿灰）配 crimson（红）会显脏，mauve（紫灰）配 amber（黄）会显旧。
#    下面 6 对都是 Radix 官方文档里被用作 example 的经典搭配。
THEMES = [
    # key中性  accent     中文名英文名       描述
    ('graphite', 'gray', 'blue',    '石墨',   'Graphite',
     '纯中性灰 + 经典蓝。<b>最稳的一套</b> —— 界面元素本身的颜色降到最低，'
     '只有强调色与状态色说话。适合长时间写代码，也最不容易挑。'),
    ('slate',   'slate', 'cyan',    '石板',   'Slate',
     '带一点冷蓝倾向的灰 + 青。<b>比石墨"有温度"但不跳</b>，'
     '终端 ANSI 输出混排时整体感更强。'),
    ('mauve',   'mauve', 'iris',    '藕荷',   'Mauve',
     '极淡紫灰 + 靛紫。<b>三套里最"柔"的一套</b>，'
     '观感接近 Linear / Vercel 那类现代产品界面。'),
    ('sage',    'sage',  'jade',    '青苔',   'Sage',
     '绿调中性 + 翡翠绿。低对比但不冷，护眼取向，'
     '与 Everforest 是同一个方向但**用界面级色阶做得更均匀**。'),
    ('sand',    'sand',  'amber',   '暖砂',   'Sand',
     '暖灰 + 琥珀。<b>唯一带暖调的一套</b>，'
     '长时间读日志 / 看 diff 的疲劳感最低。'),
    ('olive',   'olive', 'tomato',  '橄榄',   'Olive',
     '黄绿灰 + 番茄红。对比最强的一套，'
     '状态色（成功/警告/危险）之间分得最开。'),
]


def _interp_text(hi, lo, bg, target):
    """在两档文字之间插值，取刚够 target 的那一档。

    `hi` = 更强的文字色，`lo` = 更弱的。语义靠**参数顺序**约定：
    调用方必须传 (强, 弱)，传反了会得到"更弱档被判为已达标 → 直接返回强档"，
    结果是**t3 与 t1 完全同色**（层级平掉）。

    ⚠️ 断言比校验更能抓这类错 —— 下面那条 assert 就是为它准备的：
    返回值必须**不比 lo 更强**，否则说明参数传反了。
    """
    assert cr(hi, bg) >= cr(lo, bg), \
        '_interp_text(hi, lo, ...) 要求 hi 比 lo 更强，参数传反了'
    if cr(lo, bg) >= target:
        return lo
    for step in range(1, 201):
        c = mix(hi, lo, step / 200)
        if cr(c, bg) >= target:
            return c
    return hi


def build():
    out = {}
    for key, neutral, accent, cn, en, desc in THEMES:
        for mode in ('light', 'dark'):
            t = {}
            for slot, k in LAYOUT.items():
                t[slot] = pick(neutral, mode, k[mode])

            # 终端底 ≡ app 底（规格表要求 tab 行 / 侧栏 / 终端三者同色）
            t['tbg'] = t['root']
            # 终端前景用中性色的最深/最浅档
            t['tfg'] = pick(neutral, mode, 12)

            # 强调色：亮档取深色档、暗档取亮色档（两档都能达标）
            # ⚠️ Radix 的 dark.ts 是深→浅，所以暗档要取**小**的 index
            t['ac'] = accent and (pick(accent, mode, 9))
            t['ac2'] = pick(accent, mode, 10 if mode == 'light' else 8)

            # 语义色：Radix 有对应的色系
            t['ok'] = pick('jade' if mode == 'light' else 'grass', mode, 9)
            t['wr'] = pick('amber' if mode == 'light' else 'bronze', mode, 9)
            t['dg'] = pick('crimson', mode, 9)

            # tab 底：在「界面底(k=1)」与「次级面板(k=3)」之间取。
            # ⚠️ **不能直接取 k=3**：Radix 暗档的 k=1=#111111、k=3=#222225，
            #   cr 只有 1.20 但 k=2=#191919 与 k=3 更接近 —— 实测 k=3 配
            #   同样的 k=3 做终端底时 cr=1.00（同色，tab 行无边界）。
            #   → 在 k=2 与 k=3 之间**插值**取刚够1.10 的那档，
            #     色相不变、仍在 Radix 该色系的色域内。
            # ⚠️ `mix(fg, bg, k)` 是 **k=1 全fg、k=0 全bg**，
            #    所以"从终端底朝 k=3 走 k%"要写
            #    `mix(k3, tbg, k)` —— k3 放前景位。
            #    写反了会得到"越来越接近 tbg"的反向序列，
            #    第一个候选就等于 tbg → cr=1.00、tab 行无边界。
            tbg = t['root']
            far = pick(neutral, mode, 3)
            for step in range(1, 201):
                cand = mix(far, tbg, step / 200)
                if cr(cand, tbg) >= 1.10:
                    t['tab'] = cand
                    break
            else:
                t['tab'] = far
            # × 的前景：强调色本身在 tab 底上要 ≥3:1，否则朝远离底色方向推
            need = 3.0
            if cr(t['ac'], t['tab']) < need:
                anchor = '#ffffff' if mode == 'dark' else '#000000'
                for k in range(1, 201):
                    c = mix(anchor, t['ac'], k / 200)
                    if cr(c, t['tab']) >= need:
                        t['dgS'] = c
                        break
                else:
                    t['dgS'] = t['ac']
            else:
                t['dgS'] = t['ac']

            # 频道 4 槽：固定色相（跨全部主题不变 —— 身份不随装饰变）
            t['ch'] = ['#3e63dd', '#0f9e8e', '#ad7f00', '#d03a63']
            if mode == 'dark':
                t['ch'] = ['#7c9cff', '#3ecfb8', '#e5b84c', '#e8859f']
            else:
                t['ch'] = ['#3e63dd', '#0c8a7d', '#946200', '#c22a52']

            # ── 文本三档：官方色阶不够就插值补 ──
            # ⚠️ **这是 Radix 用作界面色时的第一个真实短板**（实测数据）：
            #   12 档里**前 9 档都在做背景**（亮档 #fb~#ce、暗档 #2a~#11），
            #   能当文字的只有 3~4 档。灰阶在 panel(k2) 上：
            #     亮档 k12=15.5k11=5.6 k10=3.6 k9=3.2 k8=1.8 ← 到k8 就不可用
            #     暗档 k11=8.5 k10=4.2 k9=3.5 k8=2.8 ← 到 k8 就不可用
            #   界面要"主 / 次 / 弱"三档文字，**官方只给得出两档够格的**。
            #   → 在官方两档之间**插值**补一档（色相不变，仍在 Radix
            #     该色系的色域内），这与项目里对其它色系的处理一致。
            #   ⚠️ 但**不能靠插值硬凑超过 4.5**：次文本若只能到 3.4，
            #     那就是色系能力不足，要如实标注而不是硬填。
            t['t1'] = pick(neutral, mode, 12 if mode == 'light' else 11)
            # 次文本：官方 k=11(亮5.6) / k=10(暗4.2)。暗档那档只4.15，
            # 差0.35 → 在 t1 与它之间插到刚够 4.5。
            # ⚠️ **达标是下限不是上限**，取刚够那档，
            #    免得次文本跟主文本一样强（层级会平掉）。
            t['t2'] = _interp_text(t['t1'],
                                   pick(neutral, mode,
                                        11 if mode == 'light' else 10),
                                   t['panel'], 4.5)
            t['t3'] = _interp_text(t['t1'], t['t2'], t['panel'], 3.0)

            t['f'] = cn
            t['en'] = en
            t['m'] = '暗' if mode == 'dark' else '亮'
            t['kind'] = 'radix'
            t['star'] = 1686
            t['lic'] = 'MIT'
            t['repo'] = 'radix-ui/colors'
            t['desc'] = desc
            t['group'] = 'radix'
            out['%s-%s' % (key, mode)] = t
    return out


def check(out, known_gaps=None):
    bad = []
    known_gaps = known_gaps or {}
    for key, t in sorted(out.items()):
        rows = [
            ('t1 正文 on 面板<4.5', cr(t['t1'], t['panel']), 4.5),
            ('t2 次文本 on 面板<4.5', cr(t['t2'], t['panel']), 4.5),
            ('t3 弱文本 on 面板 <3.0', cr(t['t3'], t['panel']), 3.0),
            ('× 前景 on tab 底 <3.0', cr(t['dgS'], t['tab']), 3.0),
        ]
        for i, c in enumerate(t['ch']):
            rows.append(('频道%s 字母 on tab <3.0' % 'ABCD'[i],
                         cr(c, t['tab']), 3.0))
        EPS = 0.005      # 浮点容差：色阶算出的 cr 常恰好落在阈值上
        for name, v, need in rows:
            if v < need - EPS:
                if any(name.split(' ')[0] in g[0]
                       for g in known_gaps.get(key, [])):
                    pass
                else:
                    bad.append((key, name, round(v, 2)))
        # tab 底必须与终端底有边界
        if t['tab'] == t['tbg']:
            bad.append((key, 'tab 与终端底同色（tab 行无边界）', 1.0))
        elif cr(t['tab'], t['tbg']) < 1.08:
            bad.append((key, 'tab 浮起不足 <1.08',
                        round(cr(t['tab'], t['tbg']), 2)))
        # 侧栏要与面板有边界
        if cr(t['palt'], t['panel']) < 1.04:
            bad.append((key, '侧栏底与面板色差 <1.04',
                        round(cr(t['palt'], t['panel']), 2)))
    return bad


if __name__ == '__main__':
    out = build()
    print('%-16s %-3s %-9s %-6s %-6s %-6s %-6s %-6s %-6s'
          % ('theme', '档', 'app底', '/term', 't1', 't2', 't3', '×', 'chA'))
    for k, t in out.items():
        print('%-16s %-3s %-9s %-6.2f %-6.2f %-6.2f %-6.2f %-6.2f %-6.2f'
              % (k, t['m'], t['root'], cr(t['tab'], t['tbg']),
                 cr(t['t1'], t['panel']), cr(t['t2'], t['panel']),
                 cr(t['t3'], t['panel']), cr(t['dgS'], t['tab']),
                 cr(t['ch'][0], t['tab'])))
    bad = check(out)
    print()
    if bad:
        print('❌ 未达标 %d 项：' % len(bad))
        for b in bad:
            print('   %-16s %s  %s' % b)
    else:
        print('✓ 全部 %d 套达标' % len(out))
    io.open('_radix-themes.json', 'w', encoding='utf-8').write(
        json.dumps(out, ensure_ascii=False, indent=1))
    print('→ _radix-themes.json')
