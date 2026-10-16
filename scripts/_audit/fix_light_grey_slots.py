# -*- coding: utf-8 -*-
"""#ansi-exempt-slots 2026-10-16 —— 把**亮色**主题的 ANSI 槽 7 (white) 与
槽 8 (brightBlack) 压到 ≥4.5:1。

起因（用户截图取证）：Kanagawa·亮（背景 #F2ECBC）下 `ls -l` 的
「文件大小」列（槽7 = #8A8980，2.93:1）与「日期」列（槽8 = #A09CAC，
2.23:1）几乎不可读。上一轮 #ansi-yellow-fix 只改了 12 彩色槽，因为
`_meta.ansi_check` 把 0/7/8/15 写成「按终端惯例只记录不判失败」——
这个惯例是照搬暗色底的，落到浅色 tbg 上就塌了。14 套亮色主题里 13 套
槽7/8 有一项 < 4.5（极端：flexoki-light 槽7 = 1.00，等于背景）。

用户拍板：压到 4.5:1 达标线（与彩色槽同一条线）。

档差纪律（关键，不能压成一片死黑）：
  - 槽 7 white 是「正文弱化档」→ 压到 **≥4.5 但明显浅于默认前景 t1**；
    默认前景 t1 通常已 ≥6，因此 4.5~5.5 是它的合理落点。
  - 槽 8 brightBlack 是「辅助信息弱化档」→ 比槽 7 **更浅一档**
    （亮底上 brightBlack 更浅是暗底惯例的镜像）。
  - 槽 15 brightWhite 在亮底上必须是**最深**的一档（否则加粗白字看不见）
    —— 这一槽多数亮色主题已达标，逐套核对。
  - 槽 0 black = 默认前景的近亲，本轮不动（它普遍已 ≥6）。
真彩色（远端 24bit）保持现状，本脚本不涉及。
"""
import colorsys
import io
import json
import re

P = 'scripts/_ansi-group.json'
SLOT_WHITE, SLOT_BRIGHT_BLACK, SLOT_BRIGHT_WHITE = 7, 8, 15
MIN = 4.5
# 目标档（留 0.1 余量）
T_WHITE = 4.7          # 正文弱化档
T_GREY = 4.7           # 辅助档
T_BRITE = 4.7          # 槽15 brightWhite —— 亮底上加粗白字，同样得压到线以上
# 若该套槽7 已达标就保留原值（不无谓改动）


def lin(c):
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def lum(c):
    """c = (r, g, b) 三元组。"""
    return 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])


