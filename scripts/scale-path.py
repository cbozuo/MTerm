#!/usr/bin/env python3
"""按比例缩放 SVG path 的绝对/相对坐标，并同步缩放 circle。
用法: scale-path.py <s> <cx> <cy> <path>   —— 以 (cx,cy) 为不动点缩放 s 倍
"""
import re, sys

NUM = re.compile(r"[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?")
CMD = re.compile(r"([MmZzLlHhVvCcSsQqTtAa])([^MmZzLlHhVvCcSsQqTtAa]*)")
# 每条命令里“坐标对”的数量（A 特殊处理）
PAIRS = {"M": None, "L": None, "T": None, "S": 2, "Q": 2, "C": 3}


def fmt(v):
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def scale_path(d, s, ox, oy, dx=None, dy=None):
    """源中心 (ox,oy) 缩放 s 倍后，中心平移到 (dx,dy)（默认仍在 (ox,oy)）。"""
    out = []
    cx = cy = 0.0
    gx = ox if dx is None else dx
    gy = oy if dy is None else dy
    tx = lambda v: (v - ox) * s + gx
    ty = lambda v: (v - oy) * s + gy
    for cmd, argstr in CMD.findall(d):
        a = [float(x) for x in NUM.findall(argstr)]
        rel = cmd.islower()
        C = cmd.upper()
        if C == "Z":
            out.append("z")
            continue
        if C == "A":
            # rx ry rot laf sf x y  —— rx/ry 与终点 x,y 缩放；相对时终点为 delta
            vals = []
            for i in range(0, len(a), 7):
                rx, ry, rot, laf, sf = a[i], a[i+1], a[i+2], a[i+3], a[i+4]
                ex, ey = a[i+5], a[i+6]
                if rel:
                    nex, ney = ex * s, ey * s
                else:
                    nex, ney = tx(ex), ty(ey)
                vals += [fmt(rx*s), fmt(ry*s), fmt(rot), int(laf), int(sf), fmt(nex), fmt(ney)]
                cx, cy = (cx+nex, cy+ney) if rel else (nex, ney)
            out.append(cmd + " ".join(map(str, vals)))
            continue
        if C == "H":
            vals = [fmt(v*s if rel else tx(v)) for v in a]
            cx = (cx + a[-1]*s) if rel else tx(a[-1])
            out.append(cmd + " ".join(vals))
            continue
        if C == "V":
            vals = [fmt(v*s if rel else ty(v)) for v in a]
            cy = (cy + a[-1]*s) if rel else ty(a[-1])
            out.append(cmd + " ".join(vals))
            continue
        vals = []
        for i in range(0, len(a) - 1, 2):
            x, y = a[i], a[i+1]
            vals += [fmt(x*s if rel else tx(x)), fmt(y*s if rel else ty(y))]
            if not rel:
                cx, cy = tx(x), ty(y)
            else:
                cx += x*s; cy += y*s
        out.append(cmd + " ".join(vals))
    return "".join(out)


if __name__ == "__main__":
    # scale-path.py <s> <srcX> <srcY> <dstX> <dstY> <path>
    a = sys.argv[1:]
    print(scale_path(a[5], float(a[0]), float(a[1]), float(a[2]),
                     float(a[3]), float(a[4])))
