# -*- coding: utf-8 -*-
"""#ansi-dark-grey 2026-10-19 —— 把**暗色**主题的 ANSI 槽 8 (brightBlack)
提亮到 ≥4.5:1。

起因（用户反馈）：「黄色的颜色在日期上，任何主题上面都不清晰」。取证结论：
终端里 `ls -l` / eza 的**日期列用的是槽 8**（同一处取证见 CHANGELOG
「亮色主题的白色也不可读」：那句写着「整片列表的日期列是槽 8 brightBlack」，
当时 2.23:1）。亮色 14 套已在 #ansi-exempt-slots 里压到 ≥4.7，但**暗色 14 套
按终端惯例原样保留** —— 实测对各自 tbg：mterm 3.66 / onedark 2.67 /
graphite 2.06 / nord 1.69 / catppuccin 2.46 / dracula 3.03 / rosepine 3.42 /
everforest 2.79 / kanagawa 3.33 / gruvbox 2.09 / ayu 2.78 / flexoki **1.28** /
solarized **1.00（色值等于 tbg，完全不可见）** / darcula 2.21 —— 全部不达标。

修法（与亮色那轮镜像）：
  - 只动暗色档、只动槽 8；**保色相、提亮度**（暗底是往上提，不是压深）。
  - 槽 8 是"辅助信息弱化档"，不是彩色档：饱和**封顶 0.18**，否则
    solarized 的 #002B36（H=192°、S=100%）提亮后会变成刺眼的青绿。
  - 目标 4.6:1（比验收线 4.5 留 0.1 余量）。
  - 纪律：提亮后仍须**暗于槽 7**（槽 8 永远比正文弱化档更弱），脚本内断言。

真源是 scripts/_ansi-group.json，改完跑 node scripts/gen_theme_rust.js 重生成
src/theme/palettes.rs。
"""
import colorsys
import json

P = 'scripts/_ansi-group.json'
SLOT = 8
TARGET = 4.6
SAT_CAP = 0.18


def lin(c):
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def lum(t):
    return 0.2126 * lin(t[0]) + 0.7152 * lin(t[1]) + 0.0722 * lin(t[2])


def cr(a, b):
    la, lb = lum(a), lum(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def hx2rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def rgb2hx(t):
    return '#%02X%02X%02X' % tuple(max(0, min(255, int(round(x)))) for x in t)


def lift(hexc, bg_rgb, target):
    """保色相、饱和封顶，提亮度到对比度达 target。"""
    r, g, b = hx2rgb(hexc)
    h, l, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
    s = min(s, SAT_CAP)
    for step in range(0, 1001):
        nl = l + step / 1000.0
        if nl >= 0.98:
            break
        rr, gg, bb = colorsys.hls_to_rgb(h, nl, s)
        cand = rgb2hx((rr * 255, gg * 255, bb * 255))
        if cr(hx2rgb(cand), bg_rgb) >= target:
            return cand
    return None


def load_ui_tokens():
    """tbg 不在 _ansi-group.json 里，来自三份 UI 槽表（与 gen_theme_rust.js 同源）。"""
    ui = {}
    for f in ('theme-tokens.json', '_radix-themes.json', '_extra-themes.json'):
        d = json.load(open('scripts/' + f, encoding='utf-8'))
        for k, v in d.items():
            if k.startswith('_'):
                continue
            ui[k] = v
    return ui


def main():
    d = json.load(open(P, encoding='utf-8'))
    ui = load_ui_tokens()
    rows = []
    for tid, t in d.items():
        if tid.startswith('_') or t.get('m') != '暗':
            continue
        if tid not in ui:
            # meat-dark/light 是迁移保留档，不进 28 套选择器（gen_theme_rust.js 的 ORDER）
            print(f'跳过 {tid}（不在选择器内）')
            continue
        ansi = t['ansi16']
        bg = hx2rgb(ui[tid]['tbg'])
        old = ansi[SLOT]
        c_old = cr(hx2rgb(old), bg)
        if c_old >= 4.5:
            rows.append((tid, old, old, c_old, c_old, '已达标'))
            continue
        new = lift(old, bg, TARGET)
        assert new, f'{tid}: 提不上去'
        c_new = cr(hx2rgb(new), bg)
        # 纪律：槽 8 仍须弱于槽 7
        c7 = cr(hx2rgb(ansi[7]), bg)
        assert c_new < c7, f'{tid}: 槽8 {c_new:.2f} 不应亮过槽7 {c7:.2f}'
        ansi[SLOT] = new
        t.setdefault('notes', []).append(
            f'#ansi-dark-grey：ansi8 {old} → {new}（对 tbg {c_new:.2f}:1；槽8=日期列，原 {c_old:.2f}:1 读不清）'
        )
        rows.append((tid, old, new, c_old, c_new, '提亮'))

    d['_meta']['ansi_check'] = (
        '12 彩色槽(1-6,9-14) ≥ 4.5:1 为达标线；亮色档 7/8/15 与**暗色档 8** 同样按 ≥4.5:1 判'
        '（#ansi-exempt-slots / #ansi-dark-grey）；暗色档 0/7/15 按终端惯例只记录不判失败'
    )
    d['_meta']['dark_grey_note'] = (
        '#ansi-dark-grey 2026-10-19：暗色 14 套的槽8(brightBlack) 原本 1.00~3.66:1'
        '（solarized-dark 的 #002B36 与 tbg 同色，日期列完全不可见），'
        '统一保色相、饱和封顶 0.18、提亮到 ≥4.6；只动槽 8，其余槽逐值不变。脚本 scripts/_audit/fix_dark_grey_slot.py'
    )

    json.dump(d, open(P, 'w', encoding='utf-8'), ensure_ascii=False, indent=1)

    print(f'{"主题":22s} {"旧":9s} {"新":9s} {"旧比":>5s} {"新比":>5s}')
    for tid, old, new, c0, c1, tag in rows:
        print(f'{tid:22s} {old:9s} {new:9s} {c0:5.2f} {c1:5.2f}  {tag}')
    print(f'\n共处理 {len(rows)} 套暗色档')


if __name__ == '__main__':
    main()
