# -*- coding: utf-8 -*-
"""Regenerate docs/design/icons-preview.html as a flat, filename-sorted grid.

- Cards are generated straight from assets/icons/*.svg (sorted by name).
- desc/code dictionaries carry over the hand-written notes from the previous
  page; new icons fall back to "—" code + desc from NEW_DESC below.
- Each card is an <a> that opens the actual svg file in a new tab.
"""
import html
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
ICON_DIR = ROOT / "assets/icons"
OUT = ROOT / "docs/design/icons-preview.html"

# ---- hand-written metadata: code (Material codepoint or —) + desc ----
NEW_DESC = {
    "clipboard-check": "粘贴确认弹框头部（clipboard+对勾）",
    "clock": "历史/最近记录（搜索面板 chip、发送命令窗）",
    "corner-down-left": "回车执行角标（搜索面板行尾 ↵）",
    "disk": "磁盘/存储（粘贴确认弹框新增注册）",
    "file-archive": "压缩包（SFTP 列表按扩展名给类型图标）",
    "file-code": "程序/脚本（SFTP 列表按扩展名给类型图标）",
    "file-data": "结构化数据/清单（SFTP 列表按扩展名给类型图标）",
    "file-db": "数据库（SFTP 列表按扩展名给类型图标）",
    "file-image": "图片文件（SFTP 列表按扩展名给类型图标）",
    "file-font": "字体文件（SFTP 列表按扩展名给类型图标）",
    "file-log": "日志文件（SFTP 列表按扩展名给类型图标）",
    "file-markdown": "Markdown 文档（SFTP 列表按扩展名给类型图标）",
    "file-table": "表格文件（SFTP 列表按扩展名给类型图标）",
    "file-x": "文件失效/不可用（预留）",
    "hash": "范围徽标「已选主机数」（发送命令窗）",
    "hscroll": "横向滚动（预留）",
    "layers": "分层/作用域（预留·发送命令窗 v2）",
    "library": "命令库入口（发送命令窗标题栏）",
    "logo-m": "MTerm 徽标源图（窗内左上角/应用图标，512 网格）",
    "permission": "权限钥匙（SFTP 权限菜单 #permission-icon）",
    "retry": "重试（发送命令窗投递失败重试）",
    "send": "发送（发送命令窗投递按钮）",
    "shield-alert": "防火墙告警（粘贴确认弹框 bracketed 徽标）",
    "shield": "防火墙拦截（粘贴确认弹框）",
    "star": "五角星收藏（实心填充）",
    "swap-vert": "传输管理入口（标题栏，上下双箭头）",
    "tabs": "全部标签页（发送命令窗范围徽标）",
    "trash": "删除徽标（粘贴确认弹框递归删除类别）",
    "wrap": "软折行开关（粘贴确认弹框）",
}

def extract_old_meta(doc: str):
    """Pull (name -> code, desc) out of the existing preview page."""
    meta = {}
    # Cards are <a class="card"> … </a> (the page was refactored from <div class="card">;
    # the old regex silently matched nothing and wiped every hand-written note).
    for card in re.findall(r'<a class="card"[^>]*>(.*?)\n</a>', doc, flags=re.S):
        m = re.search(r'<div class="name">([^<]+)</div>', card)
        if not m:
            continue
        name = m.group(1).strip().removesuffix(".svg")
        code_m = re.search(r'<div class="code">([^<]*)</div>', card)
        desc_m = re.search(r'<div class="desc">([^<]*)</div>', card)
        meta[name] = (code_m.group(1).strip() if code_m else "—",
                      desc_m.group(1).strip() if desc_m else "")
    return meta

old_doc = OUT.read_text(encoding="utf-8")
meta = extract_old_meta(old_doc)

def card_html(name: str, svg: str) -> str:
    code, desc = meta.get(name, ("—", ""))
    if name in NEW_DESC and not desc:
        desc = NEW_DESC[name]
    desc = desc or "—"
    esc_name = html.escape(name)
    return f'''<a class="card" href="../../assets/icons/{esc_name}.svg" target="_blank" rel="noopener" title="打开 {esc_name}.svg">
  <div class="big">{svg}</div>
  <div class="row"><span class="s16">{svg}</span><span class="s24">{svg}</span></div>
  <div class="name">{esc_name}.svg</div>
  <div class="code">{html.escape(code)}</div>
  <div class="desc">{html.escape(desc)}</div>
</a>'''

