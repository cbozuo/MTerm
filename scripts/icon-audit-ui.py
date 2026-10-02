# 全库核对：① 预览页描述 vs 代码里的真实标签；② 高相关图对的“同屏风险”
#            ③ 悬空引用 / 库内预留清单
#
# #icon-svg-migration 2026-10-02：本脚本原先按 **Material 码位** 在源码里找 `\u{Exxxx}`。
# 迁移为 SVG 后源码只出现 **图标名**，码位已废弃 —— 旧版会给出 87 条「无活跃引用」的
# 全量误报。现改为按名匹配，并把「悬空引用（引用了不存在的 svg）」纳入检查。
import io, os, re

ROOT = r"C:/workspace/MTerm"
ICON = os.path.join(ROOT, "assets/icons")

pv = io.open(os.path.join(ICON, "gen-preview.sh"), encoding="utf-8").read()
rows = re.findall(r'^row\s+"([^"]+)"\s+([a-z0-9-]+)\s+"([^"]*)"\s+"([^"]*)"', pv, re.M)

# 逐行读 ui/ 与 src/
LINES = []                      # (relpath, lineno, text)
for base in ("ui", "src"):
    for root, _, fs in os.walk(os.path.join(ROOT, base)):
        for f in fs:
            if not f.endswith((".slint", ".rs")):
                continue
            p = os.path.join(root, f)
            rel = os.path.relpath(p, ROOT).replace("\\", "/")
            for i, t in enumerate(io.open(p, encoding="utf-8", errors="ignore"), 1):
                LINES.append((rel, i, t))

# ── 图标名的三种引用形式（迁移后）────────────────────────────────────────────
#   1) glyph: "name"                     —— Icon 组件入口
#   2) icon:  "name"                     —— NavItem / 顶栏按钮的 icon 属性
#   3) return "name";                    —— 返回图标名的字符串函数（Theme.protocol-icon、
#                                           session_dialog 的 footer-status-icon 等）
# ①② 的值可能是三元表达式（如 `glyph: root.hidden ? "visibility" : "visibility-off"`），
# 因此取冒号后到 `;`/行尾的整段，再从中抠出全部字面量。
# 所有形式都只在该字面量与某个**真实存在的图标名**完全相等时才计入 —— 借此避开
# `icon:`（窗口图标）/ `return "..."`（非图标字符串）这类同名字段的噪声。
# ⚠️ 只扫 .slint：Rust 侧的 `"settings"` 之类是日志字段值，与图标无关。
# 映射表 ui/icons.slint 自身不算“使用”。
MAP_FILE = "ui/icons.slint"
REF = re.compile(r'\b(?:glyph|icon)\s*:\s*(?P<rhs>[^;\n]*)'
                 r'|return\s+"(?P<b>[a-z0-9-]+)"\s*;')

LIT = re.compile(r'"([^"\\]{2,40})"')
# 界面文案只认这三个属性 —— 图标块是 4~5 行的结构（Icon { glyph / tint / size }），
# 而 `icon:` 的兄弟属性里混着 `icon: "right"`（方位）、`icon: "23"`（串口预设）这类
# 与文案无关的字面量，一把捞会把它们当成标签，反而误导。
UI_TEXT = re.compile(r'\b(?:text|label|tooltip)\s*:\s*(?:@tr\(\s*)?"([^"\\]{1,40})"')

def _in_comment(t, pos):
    return "//" in t[:pos]

def _names_at(t):
    """列出该行里以图标名形式出现的 (名字, 列号, 形式)"""
    out = []
    for m in REF.finditer(t):
        if m.group("rhs") is not None:
            form = m.group(0).split(":")[0].strip()
            rhs, base = m.group("rhs"), m.start("rhs")
            for lm in LIT.finditer(rhs):
                # 三元表达式里夹着**比较用的字符串**（`root.dock-edge == "right" ? ...`）。
                # 字面量前面是 `==` / `!=` 就说明它在比条件，不是被赋的图标名。
                head = rhs[:lm.start()].rstrip()
                if head.endswith(("==", "!=", "=", "<", ">")):
                    continue
                out.append((lm.group(1), base + lm.start(), form))
        else:
            out.append((m.group("b"), m.start("b") - 1, "return"))
    return out

def labels_after(rel, ln, span=6):
    """取该行及其后 span 行里同类元素的界面文案（text / label / tooltip，跳过注释）"""
    out = []
    for r2, l2, t2 in LINES:
        if r2 != rel or not (ln <= l2 <= ln + span):
            continue
        for m in UI_TEXT.finditer(t2):
            if _in_comment(t2, m.start()):
                continue
            out.append(m.group(1))
    return out

def usages(name):
    """返回该图标名的全部 (文件,行号,是否注释)"""
    res = []
    for rel, ln, t in LINES:
        if rel == MAP_FILE or not rel.endswith(".slint"):
            continue
        for v, pos, form in _names_at(t):
            if v == name:
                res.append((rel, ln, _in_comment(t, pos)))
    return res

def active(name):
    return [u for u in usages(name) if not u[2]]

names = [n for g, n, c, d in rows]
svg = {os.path.basename(p)[:-4] for p in os.listdir(ICON) if p.endswith(".svg")}

