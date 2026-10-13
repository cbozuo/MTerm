#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""按锚点行把原型中的手写近似 SVG 替换为 assets/icons/*.svg 真实内容。
每个锚点必须命中且仅命中 1 行；替换该行第一个 <svg>…</svg>。"""
import re, sys

HTML = 'docs/design/panel-unified-hifi-2026-10-14.html'
ICONS = 'assets/icons'

def inner(name):
    txt = open(f'{ICONS}/{name}.svg', encoding='utf-8').read()
    m = re.search(r'<svg[^>]*>(.*?)</svg>', txt, re.S)
    return m.group(1).strip()

WRAP = ('<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" '
        'stroke-linecap="round" stroke-linejoin="round">%s</svg>')

# (锚点子串, 真实图标名, 排除子串或 None)
RULES = [
    ('title="主题速选"',        'moon',                 None),
    ('title="壁纸"',            'wallpaper',            None),
    ('title="传输管理"',        'swap-vert',            None),
    ('title="设置"',            'settings',             None),
    ('title="最小化"',          'remove',               None),
    ('title="最大化"',          'maximize',             None),
    ('title="关闭"',            'close',                None),
    ('data-sess="db-prod-01"',  'host-server',          None),
    ('title="全屏"',            'fullscreen-enter',     None),
    ('id="leftFoldBtn"',        'chevron-left',         None),
    ('id="rightFoldBtn"',       'chevron-right',        None),
    ('>www</div>',              'expand-more',          None),
    ('title="连接"',            'play',                 None),
    ('title="编辑"',            'edit',                 None),
    ('data-sess="gateway-01"',  'display',              None),
    ('title="导入 SSH config"', 'input',                None),
    ('title="新建会话"',        'new-session',          None),
    ('title="传输到其他标签"',  'move-to-group',        None),
    ('title="列设置"',          'tune',                 None),
    ('title="上传"',            'upload',               None),
    ('title="下载选中"',        'download',             None),
    ('title="新建文件夹"',      'create-new-folder',    None),
    ('title="删除选中"',        'delete',               None),
    ('title="目录树显隐"',      'sidebar-left-collapse',None),
    ('banner.jpg',              'file',                 None),
    ('data-cmd="git pull',      'play',                 None),
    ('data-cmd="docker compose','rendering',            None),
    ('data-cmd="tail -f',       'log-file',             None),
    ('data-cmd="systemctl status','connectivity-test',  None),
    ('data-cmd="df -h',         'dashboard',            None),
    ('data-cmd="journalctl',    'command-history',      None),
    ('title="复制主机地址"',    'copy',                 None),
    ('m7 14 4-4 3 3 5-5',       'connectivity-test',    None),   # 网络卡头
    ('id="txRate"',             'arrow-up',             None),
    ('id="rxRate"',             'arrow-down',           None),
    ('ellipse cx="12" cy="5"',  'host-server',          None),   # 磁盘卡头
    ('M20 6 9 17l-5-5',        'table-rows',           'innerHTML'),  # Top5 卡头（排除 toast）
    ('id="btnSys"',             'info',                 None),
    ('id="btnProc"',            'table-rows',           None),
    ('data-a="conn"',           'play',                 None),
    ('data-a="dup"',            'copy',                 None),
    ('data-a="del"',            'delete',               None),
    ('data-rpane="mon"',        'dashboard',            None),   # 右台动态 tab
    ('data-rpane="proc"',       'table-rows',           None),
    ('id="pvPop"',              'open-in-full',         None),
    ('在独立窗口打开',           'open-in-full',         None),
    ('Ubuntu 24.04 LTS',        'display',              None),   # 系统信息卡头
]

def main():
    lines = open(HTML, encoding='utf-8').read().split('\n')
    svg_re = re.compile(r'<svg[^>]*>.*?</svg>', re.S)
    cache = {}
    fails, done = [], 0
    for anchor, icon, excl in RULES:
        hits = [i for i, ln in enumerate(lines)
                if anchor in ln and (excl is None or excl not in ln)]
        if len(hits) != 1:
            fails.append(f'ANCHOR {anchor!r}: {len(hits)} hits (expect 1)')
            continue
        i = hits[0]
        if icon not in cache:
            cache[icon] = WRAP % inner(icon)
        new_ln, n = svg_re.subn(cache[icon], lines[i], count=1)
        if n != 1:
            fails.append(f'LINE {i+1} ({anchor}): no <svg> found')
            continue
        lines[i] = new_ln
        done += 1
    open(HTML, 'w', encoding='utf-8').write('\n'.join(lines))
    print(f'replaced={done}/{len(RULES)}')
    for f in fails:
        print('FAIL:', f)
    sys.exit(1 if fails else 0)

if __name__ == '__main__':
    main()
