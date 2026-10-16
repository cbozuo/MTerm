# -*- coding: utf-8 -*-
"""#ansi-yellow-fix 2026-10-16 —— 把**亮色**主题的 ANSI yellow / brightYellow
（槽 3 / 槽 11）压到 WCAG 4.5:1 达标线。

验收线见 _ansi-group.json 的 _meta.ansi_check：12 彩色槽 ≥ 4.5:1。
原先 9 套亮色主题的槽 3 只有 2.44~4.50，槽 11 最低 2.05（ayu-light），
在终端里黄字几乎不可见 —— 本脚本保 H/S 降 L（每步补 0.12% 饱和防发灰）
把它们压到达标线以上，槽 11 一律压到比槽 3 更深（亮底 bright=更深同色相
的既有纪律，见 mterm-light）。

真源 = scripts/_ansi-group.json；改完跑 `node scripts/gen_theme_rust.js`
重生成 src/theme/palettes.rs。只改槽 3 / 11，其余槽的 fails 原样保留。
"""
import colorsys
import io
import json
import re

P = 'scripts/_ansi-group.json'
T = 4.6     # 槽 3 目标（4.5 达标线上留 0.1 余量）
TB = 5.2    # 槽 11 目标（须明显深于槽 3）
SLOT_Y, SLOT_BRIGHT_Y = 3, 11
COLOR_SLOTS = [1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14]   # 与 gen_ansi_group.js 一致


def lin(c):
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def lum(rgb):
    return 0.2126 * lin(rgb[0]) + 0.7152 * lin(rgb[1]) + 0.0722 * lin(rgb[2])


def cr(a, b):
    la, lb = lum(a), lum(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def hx2rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def rgb2hx(t):
    return '#%02X%02X%02X' % tuple(max(0, min(255, int(round(x)))) for x in t)


def darken(hexc, bg_rgb, target, sat_boost=0.12):
    """保 H/S 降 L 到对比度达 target；每步补一点 S 免得降得发灰。降不动就原样返回。"""
    r, g, b = hx2rgb(hexc)
    h, l, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
    for step in range(0, 1001):
        nl = l - step / 1000.0
        if nl <= 0.02:
            break
        ns = min(1.0, s + step / 1000.0 * sat_boost)
        rr, gg, bb = colorsys.hls_to_rgb(h, nl, ns)
        cand = rgb2hx((rr * 255, gg * 255, bb * 255))
        if cr(hx2rgb(cand), bg_rgb) >= target:
            return cand
    return hexc


def recompute_fails(ansi, bg_rgb):
    out = []
    for i in COLOR_SLOTS:
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

    # 只处理选择器里的 28 套（id 清单从 palettes.rs 读），迁移档 meat-* 不在其中
    ids = re.findall(r'id: "([^"]+)"', open('src/theme/palettes.rs', encoding='utf-8').read())

    changed = []
    for tid in ids:
        e = doc.get(tid)
        if e is None:
            raise SystemExit('缺 _ansi-group 条目: ' + tid)
        if e.get('m') != '亮':
            continue                      # 暗色档黄字本就在亮字上够读，不动
        bg_hex = '#' + str(ui[tid]['tbg']).lstrip('#').upper()
        bg_rgb = hx2rgb(bg_hex)
        ansi = e['ansi16']

        y, bY = ansi[SLOT_Y], ansi[SLOT_BRIGHT_Y]
        cy = cr(hx2rgb(y), bg_rgb)
        cby = cr(hx2rgb(bY), bg_rgb)
        if cy >= 4.55 and cby >= 4.55:
            continue                      # 两档都达标，不动

        ny = darken(y, bg_rgb, T) if cy < 4.55 else y
        # 槽 11：未达标、或不比槽 3 深 → 一律压深（亮底 bright 档纪律）
        nby = darken(bY, bg_rgb, TB) if (cby < 4.55 or lum(hx2rgb(bY)) >= lum(hx2rgb(y))) else bY
        ansi[SLOT_Y], ansi[SLOT_BRIGHT_Y] = ny, nby

        old_fails = list(e.get('fails') or [])
        e['fails'] = recompute_fails(ansi, bg_rgb)
        ncy = cr(hx2rgb(ny), bg_rgb)
        ncby = cr(hx2rgb(nby), bg_rgb)
        e['notes'] = list(e.get('notes') or []) + [
            '#ansi-yellow-fix 2026-10-16：yellow 槽3 %s(%.2f)→%s(%.2f)、'
            'brightYellow 槽11 %s(%.2f)→%s(%.2f)，保 H/S 降 L 至 ≥4.5:1'
            '（亮底上黄字原先仅 2.0~3.5，终端里几乎不可见）'
            % (y, cy, ny, ncy, bY, cby, nby, ncby)
        ]
        changed.append((tid, y, cy, ny, ncy, bY, cby, nby, ncby, len(old_fails), len(e['fails'])))

    body = json.dumps(doc, ensure_ascii=False, indent=1)
    with io.open(P, 'w', encoding='utf-8', newline='') as f:
        f.write(body.replace('\n', '\r\n'))

    print('改写 %d 套：' % len(changed))
    for t, y, cy, ny, ncy, bY, cby, nby, ncby, of, nf in changed:
        print('  %-24s 槽3 %s(%.2f)→%s(%.2f)   槽11 %s(%.2f)→%s(%.2f)   fails %d→%d'
              % (t, y, cy, ny, ncy, bY, cby, nby, ncby, of, nf))


if __name__ == '__main__':
    main()