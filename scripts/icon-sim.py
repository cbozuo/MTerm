#!/usr/bin/env python3
"""图标相似度 / 视觉重量体检。

按 16px 实际渲染取灰度图算两两皮尔逊相关度（16px 是抗锯齿与笔画粘连最
严重的一档，用户真正认错就发生在这一档），另测墨量占比（墨像素/总面积）
作为视觉重量指标，并用 32px 连通分量发现断笔。

用法:  python scripts/icon-sim.py [assets/icons] [--focus NAME] [--top N] [--vs NAME]
       --focus 指定图标名时，额外列出该图标与全部其他图标的相似度排序
       --top   聚焦时列出前 N 个对手（默认 12）
       --vs    聚焦时另报指定对手的精确 r 与排名。
               ⚠️ 查某一对时**务必用 --vs** —— 不要拿「该图标没出现在前 N 里」当作 r=0，
               那会把「没进前 12」误读成「完全不撞」（实测踩过两次）。
输出:  相关度全库分布 + 最相似图对 + 墨量极值 + 连通分量异常

实现:  把 SVG 内联进临时 HTML，页面内用 canvas 完成全部计算（避免 77 次
      截图），再用 Edge --dump-dom 把结果读回。
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


def find_browser():
    for p in EDGES:
        if os.path.exists(p):
            return p
    return None


HTML = r"""<!DOCTYPE html><html><head><meta charset="utf-8">
<style>body{background:#fff;color:#111;font:12px monospace;margin:0}
#out{white-space:pre-wrap;padding:6px}</style></head><body>
<pre id="out">pending</pre><script>
const ICONS=__ITEMS__;
const FOCUS=__FOCUS__, TOPN=__TOPN__, VS=__VS__;
function pix(svg,size){return new Promise(res=>{
  const img=new Image(),u=URL.createObjectURL(new Blob([svg],{type:'image/svg+xml'}));
  img.onload=()=>{const c=document.createElement('canvas');c.width=c.height=size;
    const g=c.getContext('2d',{willReadFrequently:true});
    g.fillStyle='#fff';g.fillRect(0,0,size,size);
    g.imageSmoothingEnabled=true;g.imageSmoothingQuality='high';
    g.drawImage(img,0,0,size,size);
    const d=g.getImageData(0,0,size,size).data;URL.revokeObjectURL(u);res(d);};
  img.onerror=()=>res(null);img.src=u;});}
function gray(d,size){const v=new Float64Array(size*size);
  for(let i=0;i<size*size;i++)v[i]=255-(d[i*4]*.299+d[i*4+1]*.587+d[i*4+2]*.114);return v;}
function mask(d,size,th){const m=new Uint8Array(size*size);
  for(let i=0;i<size*size;i++)m[i]=(d[i*4]*.299+d[i*4+1]*.587+d[i*4+2]*.114)<th?1:0;return m;}
function pear(a,b){const n=a.length;let ma=0,mb=0;
  for(let i=0;i<n;i++){ma+=a[i];mb+=b[i];}ma/=n;mb/=n;
  let nu=0,da=0,db=0;for(let i=0;i<n;i++){const x=a[i]-ma,y=b[i]-mb;nu+=x*y;da+=x*x;db+=y*y;}
  return (da&&db)?nu/Math.sqrt(da*db):0;}
function comps(m,size){const lab=new Int32Array(size*size).fill(-1),sz=[],st=[];let id=0;
  for(let s=0;s<size*size;s++){ if(!m[s]||lab[s]>=0)continue;
    let c=0;st.length=0;st.push(s);lab[s]=id;
    while(st.length){const p=st.pop();c++;const y=(p/size)|0,x=p%size;
      for(let dy=-1;dy<=1;dy++)for(let dx=-1;dx<=1;dx++){
        const ny=y+dy,nx=x+dx; if(ny<0||nx<0||ny>=size||nx>=size)continue;
        const q=ny*size+nx; if(m[q]&&lab[q]<0){lab[q]=id;st.push(q);} } }
    sz.push(c);id++; }
  sz.sort((a,b)=>b-a);return sz;}
(async()=>{
  const G={},C={};
  for(const it of ICONS){
    G[it.n]=gray(await pix(it.s,16),16);
    C[it.n]=comps(mask(await pix(it.s,32),32,170),32);
  }
  const N=ICONS.map(x=>x.n),P=[];
  for(let i=0;i<N.length;i++)for(let j=i+1;j<N.length;j++)
    P.push({a:N[i],b:N[j],r:pear(G[N[i]],G[N[j]])});
  P.sort((x,y)=>y.r-x.r);
  const rs=P.map(p=>p.r).sort((a,b)=>a-b);
  const q=t=>rs[Math.min(rs.length-1,Math.floor(rs.length*t))];
  const ink=N.map(n=>{let s=0;for(const v of G[n])s+=v;return {n,v:s/G[n].length/255}})
             .sort((a,b)=>b.v-a.v);
  const L=[];
  L.push("== 16px 相关度分布（共 "+P.length+" 对）==");
  L.push("  中位 "+q(.5).toFixed(3)+"  p90 "+q(.9).toFixed(3)+"  p99 "+q(.99).toFixed(3)+
         "  max "+rs[rs.length-1].toFixed(3));
  for(const t of [.7,.8,.85,.9]) L.push("  r>="+t.toFixed(2)+" : "+rs.filter(v=>v>=t).length+" 对");
  L.push("");
  L.push("== 最相似 20 对 ==");
  P.slice(0,20).forEach(p=>L.push("  r="+p.r.toFixed(3)+"   "+p.a.padEnd(22)+p.b));
  L.push("");
  const mean=ink.reduce((s,o)=>s+o.v,0)/ink.length;
  L.push("== 16px 墨量（均值 "+mean.toFixed(3)+"）==");
  L.push("  最重 8:"); ink.slice(0,8).forEach(o=>L.push("    "+o.n.padEnd(24)+o.v.toFixed(3)+
      (o.v>mean*1.4?"   << 显著过重":"")));
  L.push("  最轻 8:"); ink.slice(-8).forEach(o=>L.push("    "+o.n.padEnd(24)+o.v.toFixed(3)));
  L.push("");
  // 可选：聚焦某个图标，列出它与全部其他图标的相似度（降序）
  if(FOCUS){
    const fp = P.filter(p=>p.a===FOCUS||p.b===FOCUS)
                .map(p=>({o:p.a===FOCUS?p.b:p.a, r:p.r}))
                .sort((x,y)=>y.r-x.r);
    if(fp.length){
      let fs=0; for(const v of G[FOCUS]) fs+=v;
      L.push("== 聚焦 "+FOCUS+" ==");
      L.push("  墨量 "+(fs/G[FOCUS].length/255).toFixed(3)+
             "（全库均值 "+(ink.reduce((s,o)=>s+o.v,0)/ink.length).toFixed(3)+"）");
      fp.slice(0,TOPN).forEach(o=>L.push("  r="+o.r.toFixed(3)+"   "+o.o));
      L.push("  最高 "+fp[0].r.toFixed(3)+"  最低 "+fp[fp.length-1].r.toFixed(3)+
             "  共 "+fp.length+" 个对手（仅列出前 "+TOPN+"）");
      // --vs：精确查某一对。**没有它时不要拿「未出现在前 N」当 r=0** ——
      // 那是最容易被误读成「完全不撞」的地方（实测踩过两次）。
      if(VS){
        const vi=fp.findIndex(o=>o.o===VS);
        L.push(vi<0 ? "  vs "+VS+": 该图标不存在"
                    : "  vs "+VS+": r="+fp[vi].r.toFixed(3)+"   排名 "+(vi+1)+"/"+fp.length);
      }
    } else L.push("!! 聚焦图标未找到: "+FOCUS);
    L.push("");
  }
  const k=N.map(n=>({n,k:C[n].length,s:C[n]})).sort((a,b)=>b.k-a.k);
  L.push("== 连通分量（笔画断开会拆块；中位 "+k.map(x=>x.k).sort((a,b)=>a-b)[Math.floor(N.length/2)]+"）==");
  k.slice(0,10).forEach(o=>L.push("  "+o.n.padEnd(24)+" 分量 "+String(o.k).padStart(2)+
      "  面积 "+o.s.slice(0,8).join(",")));
  const fr=k.filter(o=>o.s.some(x=>x>0&&x<=3));
  L.push("  含极小碎片(<=3px): "+(fr.length?fr.map(o=>o.n).join(", "):"无"));
  document.getElementById('out').textContent=L.join("\n");
})();
</script></body></html>"""


def main():
    argv = [a for a in sys.argv[1:] if not a.startswith("--")]
    d = argv[0] if argv else "assets/icons"
    focus = None
    if "--focus" in sys.argv:
        i = sys.argv.index("--focus")
        if i + 1 < len(sys.argv):
            focus = sys.argv[i + 1]
    topn = 12
    if "--top" in sys.argv:
        i = sys.argv.index("--top")
        if i + 1 < len(sys.argv) and sys.argv[i + 1].isdigit():
            topn = int(sys.argv[i + 1])
    vs = None
    if "--vs" in sys.argv:
        i = sys.argv.index("--vs")
        if i + 1 < len(sys.argv):
            vs = sys.argv[i + 1]
    if not os.path.isdir(d):
        print("!! 目录不存在:", d)
        return 1
    items = []
    for p in sorted(glob.glob(os.path.join(d, "*.svg"))):
        items.append({"n": os.path.basename(p)[:-4],
                      "s": open(p, encoding="utf-8").read().strip()})
    print("图标数:", len(items))

    br = find_browser()
    if not br:
        print("!! 未找到 Edge/Chrome")
        return 1
    page = HTML.replace("__TOPN__", str(topn))
    page = page.replace("__VS__", json.dumps(vs, ensure_ascii=False))
    page = page.replace("__FOCUS__", json.dumps(focus, ensure_ascii=False))
    page = page.replace("__ITEMS__", json.dumps(items, ensure_ascii=False))
    tmp = os.path.join(tempfile.gettempdir(), "icon-sim.html")
    open(tmp, "w", encoding="utf-8").write(page)
    url = "file:///" + tmp.replace("\\", "/").lstrip("/")

    r = subprocess.run([br, "--headless=new", "--disable-gpu",
                        "--virtual-time-budget=30000", "--dump-dom", url],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="ignore")
    m = re.search(r'<pre id="out">(.*?)</pre>', r.stdout, re.S)
    if not m:
        print("!! 未取到结果（浏览器执行失败）")
        return 1
    print(_html.unescape(m.group(1)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
