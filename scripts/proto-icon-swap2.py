#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""补替换：锚点与 <svg> 不在同一行的 11 处（svg 在 anchor±1 行）。
每条规则指定 (锚点, 图标名, 偏移, 旧inner特征或None表示该行第一个svg)。"""
import re, sys

HTML = 'docs/design/panel-unified-hifi-2026-10-14.html'
ICONS = 'assets/icons'

def inner(name):
    txt = open(f'{ICONS}/{name}.svg', encoding='utf-8').read()
    return re.search(r'<svg[^>]*>(.*?)</svg>', txt, re.S).group(1).strip()

WRAP = ('<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" '
        'stroke-linecap="round" stroke-linejoin="round">%s</svg>')
SVG = re.compile(r'<svg[^>]*>.*?</svg>', re.S)

# (锚点, 图标, 行偏移, 旧inner特征[None=首个svg], 排除)
RULES = [
    ('data-sess="db-prod-01"',   'host-server',          +1, None, None),
    ('data-sess="gateway-01"',   'display',              +1, None, None),
    ('data-cmd="git pull',       'play',                 +1, None, None),
    ('data-cmd="docker compose', 'rendering',            +1, None, None),
    ('data-cmd="tail -f',        'log-file',             +1, None, None),
    ('data-cmd="systemctl status','connectivity-test',   +1, None, None),
    ('data-cmd="df -h',          'dashboard',            +1, None, None),
    ('data-cmd="journalctl',     'command-history',      +1, None, None),
    ('data-rpane="mon"',         'dashboard',            +1, None, None),
    ('data-rpane="proc"',        'table-rows',           +1, None, None),
    ('Ubuntu 24.04 LTS',         'display',              -1, 'rect x="2" y="4"', None),
]

def main():
    src = open(HTML, encoding='utf-8').read()
    lines = src.split('\n')
    fails, done = [], 0
    for anchor, icon, off, feat, excl in RULES:
        hits = [i for i, ln in enumerate(lines)
                if anchor in ln and (excl is None or excl not in ln)]
        if len(hits) != 1:
            fails.append(f'ANCHOR {anchor!r}: {len(hits)} hits')
            continue
        j = hits[0] + off
        if not (0 <= j < len(lines)):
            fails.append(f'ANCHOR {anchor!r}: offset out of range')
            continue
        new = WRAP % inner(icon)
        if feat is None:
            lines[j], n = SVG.subn(new, lines[j], count=1)
        else:
            # 只替换 inner 含特征的 svg
            before = lines[j]
            lines[j] = SVG.sub(lambda m: new if feat in m.group(0) else m.group(0), lines[j])
            if lines[j] == before or feat in lines[j]:
                fails.append(f'LINE {j+1} ({anchor}): feature svg not replaced')
                continue
            n = 1
        if n != 1:
            fails.append(f'LINE {j+1} ({anchor}): no <svg> replaced')
            continue
        done += 1
    src = '\n'.join(lines)
    # 右台动态 tab 的 × 字符清空（关闭键改双斜杠图形）
    src, n1 = re.subn(r'(<span class="x" data-close="(?:files|proc)">)×(</span>)', r'\1\2', src)
    open(HTML, 'w', encoding='utf-8').write(src)
    print(f'replaced={done}/{len(RULES)}  x-cleared={n1}')
    for f in fails:
        print('FAIL:', f)
    sys.exit(1 if fails else 0)

if __name__ == '__main__':
    main()
