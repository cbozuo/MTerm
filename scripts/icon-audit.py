#!/usr/bin/env python3
"""Audit assets/icons/*.svg: geometric bbox incl. stroke expansion, viewBox overflow."""
import glob
import math
import os
import re
import sys

NUM = re.compile(r"[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?")
SEG = re.compile(r"([MmZzLlHhVvCcSsQqTtAa])([^MmZzLlHhVvCcSsQqTtAa]*)")
SW = 2.0
PAD = SW / 2.0  # round cap/join expansion


def bez(p0, pts, n=24):
    """Sample a cubic bezier given control points list (3 tuples)."""
    p1, p2, p3 = pts
    out = []
    for i in range(n + 1):
        t = i / n
        u = 1 - t
        x = u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0]
        y = u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1]
        out.append((x, y))
    return out


def arc(p0, rx, ry, phi, large, sweep, p1, n=32):
    if rx == 0 or ry == 0:
        return [p0, p1]
    phi = math.radians(phi)
    cp, sp = math.cos(phi), math.sin(phi)
    dx, dy = (p0[0] - p1[0]) / 2, (p0[1] - p1[1]) / 2
    x1p, y1p = cp * dx + sp * dy, -sp * dx + cp * dy
    rx, ry = abs(rx), abs(ry)
    lam = x1p**2 / rx**2 + y1p**2 / ry**2
    if lam > 1:
        s = math.sqrt(lam)
        rx, ry = rx * s, ry * s
    num = rx**2 * ry**2 - rx**2 * y1p**2 - ry**2 * x1p**2
    den = rx**2 * y1p**2 + ry**2 * x1p**2
    co = math.sqrt(max(0.0, num / den)) if den else 0.0
    if large == sweep:
        co = -co
    cxp, cyp = co * rx * y1p / ry, -co * ry * x1p / rx
    cx = cp * cxp - sp * cyp + (p0[0] + p1[0]) / 2
    cy = sp * cxp + cp * cyp + (p0[1] + p1[1]) / 2

    def angle(ux, uy, vx, vy):
        d = (ux * vx + uy * vy) / (math.hypot(ux, uy) * math.hypot(vx, vy) + 1e-12)
        a = math.acos(max(-1.0, min(1.0, d)))
        return -a if ux * vy - uy * vx < 0 else a

    th1 = angle(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry)
    dth = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry)
    if not sweep and dth > 0:
        dth -= 2 * math.pi
    if sweep and dth < 0:
        dth += 2 * math.pi
    out = []
    for i in range(n + 1):
        t = th1 + dth * i / n
        out.append((cx + rx * math.cos(t) * cp - ry * math.sin(t) * sp,
                    cy + rx * math.cos(t) * sp + ry * math.sin(t) * cp))
    return out


def path_points(d):
    pts, cur, start = [], (0.0, 0.0), (0.0, 0.0)
    prev_c2 = prev_q = None
    for cmd, argstr in SEG.findall(d):
        a = [float(x) for x in NUM.findall(argstr)]
        rel = cmd.islower()
        C = cmd.upper()

        def P(x, y):
            return (cur[0] + x, cur[1] + y) if rel else (x, y)

        if C == "M":
            for i in range(0, len(a), 2):
                cur = P(a[i], a[i + 1])
                pts.append(cur)
                if i == 0:
                    start = cur
            prev_c2 = prev_q = None
        elif C == "Z":
            if cur != start:
                pts.append(start)
            cur = start
        elif C in "LT":
            for i in range(0, len(a), 2):
                cur = P(a[i], a[i + 1])
                pts.append(cur)
            prev_c2 = prev_q = None
        elif C == "H":
            for v in a:
                cur = (cur[0] + v, cur[1]) if rel else (v, cur[1])
                pts.append(cur)
            prev_c2 = prev_q = None
        elif C == "V":
            for v in a:
                cur = (cur[0], cur[1] + v) if rel else (cur[0], v)
                pts.append(cur)
            prev_c2 = prev_q = None
        elif C in "SQ":
            step = 4 if C == "S" else 4
            i = 0
            while i + 1 < len(a):
                if C == "S":
                    c2 = P(a[i], a[i + 1])
                    c1 = (2 * cur[0] - prev_c2[0], 2 * cur[1] - prev_c2[1]) if prev_c2 else cur
                    end = P(a[i + 2], a[i + 3])
                    i += 4
                    prev_c2 = c2
                else:
                    q = P(a[i], a[i + 1])
                    c1 = (cur[0] + 2 / 3 * (q[0] - cur[0]), cur[1] + 2 / 3 * (q[1] - cur[1]))
                    c2 = (q[0] + 2 / 3 * (end_g := P(a[i + 2], a[i + 3]))[0] - 2 / 3 * q[0],
                          q[1] + 2 / 3 * end_g[1] - 2 / 3 * q[1])
                    end = end_g
                    i += 4
                    prev_q = q
                pts.extend(bez(cur, [c1, c2, end]))
                cur = end
        elif C == "C":
            i = 0
            while i + 5 < len(a):
                c1, c2, end = P(a[i], a[i + 1]), P(a[i + 2], a[i + 3]), P(a[i + 4], a[i + 5])
                pts.extend(bez(cur, [c1, c2, end]))
                prev_c2, cur = c2, end
                i += 6
        elif C == "A":
            i = 0
            while i + 6 < len(a):
                end = P(a[i + 5], a[i + 6])
                pts.extend(arc(cur, a[i], a[i + 1], a[i + 2], int(a[i + 3]), int(a[i + 4]), end))
                cur = end
                i += 7
    return pts


def bbox_of(points, pad=PAD):
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    return (min(xs) - pad, min(ys) - pad, max(xs) + pad, max(ys) + pad)


def main():
    d = sys.argv[1] if len(sys.argv) > 1 else "."
    rows = []
    shp = re.compile(r'<path\s+d="([^"]+)"|<(rect|circle)\b([^/>]*)')
    for f in sorted(glob.glob(os.path.join(d, "*.svg"))):
        src = open(f, encoding="utf-8").read()
        body = src.split(">", 1)[1].rsplit("</svg>", 1)[0]
        pts = []
        for m in re.finditer(r'<path\s+d="([^"]+)"', body):
            pts.extend(path_points(m.group(1)))
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
        x0, y0, x1, y1 = bbox_of(pts)
        rows.append((os.path.basename(f), x0, y0, x1, y1, pts))
    name = ["file", "minX", "minY", "maxX", "maxY", "verdict"]
    print(f"{'file':<34}{'minX':>7}{'minY':>7}{'maxX':>7}{'maxY':>7}  verdict")
    bad = []
    for f, x0, y0, x1, y1, _ in rows:
        v = []
        if x0 < -0.001:
            v.append(f"LEFT {x0:+.2f}")
        if y0 < -0.001:
            v.append(f"TOP {y0:+.2f}")
        if x1 > 24.001:
            v.append(f"RIGHT {x1 - 24:+.2f}")
        if y1 > 24.001:
            v.append(f"BOTTOM {y1 - 24:+.2f}")
        verdict = "OVERFLOW: " + ", ".join(v) if v else "ok"
        if v:
            bad.append(f)
        print(f"{f:<34}{x0:>7.2f}{y0:>7.2f}{x1:>7.2f}{y1:>7.2f}  {verdict}")
    print(f"\ntotal={len(rows)}  overflow={len(bad)}")
    for b in bad:
        print("  -", b)


if __name__ == "__main__":
    main()
