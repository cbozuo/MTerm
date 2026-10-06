# -*- coding: utf-8 -*-
"""重写 theme-wallpaper-split 的主题选择器：下拉框 + 悬停预览 + 三族分组。

三处改动：
① **选择器形态**：网格（33 个根本放不下）→ **下拉框 + 右侧固定预览**
② **新增「现代 UI 色板」族**（Radix Colors），与社区终端色系、IDEA 并列
③ 下拉内按族分组，让用户按"风格"找而不是逐个翻33 个
"""

import io

from merge_themes import build, ordered, GROUPS

P = '../docs/design/theme-wallpaper-split-2026-10-05.html'
ALL = build()
GROUPS_ORD = ordered(ALL)
ALL_IDS = [i for _g, _n, _d, ids in GROUPS_ORD for i in ids]

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

# ══════════════════════════════════════════════════════════════════
# ① CSS：新增下拉框 + 预览面板
# ══════════════════════════════════════════════════════════════════
NEW_CSS = '''
  /* ═══ 主题选择器：下拉框 + 悬停预览 ═══════════════════════════
     33 个变体放不进网格 —— 一屏最多 6 列 × 4 行 = 24 个，
     而且网格没有"分组"的位置，33个平铺会让用户无从下手。
     判据：**选项数超过一屏能舒适显示的量，就换列表**。
     33 个 → 列表（可滚动 + 可分组）；≤12 个 → 网格仍然更好（省一次点击）。 */

  .tpk{display:flex;gap:11px;align-items:stretch}

  /* 下拉：触发器 */
  .tpk-btn{flex:0 0 258px;display:flex;flex-direction:column;gap:0;
    background:var(--doc-elev);border:1px solid var(--doc-line2);
    border-radius:8px;overflow:visible;cursor:pointer;position:relative}
  .tpk-btn:hover{border-color:var(--doc-t3)}
  .tpk-btn.open{border-color:var(--doc-ac)}
  .tpk-cur{display:flex;align-items:center;gap:9px;padding:8px 10px}
  .tpk-thumb{width:44px;height:30px;border-radius:5px;overflow:hidden;
    flex:0 0 44px;position:relative;border:1px solid rgba(128,128,128,.28)}
  .tpk-txt{flex:1;min-width:0;text-align:left}
  .tpk-n1{font-size:12px;color:var(--doc-t1);line-height:1.35}
  .tpk-n2{font-size:10px;color:var(--doc-t3);line-height:1.35;
    white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
  .tpk-caret{width:9px;height:9px;flex:0 0 9px;color:var(--doc-t3);
    transition:transform .14s}
  .tpk-btn.open .tpk-caret{transform:rotate(180deg)}

  /* 下拉：浮层 */
  .tpk-pop{position:absolute;left:0;right:0;top:calc(100% + 5px);
    background:var(--doc-panel);border:1px solid var(--doc-line2);
    border-radius:8px;z-index:30;box-shadow:0 16px 40px rgba(0,0,0,.55);
    display:none;overflow:hidden}
  .tpk-btn.open .tpk-pop{display:block}
  .tpk-search{padding:6px;border-bottom:1px solid var(--doc-line)}
  .tpk-si{width:100%;background:var(--doc-alt);border:1px solid var(--doc-line);
    border-radius:6px;padding:5px 8px;font-size:11px;color:var(--doc-t1);
    font-family:var(--sans)}
  .tpk-list{max-height:296px;overflow-y:auto;padding:4px 0}
  .tpk-grp{padding:8px 10px 3px;font-size:9.5px;letter-spacing:.5px;
    color:var(--doc-t3);display:flex;align-items:center;gap:6px}
  .tpk-grp::after{content:"";flex:1;height:1px;background:var(--doc-line)}
  .tpk-row{display:flex;align-items:center;gap:9px;padding:5px 10px;
    cursor:pointer;position:relative}
  .tpk-row:hover{background:var(--doc-hov)}
  .tpk-row.on{background:var(--doc-ac);color:#fff}
  .tpk-row.on .tpk-r2,.tpk-row.on .tpk-nm{color:rgba(255,255,255,.82)}
  .tpk-th{width:34px;height:22px;border-radius:4px;overflow:hidden;
    flex:0 0 34px;position:relative;border:1px solid rgba(128,128,128,.24)}
  .tpk-nm{flex:1;min-width:0;font-size:11.5px;color:var(--doc-t1);
    white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
  .tpk-r2{font-size:9.5px;color:var(--doc-t3);flex:0 0 auto}
  .tpk-empty{padding:14px 10px;font-size:11px;color:var(--doc-t3);
    text-align:center}

  /* 悬停预览：右侧固定面板（非浮出）——
     ⚠️ 不用浮出卡：下拉宽258 + 浮出卡300 = 558，
     而设置面板的内容区只有 ~560 —— 放不下就会盖住右边内容。
     判据：**浮出层与固定面板二选一时，看空间**：
     空间够用固定面板（不遮挡、可长时间停留），不够才用浮出卡。*/
  .tpk-prev{flex:1;min-width:0;background:var(--doc-bg);
    border:1px solid var(--doc-line);border-radius:8px;padding:10px;
    display:flex;flex-direction:column;gap:8px}
  .tpk-pv-hd{display:flex;align-items:baseline;gap:7px}
  .tpk-pv-n1{font-size:12.5px;color:var(--doc-t1);font-weight:600}
  .tpk-pv-n2{font-size:10px;color:var(--doc-t3)}
  .tpk-pv-star{margin-left:auto;font-size:9.5px;color:var(--doc-wr);
    font-weight:700}
  .tpk-pv-desc{font-size:11px;color:var(--doc-t2);line-height:1.7;
    min-height:36px}
  .tpk-pv-lic{font-size:10px;color:var(--doc-t3);line-height:1.6}
  .tpk-pv-bar{display:flex;gap:3px;padding:2px}
  .tpk-pv-bar i{width:22px;height:11px;border-radius:2px;display:block}
  .tpk-pv-bar i:first-child{width:42px}

  /* 预览里的迷你界面（与真实界面同构造） */
  .pv{height:112px;border-radius:6px;overflow:hidden;
    display:flex;flex-direction:column;flex:0 0 112px}
  .pv-bar{height:13px;display:flex;align-items:center;gap:3px;
    padding:0 5px;flex:0 0 13px}
  .pv-bar i{width:5px;height:5px;border-radius:50%;display:block;flex:0 0 5px}
  .pv-tab{height:20px;display:flex;align-items:stretch;flex:0 0 20px}
  .pv-tab .pt{width:88px;position:relative;display:block}
  .pv-tab .pt.on{box-shadow:inset 0 2px 0 currentColor}
  .pt .ltr{position:absolute;left:0;top:5px;width:3px;height:8px;
    border-radius:1px;display:block}
  .pt .lb{position:absolute;left:6px;top:5px;width:6px;height:8px;
    font-size:8px;line-height:8px;font-weight:800;display:block}
  .pt .tx{position:absolute;left:15px;top:5px;right:5px;height:8px;
    font-size:8px;line-height:8px;overflow:hidden;white-space:nowrap;
    display:block}
  .pv-tab .add{width:18px;display:flex;align-items:center;justify-content:center;
    font-size:9px;flex:0 0 18px}
  .pv-side{width:26%;display:flex;flex-direction:column;gap:4px;
    padding:5px 4px;flex:0 0 26%}
  .pv-side i{height:3px;border-radius:2px;display:block}
  .pv-side i.on{height:5px}
  .pv-main{flex:1;min-width:0;padding:4px 5px;display:flex;
    flex-direction:column;gap:3px}
  .pv-main .ln{height:4px;border-radius:2px;display:block}
  .pv-main .mk{height:11px;border-radius:3px;display:block;margin-top:2px}

  /* 槽位表（预览下方） */
  .tpk-slots{display:grid;grid-template-columns:repeat(2,1fr);gap:2px 8px}
  .tpk-sl{display:flex;align-items:center;gap:6px;font-size:9.5px;
    color:var(--doc-t3)}
  .tpk-sl b{font-family:var(--mono);font-size:9px;font-weight:400;
    color:var(--doc-t2);min-width:52px}
  .tpk-sl .sq{width:11px;height:11px;border-radius:3px;
    box-shadow:inset 0 0 0 1px rgba(128,128,128,.28);flex:0 0 11px}
'''