svgs = {}
for p in sorted(ICON_DIR.glob("*.svg"), key=lambda p: p.stem.lower()):
    svgs[p.stem] = p.read_text(encoding="utf-8").strip()

cards = "\n\n".join(card_html(n, s) for n, s in svgs.items())

page = f'''<!doctype html>
<html lang="zh">
<head>
<meta charset="utf-8">
<title>MTerm · 项目图标预览（高保真）</title>
<!-- 由 scripts/gen_icons_preview.py 从 assets/icons/*.svg 生成（按文件名排序、不分组）。
     手写注记（码位/描述）维护在脚本的 meta 字典里；直接改本页会被下次生成覆盖。 -->
<style>
  :root {{ color-scheme: dark; }}
  * {{ box-sizing: border-box; }}
  body {{ margin: 0; padding: 24px 28px 60px; background: #14161b; color: #e8e8ea;
         font: 13px/1.5 "Segoe UI", "Microsoft YaHei", sans-serif; }}
  h1 {{ font-size: 20px; margin: 0 0 4px; }}
  .sub {{ color: #9aa0ab; margin-bottom: 18px; max-width: 860px; }}
  .grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(148px, 1fr)); gap: 10px; }}
  a.card {{ background: #1e2128; border: 1px solid #2a2e37; border-radius: 12px;
          padding: 14px 12px 10px; text-align: center; transition: .12s;
          display: block; text-decoration: none; color: inherit; cursor: zoom-in; }}
  a.card:hover {{ border-color: #4c8bf5; background: #232733; }}
  .big svg {{ width: 46px; height: 46px; }}
  .row {{ display: flex; gap: 14px; justify-content: center; align-items: flex-end;
         margin: 10px 0 8px; color: #9aa0ab; }}
  .row .s16 svg {{ width: 16px; height: 16px; }}
  .row .s24 svg {{ width: 24px; height: 24px; }}
  .name {{ font-family: Consolas, monospace; font-size: 11.5px; color: #d7dae0; word-break: break-all; }}
  .code {{ font-family: Consolas, monospace; font-size: 10.5px; color: #6f7683; margin-top: 2px; }}
  .desc {{ font-size: 11.5px; color: #9aa0ab; margin-top: 4px; }}
  body.light {{ background: #f5f5f7; color: #22252b; }}
  body.light a.card {{ background: #fff; border-color: #e3e5ea; }}
  body.light a.card:hover {{ border-color: #4c8bf5; }}
  body.light .name {{ color: #33363c; }}
  body.light .code, body.light .desc, body.light .sub {{ color: #7a8090; }}
  #toggle {{ position: fixed; right: 22px; top: 20px; padding: 7px 14px; border-radius: 8px;
            border: 1px solid #3a3f4a; background: #1e2128; color: #e8e8ea; cursor: pointer; }}
  body.light #toggle {{ background: #fff; color: #22252b; border-color: #c9cdd6; }}
  .legend {{ color: #7fb0ff; }}
  body.light .legend {{ color: #2f6fd0; }}
</style>
</head>
<body>
<h1>MTerm 项目图标</h1>
<div class="sub">{len(svgs)} 个图标 —— 与 <code>assets/icons/</code> 目录一一对应，按文件名排序。
每张卡三档展示：46px 大图 + 16px / 24px 实际使用尺寸；码位为原始 Material Icons 码位，<code>—</code> 表示项目自定图标。
<span class="legend">点击任意卡片即在浏览器中打开对应的 svg 源文件；右上按钮切换明暗底。</span></div>
<button id="toggle">切换明暗底</button>
<div class="grid">
{cards}
</div>
<script>
  document.getElementById('toggle').addEventListener('click', () => {{
    document.body.classList.toggle('light');
  }});
</script>
</body>
</html>
'''

OUT.write_text(page, encoding="utf-8")
print(f"written {OUT}: {len(svgs)} cards")