print("=" * 100)
print("① 预览页描述 vs 代码里的真实界面标签")
print("=" * 100)
reserved = []
for g, n, code, desc in rows:
    us = active(n)
    if not us:
        reserved.append((n, g, desc))
        print("%-22s [%-9s] %-38s  未被引用（库内预留）" % (n, g, desc))
        continue
    labs, seen, keep = [], set(), []
    for rel, ln, _ in us:
        labs += labels_after(rel, ln)
    for l in labs:
        if l in seen:
            continue
        seen.add(l)
        keep.append(l)
    joined = " / ".join(keep[:6]) or "(无文案，纯图标按钮)"
    where = ", ".join(sorted({r for r, _, _ in us}))[:46]
    print("%-22s [%-9s] %-38s  代码: %s   {%s}" % (n, g, desc, joined, where))

print()
print("=" * 100)
print("② 高相关图对的“同屏风险”（两图标在同一文件里的最小行距）")
print("=" * 100)
# r 值是 assets/icons 侧跑 icon-sim.py 得到的**快照**；改了图标形状后要同步刷新，
# 否则这一列会落后于实际（行列距是现算的，不受影响）。
PAIRS = [
    ("sidebar-left-collapse", "sidebar-left-expand", 0.917),
    ("sidebar-right-collapse", "sidebar-right-expand", 0.917),
    ("refresh", "system-update", 0.890),
    ("create-new-folder", "folder", 0.876),
    ("new-session", "session-disconnect", 0.868),
    ("session-disconnect", "session-reconnect", 0.866),
    ("error", "info", 0.862),
    ("new-session", "session-reconnect", 0.850),
    ("checkbox", "sidebar-left-collapse", 0.833),
    ("checkbox", "split-vertical", 0.825),
    ("command-history", "play", 0.799),
    ("add", "bolt", 0.784),
    ("play", "refresh", 0.777),
    ("file", "paste", 0.774),
    ("add", "remove", 0.773),
    ("download", "minimize-to-tray", 0.753),
    ("move-to-group", "terminal", 0.512),
    ("quit", "minimize-to-tray", 0.000),
]

def cooccur(a, b):
    """返回 (最近行距, 说明)；跨文件或未接入则行距为 None"""
    ua, ub = active(a), active(b)
    if not ua or not ub:
        miss = a if not ua else b
        return None, "%s 未被界面引用（库内预留）" % miss
    best, where = None, ""
    for ra, la, _ in ua:
        for rb, lb, _ in ub:
            if ra != rb:
                continue
            d = abs(la - lb)
            if best is None or d < best:
                best, where = d, "%s:%d ↔ :%d" % (ra, la, lb)
    if best is None:
        fa = sorted({r for r, _, _ in ua})
        fb = sorted({r for r, _, _ in ub})
        return None, "不同文件（%s ↔ %s）" % (", ".join(fa[:2]), ", ".join(fb[:2]))
    return best, where

for a, b, r in PAIRS:
    d, where = cooccur(a, b)
    if d is None:
        note = "低 · " + where
    elif d <= 12:
        note = "★ 同一菜单/布局内（行距 %d）· %s" % (d, where)
    elif d <= 60:
        note = "中 · 同文件相邻（行距 %d）· %s" % (d, where)
    else:
        note = "低 · 同文件但相距 %d 行 · %s" % (d, where)
    print("r=%.3f  %-22s ↔ %-22s  %s" % (r, a, b, note))

print()
print("=" * 100)
print("③ 一致性检查（迁移后新增：引用的名字必须都有 svg，两者必须双向对齐）")
print("=" * 100)
# 悬空引用**只查 `glyph: "x"`** —— 只有这个属性是无歧义的图标入口（Icon 组件），
# 且映射表对未知名回落到 add.svg，会静默画出一个 "+"，正是 #icon-svg-migration
# 2026-10-02 那批回归的根因。`icon:` 是通用属性名，`icon: "right"`（停靠方位）、
# `icon: "23"`（串口预设）、`icon: "../assets/icon.png"`（窗口图标）都不是图标名，
# 一并查会凭空造出 20 条误报。
dangling = []
for rel, ln, t in LINES:
    if rel == MAP_FILE or not rel.endswith(".slint"):
        continue
    for v, pos, form in _names_at(t):
        if form == "glyph" and v not in svg:
            dangling.append("%s:%d  glyph \"%s\" 无对应 svg" % (rel, ln, v))
if dangling:
    for d in sorted(set(dangling)):
        print("  !! 悬空引用:", d)
else:
    print("  悬空引用: 0 ✓（代码引用的每个图标名都有对应 svg）")

never = sorted(svg - set(names))
if never:
    print("  !! 有 svg 但未进预览页:", ", ".join(never))
else:
    print("  预览页覆盖率: %d/%d ✓" % (len(svg), len(svg)))
print("  库内预留（有 svg、未进界面） %d 个: %s"
      % (len(reserved), ", ".join(n for n, _, _ in reserved) or "无"))
print("  预览页行数 %d / 磁盘 svg %d" % (len(rows), len(svg)))
