#!/usr/bin/env python3
"""全库体检：越界之外，再看尺寸一致性、居中偏移、家族几何分叉。"""
import glob, os, sys, math, re
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from importlib.machinery import SourceFileLoader
M = SourceFileLoader("aud", os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                         "icon-audit.py")).load_module()

def analyse(d):
    rows = []
    for f in sorted(glob.glob(os.path.join(d, "*.svg"))):
        name = os.path.basename(f)[:-4]
        src = open(f, encoding="utf-8").read()
        body = src.split(">", 1)[1].rsplit("</svg>", 1)[0]
        pts = []
        for m in re.finditer(r'<path\s+d="([^"]+)"', body):
            pts.extend(M.path_points(m.group(1)))
        for m in re.finditer(r"<rect\b([^/>]*)/?>", body):
            at = dict(re.findall(r'([a-z-]+)="([^"]*)"', m.group(1)))
            x, y = float(at["x"]), float(at["y"])
            w, h = float(at["width"]), float(at["height"])
            pts += [(x, y), (x + w, y), (x, y + h), (x + w, y + h)]
        for m in re.finditer(r"<circle\b([^/>]*)/?>", body):
            at = dict(re.findall(r'([a-z-]+)="([^"]*)"', m.group(1)))
            cx, cy, r = float(at["cx"]), float(at["cy"]), float(at["r"])
            pts += [(cx - r, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy), (cx + r, cy - r)]
        if not pts:
            continue
        x0, y0, x1, y1 = M.bbox_of(pts)
        rows.append((name, x0, y0, x1, y1, x1 - x0, y1 - y0,
                     (x0 + x1) / 2 - 12, (y0 + y1) / 2 - 12))
    return rows

d = sys.argv[1] if len(sys.argv) > 1 else "."
rows = analyse(d)
PAD = 1.0
print("== 1. 越界 ==")
for n, x0, y0, x1, y1, *_ in rows:
    v = []
    if x0 < -0.001: v.append(f"LEFT {x0:+.2f}")
    if y0 < -0.001: v.append(f"TOP {y0:+.2f}")
    if x1 > 24.001: v.append(f"RIGHT {x1-24:+.2f}")
    if y1 > 24.001: v.append(f"BOT {y1-24:+.2f}")
    if v: print(f"  {n:<22}{', '.join(v)}")

print("\n== 2. 安全区内边距不足（距边界 < 1.0，虽未越界但危险）==")
for n, x0, y0, x1, y1, *_ in rows:
    gaps = {"L": x0, "T": y0, "R": 24 - x1, "B": 24 - y1}
    tight = [f"{k}{v:.2f}" for k, v in gaps.items() if v < PAD - 0.001 and v > -0.001]
    if tight: print(f"  {n:<22}{', '.join(tight)}")

print("\n== 3. 视觉尺寸异常（宽或高 < 15 或 > 22）==")
for n, x0, y0, x1, y1, w, h, *_ in rows:
    if w < 15 or h < 15 or w > 22 or h > 22:
        print(f"  {n:<22}{w:5.1f} × {h:5.1f}")

print("\n== 4. 居中偏移（|dx| 或 |dy| > 1.2）==")
for n, x0, y0, x1, y1, w, h, dx, dy in rows:
    if abs(dx) > 1.2 or abs(dy) > 1.2:
        print(f"  {n:<22}dx {dx:+5.2f}  dy {dy:+5.2f}")

print("\n== 5. 尺寸分布 ==")
ws = sorted(r[5] for r in rows); hs = sorted(r[6] for r in rows)
def q(a, p): return a[min(len(a) - 1, int(len(a) * p))]
print(f"  宽  min {ws[0]:.1f}  p25 {q(ws,.25):.1f}  中位 {q(ws,.5):.1f}  p75 {q(ws,.75):.1f}  max {ws[-1]:.1f}")
print(f"  高  min {hs[0]:.1f}  p25 {q(hs,.25):.1f}  中位 {q(hs,.5):.1f}  p75 {q(hs,.75):.1f}  max {hs[-1]:.1f}")
print(f"  合计 {len(rows)} 个")
