#!/usr/bin/env python3
"""图标小尺寸可读性测量（连通分量法）。

用于角落手柄 / 状态点这类**渲染尺寸远小于 24 网格**的图标。这个尺度下"元素会
不会糊成一团"比形状好不好看重要得多 —— 主流形状未必可用（实测「三条斜线」式
手柄在 11~20px 全档粘连，而三点式全程保持 3 个分离分量）。

**判据是自比的，不需要预知元素数**：把图标按多档尺寸渲染成像素图，数
**面积 >=2px 的 4 邻域连通分量**。若某档的分量数少于该图标各档的最大值，
说明该档有元素粘连（= 糊）。

⚠️ 三处刻意的设计，都是为了不误报（检测工具误报会让人不再信任它）：
  1. **墨量过低（<4px）或 0 个成块分量 → 标"测不到"而非粘连** —— 细描边图标在
     小尺寸下抗锯齿像素会整体落到阈值之上，看似"分量 0"；
  2. **只报相对自身最大值的下降**，不与绝对期望值比较（不需要预知元素数）；
  3. **整库模式不下判定** —— 普通 24 网格图标在小档位合并是细描边的正常现象，
     它们本就不在小尺寸渲染。判定只在 --focus 下给出，用于**横向对比候选**。

用法:  python scripts/icon-read.py --focus NAME [目录]     # ← 主要用法：单图标判定
       python scripts/icon-read.py [目录]                   # 整库信息表（不下判定）
输出:  --focus：各档分量 / 墨量 / 内容像素尺寸，并判定 ok / 粘连 / 测不到；
       整库：各档连通分量的信息表（不下判定，见下方「注意」）。

实现:  沿用 icon-sim.py —— SVG 内联进临时 HTML，页面内 canvas 完成全部计算
       （避免逐档截图），再用 Edge --dump-dom 把结果读回。
"""
import os
import re
import sys
import json
import glob
import html as _html
import subprocess
import tempfile

EDGES = [
    r"C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    r"C:/Program Files/Microsoft/Edge/Application/msedge.exe",
    "/usr/bin/microsoft-edge",
    "/usr/bin/google-chrome",
]

SIZES = [11, 12, 14, 16, 20]
INK_FLOOR = 4          # 低于此墨量视为"测不到"

HTML = r"""<!DOCTYPE html><html><head><meta charset="utf-8">
<style>body{background:#fff;color:#111;font:12px monospace;margin:0}
#out{white-space:pre-wrap;padding:6px}</style></head><body>
<pre id="out">pending</pre><script>
const ICONS=__ITEMS__; const SIZES=__SIZES__;
function pix(svg,size){return new Promise(res=>{
  const img=new Image(),u=URL.createObjectURL(new Blob([svg],{type:'image/svg+xml'}));
  img.onload=()=>{const c=document.createElement('canvas');c.width=c.height=size;
    const g=c.getContext('2d',{willReadFrequently:true});
    g.fillStyle='#fff';g.fillRect(0,0,size,size);
    g.imageSmoothingEnabled=true;g.imageSmoothingQuality='high';
    g.drawImage(img,0,0,size,size);
    const d=g.getImageData(0,0,size,size).data;URL.revokeObjectURL(u);res(d);};
  img.onerror=()=>res(null);img.src=u;});}
function analyze(d,size,th){
  const N=size*size, m=new Uint8Array(N); let ink=0;
  let minx=size,miny=size,maxx=-1,maxy=-1;
  for(let i=0;i<N;i++){
    const a=d[i*4]*.299+d[i*4+1]*.587+d[i*4+2]*.114;
    if(a<th){ m[i]=1; ink++;
      const x=i%size,y=(i/size)|0;
      if(x<minx)minx=x; if(x>maxx)maxx=x; if(y<miny)miny=y; if(y>maxy)maxy=y; } }
  const seen=new Uint8Array(N), areas=[]; const st=[];
  for(let s=0;s<N;s++){
    if(!m[s]||seen[s]) continue;
    let area=0; st.length=0; st.push(s); seen[s]=1;
    while(st.length){
      const p=st.pop(); area++;
      const x=p%size,y=(p/size)|0;
      for(const [ax,ay] of [[x-1,y],[x+1,y],[x,y-1],[x,y+1]]){
        if(ax<0||ay<0||ax>=size||ay>=size) continue;
        const q=ay*size+ax;
        if(m[q]&&!seen[q]){ seen[q]=1; st.push(q); } } }
    areas.push(area); }
  return {ink, comps:areas.filter(a=>a>=2).length,
          w:maxx<0?0:maxx-minx+1, h:maxy<0?0:maxy-miny+1};
}
(async()=>{
  const out=[];
  for(const ic of ICONS){
    const rows=[];
    for(const s of SIZES){
      const d=await pix(ic.s,s);
      if(!d){ rows.push({s,bad:true}); continue; }
      const r=analyze(d,s,200);
      r.s=s; r.tooThin=(r.ink < __FLOOR__) || (r.comps === 0);
      rows.push(r);
    }
    out.push({n:ic.n, rows});
  }
  document.getElementById('out').textContent=JSON.stringify(out);
})();
</script></body></html>
"""


