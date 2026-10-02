#!/usr/bin/env bash
# Generate the hi-fi icon preview page (docs/design/) from assets/icons/.
set -e
cd "$(dirname "$0")"
OUT="../../docs/design/icons-preview.html"
# 卡片先累积在变量里再一次性写出 —— 不依赖 mktemp/rm，Windows 下也能跑。
CARDS=""

# row <group> <file-without-ext> <codes> <desc>
row() {
  local g="$1" f="$2" codes="$3" desc="$4"
  if [ "$g" != "$CUR" ]; then
    [ -n "$CUR" ] && CARDS+="</div>"$'\n'
    CARDS+="<h2>$g</h2>"$'\n'
    CARDS+='<div class="grid">'$'\n'
    CUR="$g"
  fi
  local svg; svg=$(cat "$f.svg")
  CARDS+="<div class=\"card\">
  <div class=\"big\">$svg</div>
  <div class=\"row\"><span class=\"s16\">$svg</span><span class=\"s24\">$svg</span></div>
  <div class=\"name\">$f.svg</div>
  <div class=\"code\">$codes</div>
  <div class=\"desc\">$desc</div>
</div>
"$'\n'
}

cat > "$OUT" <<'HEAD'
<!doctype html>
<html lang="zh">
<head>
<meta charset="utf-8">
<title>MeatShell · 项目图标预览（高保真）</title>
<style>
  :root { color-scheme: dark; }
  * { box-sizing: border-box; }
  body { margin: 0; padding: 24px 28px 60px; background: #14161b; color: #e8e8ea;
         font: 13px/1.5 "Segoe UI", "Microsoft YaHei", sans-serif; }
  h1 { font-size: 20px; margin: 0 0 4px; }
  .sub { color: #9aa0ab; margin-bottom: 18px; max-width: 860px; }
  h2 { font-size: 15px; margin: 26px 0 10px; color: #cfd3da;
       border-left: 3px solid #4c8bf5; padding-left: 9px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(148px, 1fr)); gap: 10px; }
  .card { background: #1e2128; border: 1px solid #2a2e37; border-radius: 12px;
          padding: 14px 12px 10px; text-align: center; transition: .12s; }
  .card:hover { border-color: #4c8bf5; background: #232733; }
  .big svg { width: 46px; height: 46px; }
  .row { display: flex; gap: 14px; justify-content: center; align-items: flex-end;
         margin: 10px 0 8px; color: #9aa0ab; }
  .row .s16 svg { width: 16px; height: 16px; }
  .row .s24 svg { width: 24px; height: 24px; }
  .name { font-family: Consolas, monospace; font-size: 11.5px; color: #d7dae0; word-break: break-all; }
  .code { font-family: Consolas, monospace; font-size: 10.5px; color: #6f7683; margin-top: 2px; }
  .desc { font-size: 11.5px; color: #9aa0ab; margin-top: 4px; }
  body.light { background: #f5f5f7; color: #22252b; }
  body.light .card { background: #fff; border-color: #e3e5ea; }
  body.light .card:hover { border-color: #4c8bf5; }
  body.light .name { color: #33363c; }
  body.light .code, body.light .desc, body.light .sub, body.light h2 { color: #7a8090; }
  #toggle { position: fixed; right: 22px; top: 20px; padding: 7px 14px; border-radius: 8px;
            border: 1px solid #3a3f4a; background: #1e2128; color: #e8e8ea; cursor: pointer; }
  body.light #toggle { background: #fff; color: #22252b; border-color: #d5d8de; }
  .legend { display: inline-block; margin-left: 14px; color: #6f7683; font-size: 12px; }
</style>
</head>
<body>
<h1>MeatShell · 项目图标预览</h1>
HEAD

# 图标数量动态取，避免改了图标库忘了同步文案（曾长期停留在过期数字）。
N=$(ls *.svg | wc -l | tr -d ' ')
printf '<div class="sub">%s 个线性图标 —— 项目中<b>已使用</b>或<b>待接入</b>的图标全集（24 网格 · 2px 圆头描边 · currentColor 染色）。\n每张卡片标注了对应的原始 Material Icons 码位与用途；码位显示为 <code>—</code> 的是为后续功能预留的新增图标。\n每卡三档展示：46px 大图 + 16px / 24px 实际使用尺寸。<span class="legend">悬停查看效果；右上按钮切换明暗底。</span></div>\n' "$N" >> "$OUT"

cat >> "$OUT" <<'HEAD'
<button id="toggle">切换明暗底</button>
HEAD

CUR=""
row "常用" close "E5CD" "关闭"
row "常用" check "E5CA / E876" "对勾（选中·勾选；亦作「Connection OK」连通性结果）"
row "常用" folder "E2C7" "文件夹（收起组 / SFTP 目录）"
row "常用" folder-open "E2C8 / E2C6" "文件夹打开（展开组；E2C6 亦作壁纸面板占位图）"
row "常用" expand-more "E5CF" "向下展开"
row "常用" expand-less "E5CE" "向上收起"
row "常用" chevron-right "E5CC" "右箭头（折叠组）"
row "常用" chevron-left "E5CB" "左箭头（面板收起）"
row "常用" delete "E872" "删除"
row "常用" add "E145" "加号 / 新建"
row "常用" remove "E15B" "最小化（标题栏）/ 减号（步进器）"
row "常用" no-color "—" "清除颜色 / 无颜色（色样被划掉 · 分组颜色面板底部按钮）"
row "常用" search "E8B6" "搜索"
row "常用" refresh "E5D5" "刷新"
row "常用" edit "E3C9" "编辑（右键菜单）"
row "编辑与剪贴板" copy "E14D" "复制"
row "编辑与剪贴板" cut "E14B" "剪切（库内预留 · 原「无颜色」按钮误用，2026-10-02 换成 no-color）"
row "编辑与剪贴板" paste "E14F" "粘贴"
row "编辑与剪贴板" select-all "E14C" "全选"
row "编辑与剪贴板" notebook-pen "—" "笔记 / 备注（带装订线的本子 + 正在书写的笔 · 库内预留）"
row "会话操作" new-session "E879" "新建会话"
row "会话操作" session-disconnect "—" "断开会话（新增备用）"
row "会话操作" session-reconnect "—" "重连会话（新增备用）"
row "菜单操作" unfold-more "E5D7" "全部展开"
row "菜单操作" unfold-less "E5D6" "全部折叠"
row "菜单操作" move-to-group "E9A1" "移动到 · 分区标题（会话右键菜单，双文件夹）"
row "菜单操作" palette "E40A" "分组颜色"
row "菜单操作" tune "E429" "设置（菜单条目 —— 与顶栏 settings 同为「设置」）"
row "侧栏导航" home "E88A" "欢迎页"
row "侧栏导航" keyboard "E312" "输入"
row "侧栏导航" pencil "E167" "显示与字体（设置侧栏 NavItem）"
row "侧栏导航" rendering "E1B1" "渲染（设置侧栏 NavItem）"
row "侧栏导航" output-highlight "E8D4" "输出高亮（三行 + 中行描边框；原为半透明填充）"
row "侧栏导航" wallpaper "E1BC" "壁纸"
row "侧栏导航" cloud "E2C2" "WebDAV"
row "侧栏导航" computer "E30A" "WSL"
row "侧栏导航" plug "E335" "MCP（插头/连接器，ZCode 风格）"
row "侧栏导航" system-update "E923" "新版本提示（双向循环箭头）"
row "文件与 SFTP" arrow-down "E5DB" "↓ 方向标（传输列表 / 弹层 / 侧栏速率）· 导入连接（设置菜单）"
row "文件与 SFTP" arrow-up "E5D8" "↑ 方向标（同上）· 导出连接（设置菜单）· 上级目录（SFTP 路径栏）"
row "文件与 SFTP" upload "—" "上传文件 / 文件夹（SFTP 工具栏 · 箭头→横线）"
row "文件与 SFTP" download "E2C4" "下载选中（SFTP 工具栏 / 设置侧栏「下载」）"
row "文件与 SFTP" arrow-right "—" "→ 右向箭头（库内预留；ui 里的 → 均为句内标点，暂无图标槽）"
row "文件与 SFTP" arrow-left "—" "← 左向箭头（库内预留 · 与 arrow-right 镜像；四向齐备）"
row "文件与 SFTP" create-new-folder "E2CC" "新建分组（组 / 空白处右键菜单）"
row "文件与 SFTP" file "E24D" "普通文件"
row "文件与 SFTP" table-rows "E1C3" "批量导入(文本)（设置菜单）"
row "状态与反馈" visibility "E8F4" "显示 / 隐藏本地终端（另用于密码可见性）"
row "状态与反馈" visibility-off "E8F5" "同上（E8F4 / E8F5 成对开关）"
row "状态与反馈" error "E000" "错误（主机/串口缺失）"
row "状态与反馈" warning "—" "警告（替换 confirm_dialog 的字面量 ⚠）"
row "状态与反馈" info "E88E" "关于（About · 设置菜单）"
row "状态与反馈" more-vert "E5D4" "更多（竖三点）"
row "状态与反馈" checkbox "E834" "复选框"
row "状态与反馈" dashboard "E871" "资源面板"
row "状态与反馈" moon "E51C" "暗色主题"
row "状态与反馈" sun "E518" "亮色主题"
row "窗口与面板" open-in-full "E3C2" "缩放展开手柄"
row "窗口与面板" display "—" "显示 / Show（托盘菜单；原 monitor，形状不变 · 与 minimize-to-tray 成对）"
row "窗口与面板" minimize-to-tray "E882" "最小化到托盘（窗口翻到后台 / 收起 · flip_to_back 形）"
row "窗口与面板" fullscreen-enter "—" "进入全屏（四角括号 · 角在外）"
row "窗口与面板" fullscreen-exit "—" "退出全屏（四角括号 · 角在内）"
row "窗口与面板" sidebar-left-expand "—" "侧栏展开（左）"
row "窗口与面板" sidebar-left-collapse "—" "侧栏收起（左）"
row "窗口与面板" sidebar-right-expand "—" "侧栏展开（右）"
row "窗口与面板" sidebar-right-collapse "—" "侧栏收起（右）"
row "窗口与面板" split-horizontal "—" "水平拆分视图（H · 左右并排，待定）"
row "窗口与面板" split-vertical "—" "垂直拆分视图（V · 上下堆叠，待定）"
row "窗口与面板" maximize "E835" "最大化（标题栏）"
row "窗口与面板" restore "E3E0" "还原（标题栏，方框+弧线）"
row "窗口与面板" settings "E8B8" "设置（顶栏按钮）"
row "窗口与面板" swap-vert "E8D5" "传输（上下双箭头 · 顶栏按钮）"
row "窗口与面板" bolt "E3E7" "快捷命令 · 停靠条入口按钮（点击展开面板）"
row "窗口与面板" command-bar "EA34" "命令栏 · 顶栏开关（无框提示符，与 terminal 靠外框区分）"
row "窗口与面板" drag-grip "E25D" "快捷命令面板 · 标题栏 + 拖拽手柄（面板内部）"
row "窗口与面板" resize-se "—" "右下缩放角柄（三点角三角 · 角锚定非居中 · 8 处共用；替换 ◢ 与 E3C2）"
row "终端与会话" command-history "—" "命令历史（回退弧 + 时钟）"
row "终端与会话" play "E037" "运行历史命令条目（圆形播放）"
row "终端与会话" connectivity-test "EF48" "测试连通性（脉冲波形）"
row "终端与会话" tunnel "E0B6" "端口转发 / 隧道（Tunnels 面板标签 · 窄工具栏）"
row "终端与会话" link "E627" "同步（会话同步 / 同步输入，链环）"
row "终端与会话" input "E890" "导入 ~/.ssh/config（文件 + 向内箭头）"
row "终端与会话" language "E894" "中 ⇄ 英 切换（A / 文 并列）"
row "终端与会话" quit "E8AC" "完全退出（开口容器 + 穿出箭头 · 关闭主窗口弹框）"
row "协议图标" terminal "EB8E" "SSH / 本地 · 协议回落（终端窗+提示符）"
row "协议图标" serial-port "E8C0" "串口（D-sub 接头+针脚）"
row "协议图标" window-asset "E069" "Telnet（浏览器窗+控制点）"
row "协议图标" remote-desktop "E30C" "RDP / VNC · 远程桌面（并排双屏 · 与 display 靠轮廓区分）"

CARDS+="</div>"$'\n'
printf '%s' "$CARDS" >> "$OUT"

cat >> "$OUT" <<'TAIL'
<script>
document.getElementById('toggle').onclick = function () {
  document.body.classList.toggle('light');
};
</script>
</body>
</html>
TAIL

echo "generated: $OUT ($(wc -c < "$OUT") bytes)"
