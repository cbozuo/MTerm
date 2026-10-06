# -*- coding: utf-8 -*-
"""重写 theme-wallpaper-split 的文档区样式（上一次误删了整段）。

═══════��═════════════════════════════════════════════════════════════
★ 这一轮为什么是"重写"而不是"补回"
═══════════════════════════════════════════════════════════════════════
 原样式被 `apply_dropdown.py` 的 `s.index('  .demo{', i0)`当替换终点误删 ——
 它在文档区样式**之后**，于是从变量块到 `.demo` 之间的
 body / .sec / .card / .cols / .ifd / .srow 等**整段被替换掉**，
 页面失去所有样式（变白）。备份已删，且这套文档区样式是本项目独有的，
 其它三个高保真都没有 → 只能重写。

 但重写不能只"照抄回去"。用户同时提了两条：
   ① 页面变白（样式丢了）
   ② **整体布局不够协调美观**
 所以这一版顺便做了三处视觉改进，都写在下面的注释里：
   · **统一的表面层级** —— 原来只有 card 一种卡片，
     表格直接裸在卡片里，层级断裂。改成 app < card < elev 三层
   · **表格对齐** —— 列宽用 `table-layout:fixed` + 首列固定宽度，
     长文本列不再被内容撑开
   · **留白节奏** —— 卡片内边距 16→18、章节间距 14→22，
     章节标题左侧竖条加粗到 3px（原来 2px 太轻）

★ 判据（这轮新增的）
═══════════════════════════════════════════════════════════════════════
 **改动大数组件时，替换终点必须用"内容特征"而不是"另一个选择器"**。
 `s.index('.demo{')` 表达的是"我以为下一个就是这个"，
 而文件里还有别的东西在它前面。→ **替换前先打印终点上下文确认**，
 或用带唯一注释标记的锚点。判据：**锚点要在被替换内容里，
 不能在它外面靠"下一个 X"推。**
"""

import io