def vars_css():
    L = []
    for gid, gname, _d in GROUPS:
        ids = [i for g, _n, _dd, x in GROUPS_ORD if g == gid for i in x]
        L.append('  /* ═══ %s ═══ */' % gname)
        for key in ids:
            t = ALL[key]
            star = ('%d★' % t['star']) if t.get('star') else 'JetBrains'
            L.append('  /* %s · %s —— %s */' % (t['f'], t['m'], star))
            L.append('  [data-theme="%s"]{' % key)
            for var, fld in SLOTS:
                L.append('    %-17s %s;' % (var + ':', t[fld]))
            L.append('    /* 频道 4 槽：固定色相，跨全部主题共用 */')
            for i, c in enumerate(t['ch']):
                L.append('    --ch-%-16s %s;' % ('abcd'[i] + ':', c))
            L.append('  }')
            L.append('')
    return '\n'.join(L)


def js_data():
    import json
    small = {}
    for k in ALL_IDS:
        t = ALL[k]
        small[k] = dict(
            f=t['f'], en=t['en'], m=t['m'], kind=t['kind'],
            group=t['group'], star=t.get('star'), lic=t.get('lic'),
            repo=t.get('repo'), desc=t.get('desc', ''),
            root=t['root'], panel=t['panel'], palt=t['palt'],
            elev=t['elev'], tab=t['tab'], line=t['line'],
            t1=t['t1'], t2=t['t2'], t3=t['t3'],
            tbg=t['tbg'], tfg=t['tfg'],
            ac=t['ac'], ac2=t['ac2'], ok=t['ok'], wr=t['wr'],
            dg=t['dg'], ch=t['ch'])
    out = '  /* %d 个变体的色值（真源 scripts/theme-tokens.json + _radix-themes.json）*/\n' % len(ALL_IDS)
    out += '  var THEME_IDS = ' + json.dumps(ALL_IDS, ensure_ascii=False) + ';\n'
    out += '  var GROUPS = ' + json.dumps(
        [[g, n, d, ids] for g, n, d, ids in GROUPS_ORD],
        ensure_ascii=False).replace('], [', '],\n   [') + ';\n'
    out += '  var PAL = ' + json.dumps(small, ensure_ascii=False,
                                     indent=1).replace('\n', '\n  ') + ';\n'
    return out


if __name__ == '__main__':
    print('%d 个变体，3 族' % len(ALL_IDS))
    print('vars_css %d 字节 · js_data %d 字节'
          % (len(vars_css()), len(js_data())))
    for g, n, d, ids in GROUPS_ORD:
        print('  %-9s %-14s %d 个' % (g, n, len(ids)))