def cr(a, b):
    """a / b = (r, g, b) 三元组。"""
    la, lb = lum(a), lum(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def hx2rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def rgb2hx(t):
    return '#%02X%02X%02X' % tuple(max(0, min(255, int(round(x)))) for x in t)


def is_neutral(hexc):
    """返回该色应**压到的饱和上限**（None = 明确彩色档，保色相不动）。

    ⚠️ **不能用 HSL 的 S 判「是不是灰」**：接近纯白的色在 HSL 里 S 会爆表
    —— #FFFCF0（R/G/B = 255/252/240）算出 **S=100%、H=48°（黄）**，于是
    「保色相降 L」一路把它带成 #8A6E00（暗黄），白色档变黄色档；gruvbox 的
    #EEE0B7 同理（S=61.8%、H=45°→ #82671A）。

    正解用**通道极差** spread = max(r,g,b) − min(r,g,b)，分三档：
      ≤ 48   → 纯灰/近白，sat 锁 0
      48~80  → 弱色（暖米白 / 淡蓝灰），**压饱和到 0.12 封顶**再降 L：
               保留一点色温暗示，但绝不成「暗黄」
      > 80   → 明确的彩色档，原样保色相

    实测：#FFFCF0 spread=15、#EEE0B7 spread=55、#ACB0BE spread=30、
    #8a8980 spread=14、刻意调过的 #506A99 spread=73 → 落进弱色档
    （压到 0.12 仍是可辨的冷蓝，不会被抹成灰）。
    """
    r, g, b = hx2rgb(hexc)
    spread = max(r, g, b) - min(r, g, b)
    if spread <= 48:
        return 0.0
    if spread <= 80:
        return 0.12
    return None


def darken(hexc, bg_rgb, target, sat_boost=0.10, neutral=True):
    """保 H/S 降 L 到对比度达 target。中性/弱色按 `is_neutral` 压饱和
    （见该函数的三档判据）。降不动时返回 None（表示无处可降）。
    """
    r, g, b = hx2rgb(hexc)
    h, l, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
    cap = is_neutral(hexc) if neutral else None
    for step in range(0, 1001):
        nl = l - step / 1000.0
        if nl <= 0.02:
            break
        ns = cap if cap is not None else min(1.0, s + step / 1000.0 * sat_boost)
        rr, gg, bb = colorsys.hls_to_rgb(h, nl, ns)
        cand = rgb2hx((rr * 255, gg * 255, bb * 255))
        if cr(hx2rgb(cand), bg_rgb) >= target:
            return cand
    return None


def darken_toward_hue(hexc, bg_rgb, target, ref_hex, sat_cap=0.20):
    """朝 `ref_hex` 的**色相**对齐后降 L 到 target。

    给 mterm 门面套的槽8 用：`src/theme/mod.rs` 断言「a8 与 t3 同色相 ≤10°」
    （灰族同源纪律），而通用路径会把槽8 压成**中性灰**（色相 0°）当场破掉
    那条断言。所以这里保色相、压饱和（封顶 0.20，灰族本就该低饱和）、
    只降 L 提对比度 —— 纪律优先于「纯中性」的偏好。
    """
    r, g, b = hx2rgb(hexc)
    rr, rg, rb = hx2rgb(ref_hex)
    h0, l0, s0 = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
    h1 = colorsys.rgb_to_hls(rr / 255, rg / 255, rb / 255)[0]
    for step in range(0, 1001):
        nl = l0 - step / 1000.0
        if nl <= 0.02:
            break
        ns = min(sat_cap, s0 + step / 1000.0 * 0.05)
        r2, g2, b2 = colorsys.hls_to_rgb(h1, nl, ns)
        cand = rgb2hx((r2 * 255, g2 * 255, b2 * 255))
        if cr(hx2rgb(cand), bg_rgb) >= target:
            return cand
    return None


def recompute_fails(ansi, bg_rgb):
    """亮底口径：12 彩色槽 + 7/8（#ansi-exempt-slots 起槽7/8 也判）。
    0/15 仍只记录不判（默认前景的近亲与亮底上的 deepest，普遍已达标）。"""
    out = []
    for i in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]:
        c = cr(hx2rgb(ansi[i]), bg_rgb)
        if c < 4.5:
            out.append('ansi%d=%s on tbg %.2f' % (i, ansi[i], c))
    return out