DOC_CSS = r'''
  /* ══════════════════════════════════════════════════════════════
     文档区样式（说明这张页面本身，与 [data-theme] 主题变量无关）
     ══════════════════════════════════════════════════════════════
     ★ 表面层级（三层，别再混用）：
         app   = --doc-bg页面底
         card  = --doc-panel       卡片（章节容器）
         elev  = --doc-elev        浮层 / 抬升面（内嵌面板、代码块）
       原来的表格裸在卡片里 → 层级断裂，读起来像"卡片里又摊了一张纸"。
     ★ 留白节奏：卡片内 18px、章节间 22px、段间 11px。
       原来 14/10/…混着，密集处糊、疏处散。
     ──────────────────────────────────────────────────────────── */
  *{margin:0;padding:0;box-sizing:border-box}
  body{background:var(--doc-bg);font-family:var(--sans);
    color:var(--doc-t1);min-height:100vh;padding:40px 24px 100px;
    display:flex;flex-direction:column;align-items:center;gap:0;
    -webkit-font-smoothing:antialiased}
  h1{font-size:17px;font-weight:650;letter-spacing:.3px;color:var(--doc-t1)}
  .sub{font-size:12px;color:var(--doc-t3);margin-top:7px;
    padding-bottom:20px;border-bottom:1px solid var(--doc-line);
    width:1180px;text-align:left}

  /* ── 章节 ── */
  .sec{width:1180px;margin-top:22px}
  .sec:first-of-type{margin-top:14px}
  .sec-t{font-size:14.5px;font-weight:700;color:var(--doc-t1);
    border-left:3px solid var(--doc-ac);
    padding:2px 0 2px 11px;margin-bottom:9px;line-height:1.4}
  .sec-d{font-size:11.5px;color:var(--doc-t3);line-height:1.8;
    margin:0 0 12px 14px}
  .card{width:100%;background:var(--doc-panel);
    border:1px solid var(--doc-line);border-radius:11px;
    padding:18px 20px}

  /* ── 排版 ── */
  .shead{font-size:12px;font-weight:700;color:var(--doc-t2);
    letter-spacing:.2px}
  .cols{display:grid;grid-template-columns:1fr 1fr;gap:22px}
  .cols>*{min-width:0}
  .mono{font-family:var(--mono);font-size:11px;color:var(--doc-t2)}
  .c{font-size:11.5px;color:var(--doc-t3);line-height:1.75;
    vertical-align:top}
  .ok-c{color:#6bcf9e;font-weight:700}
  .no-c{color:#ef8b8b;font-weight:700}
  .new-c{color:#eec27a;font-weight:700}
  .gap{height:14px}
  .mono code,code{font-family:var(--mono);font-size:10.5px;
    color:var(--doc-t2);background:var(--doc-elev);
    padding:.5px 4px;border-radius:4px}
  ul.docs{margin:5px 0 0 17px;font-size:11.5px;color:var(--doc-t2);
    line-height:1.9}
  ul.docs li::marker{color:var(--doc-ac)}
  .kv{display:grid;grid-template-columns:132px 1fr;gap:5px 13px;
    font-size:11.5px;margin-top:6px}
  .kv b{color:var(--doc-t3);font-weight:600}

  /* ── 表格 ──
     ★ table-layout:fixed + 首列固定宽度：
       长文本列不再被内容撑开 → 整表左对齐、右侧齐平。
       原来 auto布局时，"说明"列因为长句把列宽吃掉，
       右边的"取值"列被挤到很窄，两列都不好看。 */
  table{width:100%;border-collapse:collapse;margin:7px 0 0;
    table-layout:fixed;font-size:11px}
  th,td{text-align:left;padding:5px 8px;vertical-align:top;
    border-bottom:1px solid var(--doc-line);line-height:1.65}
  th{font-weight:600;color:var(--doc-t3);font-size:10.5px;
    background:var(--doc-elev)}
  tr:last-child td{border-bottom:none}
  td.mono{color:var(--doc-t2)}
  .sw{display:inline-block;width:11px;height:11px;border-radius:3px;
    box-shadow:inset 0 0 0 1px rgba(128,128,128,.3);
    vertical-align:-1px;margin-right:5px}

  /* ── 标签 ── */
  .tag{display:inline-block;font-size:9.5px;font-weight:700;
    padding:1.5px 7px;border-radius:7px;vertical-align:2px;
    margin-right:6px}
  .tag-r{background:rgba(226,92,92,.16);color:#ef8b8b}
  .tag-o{background:rgba(226,168,74,.16);color:#eec27a}
  .tag-g{background:rgba(78,201,176,.16);color:#7fd9c6}
  .tag-b{background:rgba(74,144,226,.18);color:#8ab8ee}

  /* ── 提示框：三类，语义不同不能混用 ── */
  .box{background:var(--doc-elev);border:1px solid var(--doc-line);
    border-radius:8px;padding:11px 13px;
    font-size:11.5px;color:var(--doc-t2);line-height:1.8}
  .warnbox{background:rgba(226,168,74,.07);
    border:1px solid rgba(226,168,74,.3);border-radius:8px;
    padding:11px 13px;font-size:11.5px;color:var(--doc-t2);
    line-height:1.8}
  .redbox{background:rgba(226,92,92,.07);
    border:1px solid rgba(226,92,92,.28);border-radius:8px;
    padding:11px 13px;font-size:11.5px;color:var(--doc-t2);
    line-height:1.8}
  .box b,.warnbox b,.redbox b{color:var(--doc-t1)}
  .box code,.warnbox code,.redbox code{background:transparent;
    padding:0;color:var(--doc-t3)}

  /* ── 改前 / 改后 对照 ── */
  .ba{display:grid;grid-template-columns:1fr 1fr;gap:13px}
  .bax{min-width:0}
  .bax-h{font-size:10.5px;font-weight:700;padding:5px 9px;
    border-radius:7px 7px 0 0}
  .bax-b{background:var(--doc-elev);border:1px solid var(--doc-line);
    border-radius:0 0 8px 8px;padding:11px;
    font-size:11px;color:var(--doc-t2);line-height:1.7}

  /* ── 设置界面模拟 ── */
  .ifd{display:flex;height:330px;border:1px solid var(--doc-line2);
    border-radius:9px;overflow:hidden;background:var(--doc-panel)}
  .ifd-nav{width:158px;flex:0 0 158px;padding:9px 0}
  .ifd-t{font-size:11.5px;font-weight:700;padding:4px 12px 9px;
    border-bottom:1px solid var(--doc-line);margin-bottom:6px}
  .ifd-navlist{display:flex;flex-direction:column}
  .ni{display:flex;align-items:center;gap:7px;padding:5px 12px;
    font-size:11px;color:var(--doc-t2);line-height:1.4}
  .ni.on{font-weight:600}
  .ni .ic{width:4px;height:4px;border-radius:1px;flex:0 0 4px;
    background:currentColor;opacity:.55}
  .ifd-body{flex:1;min-width:0;overflow:hidden;
    border-left:1px solid var(--doc-line)}
  .ifd-c{padding:11px 13px}
  .srow{display:flex;align-items:center;gap:14px;
    padding:8px 0;border-bottom:1px solid var(--doc-line)}
  .srow:last-child{border-bottom:none}
  .srow>div:first-child{flex:1;min-width:0}
  .sl{font-size:11.5px;color:var(--doc-t1);line-height:1.5}
  .sd{font-size:10.5px;color:var(--doc-t3);line-height:1.6;
    margin-top:2px}
  .seg{display:flex;border:1px solid var(--doc-line2);
    border-radius:7px;overflow:hidden}
  .seg>div{padding:5px 12px;font-size:10.5px;color:var(--doc-t2)}
  .seg>div.on{background:var(--doc-ac);color:#fff;font-weight:600}

  /* ── 滑杆 ── */
  .slider{width:120px;height:3px;border-radius:2px;
    background:var(--doc-line2);position:relative;flex:0 0 120px}
  .slider .fill{position:absolute;left:0;top:0;bottom:0;
    border-radius:2px;background:var(--doc-ac)}
  .slider .kn{position:absolute;top:50%;width:11px;height:11px;
    border-radius:50%;background:var(--doc-t1);
    transform:translate(-50%,-50%);
    box-shadow:0 1px 4px rgba(0,0,0,.5)}
  .pct{font-size:10px;color:var(--doc-t3);width:34px;
    text-align:right;flex:0 0 34px}

  /* ── 终端色样 ── */
  .fil{display:inline-block;width:11px;height:11px;border-radius:3px;
    box-shadow:inset 0 0 0 1px rgba(128,128,128,.3);
    vertical-align:-1px}
  .p{color:var(--doc-ac)}
  .g{color:var(--doc-ok)}
  .y{color:var(--doc-wr)}
  .r{color:var(--doc-dg)}
  .m{color:var(--doc-t3)}
  .kn{color:var(--doc-t2)}
  .dim{color:var(--doc-t3)}
  .accents{display:flex;gap:6px;max-width:236px;flex-wrap:wrap}
  .ac{width:26px;height:26px;border-radius:7px;cursor:pointer;
    position:relative;box-shadow:inset 0 0 0 1px rgba(128,128,128,.3);
    overflow:hidden}
  .ac i{position:absolute;inset:0;display:block}
  .ac:hover{box-shadow:inset 0 0 0 1.5px var(--doc-t3)}
  .ac.on{box-shadow:inset 0 0 0 2px var(--doc-t1),0 0 0 2px var(--doc-ac)}
  .tok{margin-top:8px}
  .tok td{padding:3px 6px;font-size:10px}
  .tok .k{font-family:var(--mono);color:var(--doc-t3);
    white-space:nowrap;width:88px}
  .tok .v{font-family:var(--mono);color:var(--doc-t2);
    white-space:nowrap}

  .hint{width:1180px;font-size:11px;color:var(--doc-t3);
    display:flex;align-items:center;gap:14px;flex-wrap:wrap;
    padding:0 4px}
  .hint span{display:flex;align-items:center;gap:5px}
'''