def find_browser():
    for p in EDGES:
        if os.path.exists(p):
            return p
    return None


def main():
    a = sys.argv[1:]
    show_all = "--all" in a
    focus = None
    if "--focus" in a:
        i = a.index("--focus")
        if i + 1 >= len(a):
            print("!! --focus 需要图标名")
            return 1
        focus = a[i + 1]
        a = a[:i] + a[i + 2:]
    d = next((x for x in a if not x.startswith("--")), "assets/icons")
    if not os.path.isdir(d):
        print("!! 目录不存在:", d)
        return 1

    items = [{"n": os.path.basename(p)[:-4],
              "s": open(p, encoding="utf-8").read().strip()}
             for p in sorted(glob.glob(os.path.join(d, "*.svg")))]
    if focus:
        items = [x for x in items if x["n"] == focus]
        if not items:
            print("!! 未找到:", focus)
            return 1

    br = find_browser()
    if not br:
        print("!! 未找到 Edge/Chrome")
        return 1

    page = (HTML.replace("__ITEMS__", json.dumps(items, ensure_ascii=False))
                .replace("__SIZES__", json.dumps(SIZES))
                .replace("__FLOOR__", str(INK_FLOOR)))
    with tempfile.TemporaryDirectory() as td:
        f = os.path.join(td, "m.html")
        open(f, "w", encoding="utf-8").write(page)
        raw = subprocess.run(
            [br, "--headless=new", "--disable-gpu",
             "--virtual-time-budget=40000", "--dump-dom",
             "file:///" + f.replace("\\", "/")],
            capture_output=True, text=True, encoding="utf-8", errors="ignore").stdout
    m = re.search(r'<pre id="out">(.*?)</pre>', raw, re.S)
    if not m:
        print("!! 未取到结果")
        return 1
    data = json.loads(_html.unescape(m.group(1)))

    def measurable(ic):
        return [r for r in ic["rows"] if not r.get("bad") and not r.get("tooThin")]

    if focus:
        ic = data[0]
        rows = ic["rows"]
        base = max([r["comps"] for r in measurable(ic)], default=0)
        print("== %s ==   各档最大连通分量 %d" % (ic["n"], base))
        for r in rows:
            if r.get("bad"):
                print("  %3dpx  渲染失败" % r["s"]); continue
            tag = "测不到" if r["tooThin"] else ("ok" if r["comps"] >= base else "粘连")
            print("  %3dpx  墨量 %4d  分量 %d  内容 %dx%d  %s"
                  % (r["s"], r["ink"], r["comps"], r["w"], r["h"], tag))
        return 0

    print("图标 %d 个 · 档位 %s px · 墨量下限 %d px（低于此视为测不到）"
          % (len(data), "/".join(map(str, SIZES)), INK_FLOOR))
    print("注意：普通 24 网格图标在小档位出现分量减少，是 2px 细描边的正常抗锯齿合流，")
    print("      不是缺陷 —— 它们本就不在小尺寸渲染。本工具的价值在 **--focus 对比候选**：")
    print("      同尺寸下元素仍能保持分离的候选，才是可用的角落手柄。")
    print("      下面只列**信息表**，不下判定。\n")

    print("== 各档连通分量（'测不到' = 墨量过低或全为亚像素碎点）==")
    for ic in data:
        cells = []
        for r in ic["rows"]:
            if r.get("bad"):
                cells.append("%s:-" % r["s"])
            elif r["tooThin"]:
                cells.append("%s:测不到" % r["s"])
            else:
                cells.append("%s:%d" % (r["s"], r["comps"]))
        print("  %-24s %s" % (ic["n"], "  ".join(cells)))
    return 0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
