# -*- coding: utf-8 -*-
"""为 theme-wallpaper-split 高保真生成变量层 + JS 数据。

被高保真直接内联，不落临时文件。
色值真源 = scripts/theme_palettes.py → theme-tokens.json
"""

import io
import json

T = json.load(io.open('theme-tokens.json', encoding='utf-8'))

# 排序：社区（按 star 降序）→ IDEA
IDS = ['dracula-dark', 'dracula-light',
       'catppuccin-dark', 'catppuccin-light',
       'solarized-dark', 'solarized-light',
       'gruvbox-dark', 'gruvbox-light',
       'tokyonight-dark', 'tokyonight-light',
       'nord-dark', 'nord-light',
       'everforest-dark', 'everforest-light',
       'onedark-dark', 'onedark-light',
       'rosepine-dark', 'rosepine-light',
       'idea-dark', 'idea-light', 'darcula']

SLOTS = [('--bg-root', 'root'), ('--bg-panel', 'panel'),
         ('--bg-panel-alt', 'palt'), ('--bg-elev', 'elev'),
         ('--bg-hover', 'hov'), ('--bg-active', 'act'),
         ('--bg-tab', 'tab'), ('--line', 'line'),
         ('--line-strong', 'lstr'),
         ('--t1', 't1'), ('--t2', 't2'), ('--t3', 't3'),
         ('--term-bg', 'tbg'), ('--term-fg', 'tfg'),
         ('--accent', 'ac'), ('--accent-2', 'ac2'),
         ('--ok', 'ok'), ('--warn', 'wr'),
         ('--danger', 'dg'), ('--danger-strong', 'dgS')]


def vars_css():
    L = []
    L.append('    /* ═══════ 派生槽位：全部主题共用同一份定义 ═══════')
    L.append('       由语义色现场算，不独立取值 —— 换主题时自动跟着变。 */')
    L.append('    --accent-soft:color-mix(in srgb, var(--accent) 15%, transparent);')
    L.append('    --accent-soft-hi:color-mix(in srgb, var(--accent) 24%, transparent);')
    L.append('    --accent-chip:color-mix(in srgb, var(--accent) 22%, transparent);')
    L.append('    --accent-line:color-mix(in srgb, var(--accent) 34%, transparent);')
    L.append('    --danger-soft:color-mix(in srgb, var(--danger) 14%, transparent);')
    L.append('    --danger-line:color-mix(in srgb, var(--danger) 45%, transparent);')
    L.append('    --warn-soft:color-mix(in srgb, var(--warn) 16%, transparent);')
    L.append('    --neutral-line:rgba(128,128,128,.3);')
    L.append('    --shadow:rgba(0,0,0,.55);')
    L.append('    --shadow-weak:rgba(0,0,0,.3);')
    L.append('  }')
    L.append('')
    for key in IDS:
        t = T[key]
        star = ('%d★' % t['star']) if t['star'] else 'JetBrains'
        L.append('  /* %s · %s —— %s */'
                 % (t['f'], t['m'], star))
        L.append('  [data-theme="%s"]{' % key)
        for var, fld in SLOTS:
            L.append('    %-17s %s;' % (var + ':', t[fld]))
        # 频道 4 槽：固定色相
        L.append('    /* 频道 4 槽：固定色相 210/165/38/345 度，'
                 '跨全部主题共用 */')
        for i, c in enumerate(t['ch']):
            L.append('    --ch-%-16s %s;' % ('abcd'[i] + ':', c))
        L.append('  }')
        L.append('')
    return '\n'.join(L)


def js_data():
    small = {}
    for key in IDS:
        t = T[key]
        small[key] = dict(
            f=t['f'], en=t['en'], m=t['m'], kind=t['kind'],
            star=t['star'], lic=t['lic'], repo=t['repo'],
            root=t['root'], panel=t['panel'], palt=t['palt'],
            elev=t['elev'], tab=t['tab'], line=t['line'],
            t1=t['t1'], t2=t['t2'], t3=t['t3'],
            tbg=t['tbg'], tfg=t['tfg'],
            ac=t['ac'], ac2=t['ac2'], ok=t['ok'], wr=t['wr'],
            dg=t['dg'], ch=t['ch'])
    out = '  /* 21 个变体的色值（真源 scripts/theme-tokens.json，勿手改） */\n'
    out += '  var THEME_IDS = ' + json.dumps(IDS, ensure_ascii=False) + ';\n'
    out += '  var PAL = ' + json.dumps(small, ensure_ascii=False,
                                     indent=1).replace('\n', '\n  ') + ';\n'
    return out


if __name__ == '__main__':
    print('%d 个变体' % len(IDS))
    print('vars_css %d 字节, js_data %d 字节'
          % (len(vars_css()), len(js_data())))