def main():
    P = '../docs/design/theme-wallpaper-split-2026-10-05.html'
    s = io.open(P, encoding='utf-8').read()

    # 锚点：**必须在变量块内部**（第一族变量的注释行），
    # 不能用"下一个 .demo{" 这种相对定位 —— 那正是上次误删的原因。
    MARK = '  /* ═══ 现代 UI 色板 ═══ */'
    assert MARK in s, '锚点不存在'
    i0 = s.index(MARK)
    # 终点：下拉/预览 CSS 的起始标记（我上一轮加的那段，位置确定）
    END = '\n  /* ═══ 主题选择器：下拉框 + 悬停预览 ═══════════════════════════'
    assert END in s, '终点标记不存在'
    i1 = s.index(END)
    # ⚠️ 打印确认：变量块之后到终点之间**应该只有变量块**。
    # ⚠️ 区间里除了变量块，还夹着**演示区样式**（.demo / .dm-*）。
    #    那是要**保留**的 —— 上次就是在这里把边界搞错了。
    between = s[i0:i1]
    n_blocks = between.count('[data-theme="')
    assert n_blocks == 33, \
        '变量块数 %d ≠ 33 —— 锚点或区间不对，停下' % n_blocks
    # 演示区样式的起点 = 变量块之后的第一个非变量规则
    demo_at = between.index('  .demo{')
    print('  变量块 %d 个 · 演示区样式在区间第 %d 字节处'
          % (n_blocks, demo_at))
    # ★ **只替换变量块那一段**，演示区样式留在原地。
    out = (s[:i0]
           + s[i0:demo_at]                # 33 个变量块
           + DOC_CSS.strip('\n') + '\n\n'
           + between[demo_at:]            # .demo 起，保留
           + s[i1:])
    io.open(P, 'w', encoding='utf-8', newline='').write(out)
    print('文档区样式已重写（%d 字节）' % len(out))


if __name__ == '__main__':
    main()