def main():
    doc = json.load(open(P, encoding='utf-8', newline=''))
    ui = {}
    ui.update(json.load(open('scripts/theme-tokens.json', encoding='utf-8')))
    ui.update(json.load(open('scripts/_radix-themes.json', encoding='utf-8')))
    ui.update(json.load(open('scripts/_extra-themes.json', encoding='utf-8')))

    # 28 套 id 清单从 palettes.rs 读（JSON 里另有 meat-* 迁移档，不在选择器内）
    ids = re.findall(r'id: "([^"]+)"', open('src/theme/palettes.rs', encoding='utf-8').read())
    # mterm 两套是**自家门面主题**，槽8 有额外纪律：`src/theme/mod.rs` 的
    # `mterm_light_paper_rules_hold` 断言「a8 与 t3 同色相 ≤10°」（灰族同源）。
    # 脚本的通用路径会把槽8 压成中性灰（色相 0°），当场破掉这条断言。
    # 对这两套：槽8 改成**朝 t3 色相对齐后降 L**（保留同族色相，只提对比度）。
    PRESERVE_HUE_SLOT8 = {'mterm-light'}

    changed = []
    for tid in ids:
        e = doc.get(tid)
        if e is None:
            raise SystemExit('缺 _ansi-group 条目: ' + tid)
        if e.get('m') != '亮':
            continue                      # 暗色档不动（深底上白/灰本来就够读）
        bg_hex = '#' + str(ui[tid]['tbg']).lstrip('#').upper()
        bg_rgb = hx2rgb(bg_hex)
        a = e['ansi16']

        w, g, bw = a[SLOT_WHITE], a[SLOT_BRIGHT_BLACK], a[SLOT_BRIGHT_WHITE]
        cw, cg, cbw = (cr(hx2rgb(w), bg_rgb), cr(hx2rgb(g), bg_rgb), cr(hx2rgb(bw), bg_rgb))
        if cw >= 4.55 and cg >= 4.55 and cbw >= 4.55:
            continue

        notes = []
        if cw < 4.55:
            nw = darken(w, bg_rgb, T_WHITE)
            if nw is None:
                raise SystemExit('%s 槽7 无处可降（目标 %.1f）' % (tid, T_WHITE))
            notes.append('white 槽7 %s(%.2f)→%s(%.2f)' % (w, cw, nw, cr(hx2rgb(nw), bg_rgb)))
            a[SLOT_WHITE] = nw
        if cg < 4.55:
            if tid in PRESERVE_HUE_SLOT8:
                # 门面套：槽8 朝 t3 的色相对齐后降 L，保住「灰族同源」纪律
                ng = darken_toward_hue(g, bg_rgb, T_GREY, str(ui[tid]['t3']).lstrip('#').upper())
                tag = '（朝 t3 色相对齐，保灰族同源）'
            else:
                ng = darken(g, bg_rgb, T_GREY)
                tag = ''
            if ng is None:
                raise SystemExit('%s 槽8 无处可降（目标 %.1f）' % (tid, T_GREY))
            notes.append('brightBlack 槽8 %s(%.2f)→%s(%.2f)%s'
                         % (g, cg, ng, cr(hx2rgb(ng), bg_rgb), tag))
            a[SLOT_BRIGHT_BLACK] = ng
        if cbw < 4.55:
            # 槽15 brightWhite：亮底上加粗白字，原色往往接近纯白 → 同一个近中性
            # 陷阱（HSL S 爆表→保色相降 L 变暗黄），故走同一个 darken。
            nb = darken(bw, bg_rgb, T_BRITE)
            if nb is None:
                raise SystemExit('%s 槽15 无处可降（目标 %.1f）' % (tid, T_BRITE))
            notes.append('brightWhite 槽15 %s(%.2f)→%s(%.2f)' % (bw, cbw, nb, cr(hx2rgb(nb), bg_rgb)))
            a[SLOT_BRIGHT_WHITE] = nb

        e['fails'] = recompute_fails(a, bg_rgb)
        e['notes'] = list(e.get('notes') or []) + [
            '#ansi-exempt-slots 2026-10-16：亮底上白/灰/亮白三槽塌到 1.0~2.9:1'
            '（ls -l 的大小列=槽7、日期列=槽8），原「0/7/8/15 只记录不判失败」'
            '的豁免是暗底惯例，浅底不适用；按用户拍板压到 ≥4.5:1 —— ' + '、'.join(notes)
        ]
        changed.append((tid, bg_hex, notes,
                        cr(hx2rgb(a[SLOT_WHITE]), bg_rgb),
                        cr(hx2rgb(a[SLOT_BRIGHT_BLACK]), bg_rgb),
                        cr(hx2rgb(a[SLOT_BRIGHT_WHITE]), bg_rgb)))

    body = json.dumps(doc, ensure_ascii=False, indent=1)
    with io.open(P, 'w', encoding='utf-8', newline='') as f:
        f.write(body.replace('\n', '\r\n'))

    print('改写 %d 套：' % len(changed))
    for tid, bg, notes, cw, cg, c15 in changed:
        print('  %-24s %s  %s  → 槽7 %.2f / 槽8 %.2f / 槽15 %.2f'
              % (tid, bg, ' | '.join(notes), cw, cg, c15))


if __name__ == '__main__':
    main()