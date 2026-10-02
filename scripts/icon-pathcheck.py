#!/usr/bin/env python3
"""SVG 路径冗余 / 无效段检测。

检查三类问题：
  1) 共线且被另一段完全覆盖的直线段（纯冗余 —— 删掉视觉无变化）
  2) 相邻点重合的零长度段
  3) 同一段路径被多个图标共用（家族共享是设计，但能暴露「复制粘贴」痕迹）

用法:  python scripts/icon-pathcheck.py [assets/icons]
"""
import io
import os
import re
import glob
import math
import collections
import sys

NUM = re.compile(r"-?\d*\.?\d+(?:[eE]-?\d+)?")
CMD = re.compile(r"[MmLlHhVvCcSsQqTtAaZz]")
STEP = {"C": 6, "S": 4, "Q": 4, "T": 2, "A": 7}


def subpaths(d):
    """把 d 拆成子路径 [{start, pts, curve}]。"""
    toks = re.findall(r"[MmLlHhVvCcSsQqTtAaZz]|-?\d*\.?\d+(?:[eE]-?\d+)?", d)
    out, pts, curve = [], [], False
    closed = False
    cur = None
    cx = cy = 0.0
    j = 0
    cmd = None

    def flush():
        if pts:
            out.append({"start": cur, "pts": list(pts),
                        "curve": curve, "closed": closed})

    while j < len(toks):
        t = toks[j]
        if CMD.fullmatch(t):
            cmd = t
            j += 1
            if cmd in "Zz":
                if cur is not None and pts:
                    # 闭合：回到起点。记 closed 标记，零长度检查时跳过这一对。
                    pts.append(cur)
                    closed = True
                continue
            nums = []
            while j < len(toks) and not CMD.fullmatch(toks[j]):
                nums.append(float(toks[j]))
                j += 1
            rel = cmd.islower()
            c = cmd.upper()
            if c == "M":
                if cur is not None:
                    flush()
                pts, curve = [], False
                closed = False
                cx, cy = (cx + nums[0], cy + nums[1]) if rel else (nums[0], nums[1])
                cur = (cx, cy)
                pts.append(cur)
                # M 之后多余坐标按标准是隐含的 L —— 必须逐点收进来，
                # 否则会被误判成「零长度段」。
                k = 2
                while k + 1 < len(nums):
                    x = nums[k] + (cx if rel else 0)
                    y = nums[k + 1] + (cy if rel else 0)
                    pts.append((x, y))
                    cx, cy = x, y
                    k += 2
            elif c == "H":
                x = nums[0] + (cx if rel else 0)
                pts.append((x, cy))
                cx = x
            elif c == "V":
                y = nums[0] + (cy if rel else 0)
                pts.append((cx, y))
                cy = y
            elif c == "L":
                k = 0
                while k + 1 < len(nums):
                    x = nums[k] + (cx if rel else 0)
                    y = nums[k + 1] + (cy if rel else 0)
                    pts.append((x, y))
                    cx, cy = x, y
                    k += 2
            elif c in STEP:
                curve = True
                st = STEP[c]
                k = 0
                while k + st <= len(nums):
                    x = nums[k + st - 2] + (cx if rel else 0)
                    y = nums[k + st - 1] + (cy if rel else 0)
                    pts.append((x, y))
                    cx, cy = x, y
                    k += st
        else:
            j += 1
    flush()
    return out


def contains(a, b, eps=0.02):
    """线段 a 是否被线段 b 完全覆盖（需共线）。"""
    (x1, y1), (x2, y2) = a
    (x3, y3), (x4, y4) = b

    def cross(ox, oy, px, py, qx, qy):
        return (px - ox) * (qy - oy) - (py - oy) * (qx - ox)

    if abs(cross(x3, y3, x4, y4, x1, y1)) > eps:
        return False
    if abs(cross(x3, y3, x4, y4, x2, y2)) > eps:
        return False
    if abs(x3 - x4) < eps:                       # 竖直
        lo, hi = sorted([y1, y2])
        lb, hb = sorted([y3, y4])
        return lo >= lb - eps and hi <= hb + eps
    lo, hi = sorted([x1, x2])                    # 水平或一般（按 x 投影）
    lb, hb = sorted([x3, x4])
    return lo >= lb - eps and hi <= hb + eps


def main():
    d = sys.argv[1] if len(sys.argv) > 1 else "assets/icons"
    files = sorted(glob.glob(os.path.join(d, "*.svg")))
    if not files:
        print("!! 未找到 svg:", d)
        return 1

    shared = collections.defaultdict(set)
    n_red = n_zero = 0

    print("=== 冗余 / 无效路径段 ===")
    for p in files:
        name = os.path.basename(p)[:-4]
        for dstr in re.findall(r'\sd="([^"]*)"', io.open(p, encoding="utf-8").read()):
            sps = subpaths(dstr)
            shared[dstr].add(name)

            for sp in sps:                        # 相邻点重合
                pts = sp["pts"]
                last = len(pts) - 1
                for k in range(last):
                    # 闭合点（Z 回到起点）不是零长度段，跳过多出来的那一对
                    if sp.get("closed") and k == last - 1:
                        continue
                    if math.dist(pts[k], pts[k + 1]) < 1e-6:
                        print("  %-22s 零长度段 @%s" % (name, pts[k]))
                        n_zero += 1

            if any(s["curve"] for s in sps):
                continue                          # 含曲线的先不判覆盖
            segs = []
            for sp in sps:
                pts = sp["pts"]
                for k in range(len(pts) - 1):
                    if math.dist(pts[k], pts[k + 1]) > 1e-6:
                        segs.append((pts[k], pts[k + 1]))
            for i in range(len(segs)):
                for j2 in range(len(segs)):
                    if i == j2:
                        continue
                    la = math.dist(*segs[i])
                    lb = math.dist(*segs[j2])
                    if lb > la + 1e-6 and contains(segs[i], segs[j2]):
                        print("  %-22s 段 %s→%s (长%.2f) 被 %s→%s (长%.2f) 完全覆盖"
                              % (name, segs[i][0], segs[i][1], la,
                                 segs[j2][0], segs[j2][1], lb))
                        n_red += 1
    if not n_red and not n_zero:
        print("  未发现")

    print("\n=== 同一段路径被多个图标共用 ===")
    for dstr, names in sorted(shared.items(), key=lambda x: -len(x[1])):
        if len(names) >= 2:
            print("  %d 个: %s" % (len(names), ", ".join(sorted(names))))
            print("      %s" % (dstr[:100] + ("..." if len(dstr) > 100 else "")))
    print("\n冗余段 %d 处，零长度段 %d 处，扫描 %d 个图标" % (n_red, n_zero, len(files)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
