#!/usr/bin/env bash
# Generate the modern (Lucide-style) icon set for #icon-svg-migration.
# One file per glyph, 24x24 grid, 2px round-cap stroke, currentColor.
set -e
cd "$(dirname "$0")"

mk() { cat > "$1.svg" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">$2</svg>
EOF
}

# 共享骨架 —— 文件夹 / 会话框只定义一次，避免家族几何分叉。
# 会话框 15×17，右上开口；角标圆心与框的右上角顶点 (18,5) 重合，角标填补该缺口。
FOLDER='<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>'
SESSION='<path d="M12 5H6a3 3 0 0 0-3 3v11a3 3 0 0 0 3 3h9a3 3 0 0 0 3-3V11"/>'

mk close '<path d="M18 6 6 18M6 6l12 12"/>'
mk check '<path d="M20 6 9 17l-5-5"/>'
mk folder "$FOLDER"
mk folder-open '<path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/>'
mk expand-more '<path d="m6 9 6 6 6-6"/>'
mk expand-less '<path d="m18 15-6-6-6 6"/>'
mk chevron-right '<path d="m9 6 6 6-6 6"/>'
mk chevron-left '<path d="m15 6-6 6 6 6"/>'
mk delete '<path d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m3 0-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6M10 11v6M14 11v6"/>'
mk copy '<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>'
mk cut '<circle cx="6" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M20 4 8.12 15.88M14.47 14.48 20 20M8.12 8.12 12 12"/>'
mk select-all '<rect x="3" y="3" width="18" height="18" rx="2" stroke-dasharray="4 3"/><rect x="8" y="8" width="8" height="8" rx="1" fill="currentColor" stroke="none"/>'
mk paste '<rect x="8" y="2" width="8" height="4" rx="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>'
mk add '<path d="M12 5v14M5 12h14"/>'
mk remove '<path d="M5 12h14"/>'
# 「清除颜色 / 无颜色」（分组颜色面板底部的按钮，app.slint:235）。
# 原先借用 `cut`（剪刀），是 Material content_cut 的遗留 —— 剪刀与"清除颜色"毫无关系，
# 2026-10-02 换掉（用户截图）。全库仅此一处用到 cut，改用后 cut 成为库内预留。
#
# 选型（实测数据，候选在完整库里测）：
#   方形色样 + **出头**斜线   墨量 0.256  最相似 serial-port 0.509  ← 采用
#   小圆 + 长斜线             墨量 0.183  最相似 close 0.473，但读作"禁止符号"
#   三条色带 + 斜线           墨量 0.174  最相似 add 0.507，13px 下读作"列表"
#   大圆角方块 + 斜线         撞 maximize 0.864 —— 出局
# 两条要点：① 斜线**跨出方框**（出头）比压在框内更像"划掉"，且把与 serial-port
# 的相似度从 0.73 压到 0.51；② 方块一放大就撞 maximize（与 fullscreen 那次
# "撑满画面的方括号不能用方形外框"是同一条判据）。
mk no-color '<rect x="5" y="5" width="14" height="14" rx="1"/><path d="M3.5 20.5 20.5 3.5"/>'
mk search '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>'
# ── 箭头族 ────────────────────────────────────────────────────────────────
# E5D8 / E5DB 在 Material 里就是 arrow_upward / arrow_downward（纯箭头）。这两个码位共 8 处用途，
# 其中 6 处是**方向标**（传输列表的绿/紫方向、传输弹层 16px 槽、侧栏 ↑上传/↓下载速率），
# 只有 2 处是设置菜单的「导出连接 / 导入连接」。方向标要的是干净箭头 ——
# 原先的「托盘 + 箭头」把「方向」说成了「下载按钮」，且托盘形与 minimize-to-tray 撞到 0.753。
mk arrow-up '<path d="M12 21V3m0 0-8 8m8-8 8 8"/>'
# ⚠️ 原 parent-directory（SFTP 上级目录，sftp_panel.slint 两处）与 arrow-up **完全同形**
#（实测相关度 1.000），已于 2026-10-02 并入本形并删除 —— 同一个"向上"语义复用同一形状。
# 若再要新增"上级目录"图标，先确认不是在造重复品。
mk arrow-down '<path d="M12 3v18m0 0-8-8m8 8 8-8"/>'
# 右向箭头（库内预留）。与 arrow-up / arrow-down 严格同族：轴长 18、臂 8,8，
# 把 arrow-up 的坐标轴互换即得本形（严格 90° 旋转），三者并排不会大小不一。
# 注意：`chevron-right`(E5CC) 是「折叠 / 展开器」语义（细 V 形，无箭杆），
# 与本图标「前进 / 指向」不同义 —— 实测两者 16px 相关度仅 0.016~0.015。
# ⚠️ 现状 ui/ 里所有 `→` 都是**句内标点**（拼在字符串里的 "A → B"、@"→ {} · #{}"），
#    没有任何图标槽需要它；本文件因此属"库内预留"，不是待替换项。
mk arrow-right '<path d="M3 12h18m0 0-8-8m8 8-8 8"/>'
# 左向箭头（库内预留）。四向到此齐备：up/down/right/left 轴长均 18、臂均 8,8，
# 两两互为 90° 旋转或镜像，并排不会大小不一。
# 同 arrow-right：现状 ui/ 无图标槽需要它（`←` 未出现在界面文案里），属预留。
mk arrow-left '<path d="M21 12H3m0 0 8-8m-8 8 8 8"/>'
# 传输动作（SFTP 工具栏的上传 / 下载、下载弹层）。约定只有一条：**横线＝目的地，箭头指向它** ——
# 上传时线在上方（远端），下载时线在下方（本地）。＝ Lucide arrow-up-to-line / arrow-down-to-line。
# 用「末端一条线」而非托盘：↑↓ 两版互撞从 0.692 降到 0.326（托盘是两版共用的最大一块面积）。
mk upload '<path d="M12 21V7m0 0-5 5m5-5 5 5M5 3h14"/>'
# E2C4（Material file_download）：SFTP 下载按钮（sftp_panel.slint:649）+ 设置侧栏「下载」（interface_panel.slint:746）。
mk download '<path d="M12 3v14m0 0-5-5m5 5 5-5M5 21h14"/>'
mk create-new-folder "$FOLDER"'<path d="M12 9.5v7M8.5 13h7"/>'
# 数据目录设置页「恢复默认目录」按钮（2026-10-04 新增）。
# 语义 = 目录回到默认，故**复用 $FOLDER 骨架**并在内部放复位箭头 —— 与 folder / folder-open
# 的区别只在内部符号。实测：与 folder 0.44，全库最高 0.515（未撞形）。
# 箭头长度是调过的：6.5 格偏短不协调，9.5 格会让 11px 与文件夹左壁合并，定在 8 格（12–20px 全档 ok）。
mk folder-reset "$FOLDER"'<path d="M17.5 13.5H9.5"/><path d="M13 11 9.5 13.5 13 16"/>'
# 密码可见性 / 显示·隐藏本地终端（app.slint:3394 的 E8F4/E8F5 开关，widgets.slint 密码框也用它）。
# 原杏仁形两端是**尖角**（`s3-8` 三次曲线的端点就是拐点），16px 下是两根硬刺；
# 改用 Lucide eye 的圆端收尾（`a1 1 0 0 1 0-.696`）。两版互撞 0.671 → 0.582。
mk visibility '<path d="M2.06 12.35a1 1 0 0 1 0-.7 10.75 10.75 0 0 1 19.88 0 1 1 0 0 1 0 .7 10.75 10.75 0 0 1-19.88 0"/><circle cx="12" cy="12" r="3"/>'
# 原斜线 `M2 2l20 20` 画满对角线，视觉重量明显重于可见版（0.276 vs 0.233）；
# 改 Lucide eye-off 的**断开弧 + 斜线**，两版重量差从 0.043 收到 0.026。
mk visibility-off '<path d="M10.73 5.08A10.74 10.74 0 0 1 21.94 11.65a1 1 0 0 1 0 .7 10.75 10.75 0 0 1-1.44 2.49M14.08 14.16a3 3 0 0 1-4.24-4.24M17.48 17.5a10.75 10.75 0 0 1-15.42-5.15 1 1 0 0 1 0-.7 10.75 10.75 0 0 1 4.45-5.14M2 2l20 20"/>'
mk dashboard '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>'
mk link '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>'
mk more-vert '<circle cx="12" cy="5" r="1.7" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1.7" fill="currentColor" stroke="none"/><circle cx="12" cy="19" r="1.7" fill="currentColor" stroke="none"/>'
mk swap-vert '<path d="m8 3 4 4-4 4M12 7H4m12 14-4-4 4-4M12 17h8"/>'
# 输出高亮（设置侧栏 NavItem「输出高亮 / Output」，interface_panel.slint:706，码位 E8D4）。
# 语义：高亮纯文本终端输出 / 高亮整行 —— 三行文字中把中间那行框出来。
# 【为何重做】原用 fill + opacity=".35" 做高亮条，是**全库唯一**使用半透明填充的图标 ——
#   图标被染色 / 走 SVG 迁移后，半透明的合成结果与其它图标不一致。改为描边框，语义不变。
mk output-highlight '<path d="M6 5h15M6 19h8"/><rect x="3" y="9" width="18" height="6" rx="1.5"/>'
# 关于（About，app.slint:4817 —— 预览页原写"信息提示"，实际菜单标签是 About/关于）。
# 原 i 的点用 `M12 8h.01`（靠圆头描边成形）：32px 下连通块仅 6px²，换算到 16px 约 0.67px。
# 改为实心圆 r=1.2 + 竖杆 4→5 单位：与 error 的 16px 相关度 0.862 → 0.846。
# ⚠️ 不要动外圆半径：曾试把 r=10 收到 r=9（想与 error 拉开、尺寸回到中位 20×20），
# 结果与 `play`（同为 r=9 圆 + 内部符号）撞到 **0.900**，反而制造了新撞车。
mk info '<circle cx="12" cy="12" r="10"/><path d="M12 17v-5"/><circle cx="12" cy="8" r="1.2" fill="currentColor" stroke="none"/>'
mk home '<path d="M3 10.5 12 3l9 7.5V20a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M9 22v-8h6v8"/>'
mk moon '<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>'
mk sun '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/>'
mk keyboard '<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8"/>'
mk pencil '<path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/>'
mk edit '<path d="M12 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-6"/><path d="M18.4 2.6a2.1 2.1 0 0 1 3 3L13 14l-4 1 1-4 8.4-8.4z"/>'
# 笔记本 / 笔记（Lucide notebook-pen 上游原形）。喻体是「带装订点的本子 + 正在书写的笔」——
# 与 pencil（纯笔）、edit（方框 + 笔）区分开：本子表达"记录内容"，笔表达"正在写"。
# 取舍（用户选 A，即上游原形·无脊线）：曾试过补一条脊线 M8 2v20，墨量 0.392（全库最重），
# 且与 file 的 16px 相关度 0.535（脊线确是"这是本子"的判据）；但参考形观感偏重，
# 无脊线降到 0.332，代价是与 file 升到 0.655（仍低于 p99 0.746）。
mk notebook-pen '<path d="M13.4 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7.4"/><path d="M2 6h4"/><path d="M2 10h4"/><path d="M2 14h4"/><path d="M2 18h4"/><path d="M21.378 5.626a1 1 0 1 0-3.004-3.004l-5.01 5.012a2 2 0 0 0-.506.854l-.837 2.87a.5.5 0 0 0 .62.62l2.87-.837a2 2 0 0 0 .854-.506z"/>'
mk new-session "$SESSION"'<path d="M18 2v6M15 5h6"/>'
mk session-disconnect "$SESSION"'<path d="M15.74 2.74 20.26 7.26M20.26 2.74 15.74 7.26"/>'
mk session-reconnect "$SESSION"'<circle cx="18" cy="5" r="3"/>'
mk unfold-less '<path d="m7 20 5-5 5 5M7 4l5 5 5-5"/>'
mk unfold-more '<path d="m7 15 5 5 5-5M7 9l5-5 5 5"/>'
mk palette '<circle cx="12.95" cy="7.24" r=".86" fill="currentColor" stroke="none"/><circle cx="16.76" cy="11.05" r=".86" fill="currentColor" stroke="none"/><circle cx="8.19" cy="8.19" r=".86" fill="currentColor" stroke="none"/><circle cx="6.29" cy="12.95" r=".86" fill="currentColor" stroke="none"/><path d="M11.52 2.96C6.29 2.96 2 7.24 2 12.48s4.28 9.52 9.52 9.52c0.89 0 1.67-.69 1.67-1.67 0-.44-.17-.82-.44-1.11-.27-.3-.44-.67-.44-1.09 0-.92.74-1.67 1.67-1.67H15.33c3.68 0 6.66-2.98 6.66-6.66 0-4.72-4.72-7.8-10.47-7.8z"/>'
# 「移动到」分区标题（会话右键菜单 app.slint:3211）。原形 Material drive_file_move(E9A1)。
# 演进：folder + 中下箭头 → 双文件夹。理由是**同屏关系**：该标题正下方 30px 就是那排组
# 条目，用的是普通 folder(E2C7)，两者同 16px 槽、同左缘；旧形与它 16px 相关度 0.884，
# 标题读起来像多出来的一条组条目。改双文件夹后降到 0.463。
# 注：Lucide folder-input（左开口 + 外箭头）才是主流「移入」标准形，但实测只降到 0.818 —
# 共享的文件夹轮廓主导了相关度，开个口/加箭头改变的面积太小（把箭头加长反而回升到 0.872）。
# 有效手段是换轮廓与重心，不是改内部符号。这也意味着此处**不再复用 $FOLDER 骨架**。
mk move-to-group '<path d="M20 5a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h2.5a1.5 1.5 0 0 1 1.2.6l.6.8a1.5 1.5 0 0 0 1.2.6z"/><path d="M3 8.27a2 2 0 0 0-1 1.74V19a2 2 0 0 0 2 2h11a2 2 0 0 0 1.73-1"/>'
mk tune '<path d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3M2 14h5M9 8h6M17 16h5"/>'
mk error '<circle cx="12" cy="12" r="10"/><path d="m15 9-6 6M9 9l6 6"/>'
# 警告（状态与反馈；替换 confirm_dialog.slint:109 的字面量 "⚠"）。
# 三态语义统一为「圆 + 内部符号」的兄弟形：error=圆+×、info=圆+i、warning=三角+!。
# 三角而非圆，是与另两者在轮廓上就分开（圆↔圆靠内部符号区分，16px 下易糊；
# 实测 warning vs error 0.085 / vs info 0.095，远低于 error↔info 的 0.862）。
# 竖杆与点分开、点用实心（与 info 的 r=1.2 一致，不用 h.01 —— 那在 16px 下只有约 0.7px）。
mk warning '<path d="M10.5 4.5 3 18a2 2 0 0 0 1.73 3h14.54A2 2 0 0 0 21 18l-7.5-13.5a2 2 0 0 0-3 0z"/><path d="M12 9.5v4"/><circle cx="12" cy="16.5" r="1.2" fill="currentColor" stroke="none"/>'
# 命令历史 —— 历史搜索面板入口（原仅 Ctrl+Shift+R，无 UI 图标）。
# 注意：回退弧 + L 形箭头与 refresh 形状同族，并排时留意是否会被误读为"刷新"。
mk command-history '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l4 2"/>'
mk play '<circle cx="12" cy="12" r="9"/><path d="m9.5 8.5 6 3.5-6 3.5z" fill="currentColor" stroke="none"/>'
mk drag-grip '<circle cx="8" cy="6" r="1.4" fill="currentColor" stroke="none"/><circle cx="16" cy="6" r="1.4" fill="currentColor" stroke="none"/><circle cx="8" cy="12" r="1.4" fill="currentColor" stroke="none"/><circle cx="16" cy="12" r="1.4" fill="currentColor" stroke="none"/><circle cx="8" cy="18" r="1.4" fill="currentColor" stroke="none"/><circle cx="16" cy="18" r="1.4" fill="currentColor" stroke="none"/>'
# 右下缩放角柄（#grip-icon-3dots 的单一事实源）。收敛掉此前并存的三套形态：
#   · app.slint:4270 / editor_window.slint:337 用的字体字符 "◢"（2 处）
#   · 6 处纯 Slint 画的 3 个 2×2px 方块（app.slint×4 / proc_window / system_info_window）
#   · E3C2 open_in_full（3 处注释备选，已删）
# 几何按 16px 框逐像素复现现状（scale = 24/16 = 1.5）：
#   点径 2px → 3 网格单位（r=1.5）；圆心 12/8px → 网格 18/12。
# **角锚定，非居中**：bbox x[9.5,20.5]，故 16px 框内直接贴右下角摆放即可对齐现状。
# 为何不用 open_in_full / 主流「三斜线」：二者在 6–12px 实测连通分量 1~2（应为 3），
# 即元素糊成一团；且 open_in_full 形与库内 open-in-full 撞 0.608。详见
# docs/design/resize-handle-2026-10-02.html §4。
mk resize-se '<circle cx="18" cy="18" r="1.5" fill="currentColor" stroke="none"/><circle cx="12" cy="18" r="1.5" fill="currentColor" stroke="none"/><circle cx="18" cy="12" r="1.5" fill="currentColor" stroke="none"/>'
# 快捷命令 · 停靠条入口按钮。喻体=闪电（面板名 Quick commands，强调"快"）。
# 实心不描边 → bbox 18×20 居中（原线性版 20×22 且 maxY 贴到安全区边界 23）。
# 其余两个喻体（终端提示符 >_ / 命令列表+播放）因与 terminal、output-highlight 撞族而弃用。
mk bolt '<path d="M13 3 4 14h8l-1 7 9-11h-8l1-7z" fill="currentColor" stroke="none"/>'
# 测试连通性（原 wifi-tethering，码位 EF48 保持不动）。
# 改名理由：按「功能」而非「形状」命名 —— wifi-tethering 是按 Material 形状起的名，
# 换成脉冲波形后名字就失真了；connectivity-test 与 session-disconnect 等同构（对象+动作）。
mk connectivity-test '<path d="M3 12h4.5l2.5-6 4 12 2.5-6H21"/>'
# 端口转发 / 隧道（SFTP 面板 Tunnels 标签，码位 E0B6 —— 代码已在用而库里原先缺，
# 属迁移会漏掉的码位）。两条平行导轨 + 穿过其中的箭头 = 「流量经通道抵达对端」。
# ⚠️ 导轨取 y=4/20 而非 5/19：5/19 版在**完整库**里与 table-rows 撞 0.656
#    （缺口分析页当时只对了 7 个同类图标，漏掉了这个对手），外扩一档后降到 0.473，
#    同时高度 16→18 更接近全库中位 20。
mk tunnel '<path d="M3 4h18M3 20h18"/><path d="M7 12h10m0 0-3-3m3 3-3 3"/>'
mk checkbox '<rect x="3" y="3" width="18" height="18" rx="3"/><path d="m8 12 3 3 5-6"/>'
mk open-in-full '<path d="M21 11V3h-8M3 13v8h8M21 3l-7 7M3 21l7-7"/>'
mk maximize '<rect x="4" y="4" width="16" height="16" rx="2"/>'
mk restore '<rect x="4" y="8" width="12" height="12" rx="2"/><path d="M10 4h6a4 4 0 0 1 4 4v6"/>'
# 最小化到托盘。按 Microsoft UX Guide（Notification Area）"Prefer icons with unique
# outlines over square or rectangular shaped icons" 改版：去掉窗口矩形（方框轮廓辨识度最差），
# 只留"向下箭头 + 底线"—— 收到屏幕边缘的通行语义，且与 download 的开口杯完全不像。
# 最小化到托盘（app.slint:5997 关闭确认弹框的选项卡片，E882）。运行时 Material 的 E882 就是
# **flip_to_back**（虚线圆角框 + 左下实心 L 角）—— 原先的 SVG（↓ + 底线）反而与 app 现状不符，
# 而且它本质就是「向下到一条线」，与 SFTP 下载形撞到 0.753。改回 flip_to_back：
# ① 与 app 现状一致（迁移后视觉变化为零）② 释放「↓ + 线」这一族给下载 ③ 墨量 0.148 → 0.234（均值 0.221）。
# 虚线参数 `4 3` 与 select-all 保持一致（库内只有一种虚线节奏）。
mk minimize-to-tray '<rect x="9" y="3" width="12" height="12" rx="1.5" stroke-dasharray="4 3"/><path d="M3 5.5v13a2 2 0 0 0 2 2h13"/>'
# 全屏 / 退出全屏：成对出现，靠括号朝向区分方向 ——
# 进入 = 拐点在画面外角（撑满四角）；退出 = 拐点移到画面内侧（从四角收回）。
# 刻意不用对角箭头：那会和 open-in-full（缩放展开手柄）撞进同一个箭头家族。
mk fullscreen-enter '<path d="M8 3H6a3 3 0 0 0-3 3v2M16 3h2a3 3 0 0 1 3 3v2M8 21H6a3 3 0 0 1-3-3v-2M16 21h2a3 3 0 0 0 3-3v-2"/>'
mk fullscreen-exit '<path d="M8 3v2a3 3 0 0 1-3 3H3M16 3v2a3 3 0 0 0 3 3h2M8 21v-2a3 3 0 0 0-3-3H3M16 21v-2a3 3 0 0 1 3-3h2"/>'
mk sidebar-left-collapse '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18"/><path d="m16 15-3-3 3-3"/>'
mk sidebar-left-expand '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18"/><path d="m14 15 3-3-3-3"/>'
mk sidebar-right-collapse '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M15 3v18"/><path d="m8 9 3 3-3 3"/>'
mk sidebar-right-expand '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M15 3v18"/><path d="m10 9-3 3 3 3"/>'

