# 量测 theme_pick_probe.png：浮层几何与行内元素的像素级核对（#theme-dedupe §5）
# 用法: 托管 venv python scripts/measure_theme_pick.py
from PIL import Image

img = Image.open('target/theme_pick_probe.png').convert('RGB')
W, H = img.size
px = img.load()
print(f'画布 {W}x{H}')


def find_blobs(rgb, tol=8, min_px=6):
    """颜色 rgb(±tol) 的连通块 bbox 列表（4 邻接）。"""
    seen = set()
    blobs = []
    for y in range(H):
        for x in range(W):
            if (x, y) in seen:
                continue
            r, g, b = px[x, y]
            if abs(r - rgb[0]) > tol or abs(g - rgb[1]) > tol or abs(b - rgb[2]) > tol:
                continue
            stack = [(x, y)]
            seen.add((x, y))
            minx = maxx = x
            miny = maxy = y
            n = 0
            while stack:
                cx, cy = stack.pop()
                n += 1
                minx = min(minx, cx); maxx = max(maxx, cx)
                miny = min(miny, cy); maxy = max(maxy, cy)
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    nx, ny = cx + dx, cy + dy
                    if 0 <= nx < W and 0 <= ny < H and (nx, ny) not in seen:
                        rr, gg, bb = px[nx, ny]
                        if abs(rr - rgb[0]) <= tol and abs(gg - rgb[1]) <= tol and abs(bb - rgb[2]) <= tol:
                            seen.add((nx, ny))
                            stack.append((nx, ny))
            if n >= min_px:
                blobs.append((minx, miny, maxx, maxy, n))
    return blobs


TARGETS = {
    'ac 圆点 / 八带第6格 #1FC8B4': (0x1F, 0xC8, 0xB4),
    '八带第1格 #F75464': (0xF7, 0x54, 0x64),
    '★ / 八带第3格 #E8A33E': (0xE8, 0xA3, 0x3E),
    'n3 首格 #161619': (0x16, 0x16, 0x19),
}
for name, rgb in TARGETS.items():
    blobs = find_blobs(rgb)
    blobs.sort(key=lambda b: (b[1], b[0]))
    print(f'{name}: {len(blobs)} 块')
    for b in blobs[:14]:
        print(f'   x{b[0]}..{b[2]} y{b[1]}..{b[3]}  ({b[2]-b[0]+1}x{b[3]-b[1]+1}, {b[4]}px)')