# 视图拆分：与 sidebar-* 同骨架（18×18 圆角方框），区别在于不带箭头 ——
# 箭头表达「动作」（收起/展开），分隔线表达「布局状态」（已拆分）。
# H 拆分 = 左右并排（竖分隔线）；V 拆分 = 上下堆叠（横分隔线）。
mk split-horizontal '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M12 3v18"/>'
mk split-vertical '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 12h18"/>'
# 完全退出（关闭主窗口弹框的右卡片）。原名 power，语义是"关掉设备"，与"退出应用"不等同。
# 现改为「开口容器 + 穿出箭头」（＝ Lucide log-out / Material exit_to_app 一族），语义是"离开这个容器"。
# 依据：① 换形后 16px 全库最相似者从 0.590 降到 0.514（换形同时把撞车一并降下来）；
#       ② bbox 由 20×22 收到 20×20，22px 槽里与左卡 minimize-to-tray 的视觉高差从 2.3px 收到 0.4px。
# 括号保留圆角，与库内清一色圆角风格一致（直角版 MAX 略低 0.498，但风格不齐，不取）。
mk quit '<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><path d="m16 17 5-5-5-5"/><path d="M21 12H9"/>'
mk settings '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>'
# 新版本提示（设置侧栏 NavItem，带文字标签）。
# 演进：Material「U 形托盘 + 上箭头」（U 形=装入容器的隐喻，与更新关联弱，徽章 r=1.5
# 在 16px 下仅 2px 读不出）→「底线 + 上箭头」（方向靠"上传"）→ 现为**双向循环箭头**。
# 依据：Icons8 把 "Available Updates" 归入 cycle-arrows，Windows 11 亦用循环箭头；
# 单向 refresh 表示"刷新"，双向表示"检查更新"，两者语义不同。
mk system-update '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>'
# 【2026-10-04 重画】旧形是「文件内部一支向下箭头」：箭头与文件壁墨距不足，14px 起糊成一坨，
# 且语义偏「下载文件」。改用 file-input 标准形 —— 文件右置，左缘在 y=8 以下断开，
# 箭头从开口水平穿入，「导入」语义由箭头的**进入方向**表达，不再靠内部符号。
mk input '<path d="M4 21h12a2 2 0 0 0 2-2V8l-4-4H6a2 2 0 0 0-2 2v4"/><path d="M14 4v4h4"/><path d="M3 15h8"/><path d="M8 12l3 3-3 3"/>'
mk language '<path d="m5 8 6 6"/><path d="m4 14 6-6 2-3"/><path d="M2 5h12"/><path d="M7 2h1"/><path d="m22 22-5-10-5 10"/><path d="M14 18h6"/>'
# 协议回落（theme.slint:227 protocol-glyph 的兜底：ssh / local / 未知协议）。
# 只承载 **EB8E**。EA34（顶栏「命令栏」开关）已拆到下一行的 command-bar —— 两者原先同形，
# 在顶栏与协议标签两处会互相误认。
mk terminal '<rect x="2.5" y="3.5" width="19" height="17" rx="3"/><path d="m7 9 3 3-3 3M13 15h4"/>'
# 顶栏「命令栏」开关（app.slint:1691，label "Command"/"命令栏"）。原用 EA34 = Material terminal，
# 与上面的协议回落同形。改为**无框提示符**（= Lucide terminal 原形）—— 靠「有没有外框」做
# 结构性区分，而不是改内部符号。实测与 terminal 的 16px 相关度低到未进前 12（<0.35），墨量 0.096。
mk command-bar '<path d="m4 17 6-6-6-6M12 19h8"/>'
mk serial-port '<path d="M6 5h12l3 6v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-6l3-6z"/><path d="M8 8.5h.01M12 8.5h.01M16 8.5h.01M7 12h.01M12 12h.01M17 12h.01"/>'
mk window-asset '<rect x="2" y="4" width="20" height="16" rx="3"/><path d="M2 9h20M5.5 6.5h.01M8.5 6.5h.01"/>'
# RDP / VNC · 远程桌面（协议图标 E30C，由 theme.slint 的 protocol-glyph() 供给协议下拉框
# 与会话列表两处）。
# 【改名】原 desktop-windows 是 Material 形状名，而它承载的是「远程桌面协议」；
#   项目其余图标均按功能命名。码位 E30C 未变，protocol-glyph() 不需要改。
# 【为何换形】原形是「显示器 + 底座 + 屏内 ›」，与托盘菜单的「显示 / Show」(display)
#   撞成 0.754，超过全库 p99 = 0.746 —— 屏内那个 › 太小，压不下相关度。
#   改「两台屏并排」后：vs display 0.000、vs terminal 0.000，MAX 0.423 (keyboard)。
# 【为何并排而非交叠】Windows 的惯例是交叠双屏，但交叠矩形避不开窗口还原 restore
#   （实测 0.854），故取并排（不交叠）。
# 【为何不拆 RDP / VNC 两个】主流对两者用同一个「远程桌面」图标；实测拆分后两者互撞
#   0.729，而它们在会话列表里正是并排的 —— 拆分解决不了问题。
# 【2026-10-04 重画】旧形两屏路径间距只有 2 格 → 墨距 0，两屏是**贴死**的，再加双竖杆
# 双底座共 6 个元素，16px 全粘。新形两屏间距 4 格（墨距 2），去掉各自底座改共享底座横线。
mk remote-desktop '<rect x="3" y="4" width="7" height="9" rx="1.5"/><rect x="14" y="4" width="7" height="9" rx="1.5"/><path d="M6.5 15v2M17.5 15v2"/><path d="M4 19h16"/>'
# 显示（托盘菜单「显示 / Show」，tray_menu.slint 的 TrayGlyph kind=0 的库内对应）。
# 【改名】原名 monitor 是物件名，而它承载的功能是「显示」；项目其余图标均按功能命名。
# 【码位】托盘「显示」是**自绘** TrayGlyph，没有 Material 码位，故为 —；
#   原先给它的 E1B1 实为「渲染 / Rendering」，已转给下面的 rendering。
mk display '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M12 17v4M8 21h8"/>'
# 渲染（设置侧栏 NavItem「渲染 / Rendering」，interface_panel.slint:701，码位 E1B1）。
# 【为何重做】原用 monitor 形，与托盘「显示」撞成同一个图形（实测相关度 1.000）。
# 改「层叠」= 渲染层级 / 图形合成；实测撞车 0.476（全库 p99 = 0.746），且与 display 无关。
# 曾比较：芯片（语义最贴该页的 CPU/GPU 选型，0.509）、画笔（0.487）；用户选层叠。
mk rendering '<path d="m12 3 8 4-8 4-8-4z"/><path d="m4 12 8 4 8-4"/><path d="m4 16 8 4 8-4"/>'
mk wallpaper '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-4.5-4.5L5 21"/>'
# 批量导入(文本)（app.slint:4722，设置菜单条目 —— 预览页原写"表格行"，与实际不符）。
# 原为三条 4 单位高的圆角矩形：2px 描边几乎填满矩形，16px 墨量 0.420 是全库最重（均值 0.222）。
# 改为三行文字 + 右下角导入箭头：墨量 0.221 回到均值，且补上了"导入"语义。
# 已排除的形态：三行纯文字会被 `output-highlight`（也是三行文字）撞到 0.563；
# 文本框形会被 `keyboard`（矩形 + 内部横线）撞到 0.688。
# 批量导入(文本)（app.slint:4722，设置菜单）。原先三条 4 单位高的圆角矩形：2px 描边几乎填满，
# 16px 墨量 0.420 是全库最重（均值 0.221）；而且它<b>从不创建文件夹</b>，是「把多行文本里的多条连接一次加进来」。
# ＝ Lucide list-plus / Material playlist_add 的「多行 + 加号」。
# 尤其注意：**不含箭头** —— 同一菜单上方已有 ↑（导出连接）与 ↓（导入连接），
# 若这里再放箭头就成了三个箭头排一列（上一版我加过箭头，是在孤立语境下误判的）。
mk table-rows '<path d="M16 5H3M11 12H3M16 19H3M18 9v6M21 12h-6"/>'
mk file '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/>'
# 日志（数据目录页「包含」条目 / 日志文件，2026-10-04 新增）。
# 折角刻意放在**右下**：与 file 同为右上折角时 16px 相关度 0.755（撞形），换到右下后降到 0.473。
mk log-file '<path d="M20 16V5a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h9z"/><path d="M20 16l-5 5"/><path d="M8 7h8M8 11h8M8 15h5"/>'
mk cloud '<path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z"/>'
mk computer '<path d="M20 16V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9m16 0H4m16 0 1.28 2.55A1 1 0 0 1 20.38 20H3.62a1 1 0 0 1-.9-1.45L4 16"/>'
# 主机（数据目录页「包含 · 主机」条目，2026-10-04 新增）。
# 造型 = 机架两层横条 + 左端指示灯。三条约束：① 两条墨距必须 2 格（上一版墨距 0，视觉贴死）；
# ② 上下边距对称 2/2；③ 指示灯 r=0.9，距内壁 1.6 格 —— 再大就与横条糊在一起。
# 实测 16 / 20px 分量 3（两横条 + 灯各自成块），最高对手 dashboard 0.692。
mk host-server '<rect x="3" y="3" width="18" height="7" rx="2"/><rect x="3" y="14" width="18" height="7" rx="2"/><circle cx="7.5" cy="6.5" r="0.9" fill="currentColor" stroke="none"/><circle cx="7.5" cy="17.5" r="0.9" fill="currentColor" stroke="none"/>'
# 快捷指令（数据目录页「包含 · 快捷指令」条目，2026-10-04 新增）。
# 造型 = 尖括号 + 斜杠。斜杠端点距括号尖必须 >=1.6 格：拉斜到 5 格更接近参考图，但 16px 三笔合一，
# 故定在 3 格（近竖直）。实测 12 / 16 / 20px 分量 3，最高对手 arrow-up 0.471。
mk quick-command '<path d="M7 7 3 12l4 5"/><path d="M13.5 6 10.5 18"/><path d="M17 7l4 5-4 5"/>'
mk plug '<path d="M17 19a1 1 0 0 1-1-1v-2a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2a1 1 0 0 1-1 1z"/><path d="M17 21v-2"/><path d="M19 14V6.5a1 1 0 0 0-7 0v11a1 1 0 0 1-7 0V10"/><path d="M21 21v-2"/><path d="M3 5V3"/><path d="M4 10a2 2 0 0 1-2-2V6a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2a2 2 0 0 1-2 2z"/><path d="M7 5V3"/>'
mk refresh '<path d="M21 12a9 9 0 1 1-2.64-6.36L21 8"/><path d="M21 3v5h-5"/>'

echo "generated: $(ls *.svg | wc -l) icons"
