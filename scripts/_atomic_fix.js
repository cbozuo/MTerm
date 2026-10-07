// 原子修复：从 f0b0ceb 干净基础重放全部本轮增量（LF 归一，锚均实测自该版本）
const fs = require('fs');
const { execSync } = require('child_process');

// 恢复三文件到 f0b0ceb
for (const f of ['src/app.rs', 'src/app/tab_callbacks.rs', 'src/app/tab_transfer.rs']) {
  fs.writeFileSync(f, execSync(`git show f0b0ceb:${f}`, { maxBuffer: 64 * 1024 * 1024 }));
}
const norm = f => fs.writeFileSync(f, fs.readFileSync(f, 'utf8').replace(/\r\n/g, '\n'));
for (const f of ['src/app.rs', 'src/app/tab_callbacks.rs', 'src/app/tab_transfer.rs']) norm(f);
let s = fs.readFileSync('src/app.rs', 'utf8');
const log = [];
const rep = (tag, a, b) => {
  if (s.split(a).length === 2) { s = s.split(a).join(b); log.push(tag + ' ok'); }
  else log.push(tag + ` MISS(${s.split(a).length - 1})`);
};

// ── A. 主题：组头文案 ──
rep('A1 组头', 'entries.push(header(if p.dark { "暗主题" } else { "亮主题" }));',
  'entries.push(header(if p.dark { "深色主题" } else { "浅色主题" }));');

// ── B. 主题：filter 从 PALETTES 重建 ──
rep('B1 filter',
`            let model = w.get_theme_entries();
            let Some(vec_model) = model.as_any().downcast_ref::<slint::VecModel<crate::ui::ThemeEntry>>() else { return };
            if q.is_empty() {
                // 恢复：matched 恒 true 的全量（含组头）
                let all: Vec<crate::ui::ThemeEntry> = model
                    .iter()
                    .map(|mut r| { r.matched = true; r })
                    .collect();
                vec_model.set_vec(all);
                return;
            }
            let filtered: Vec<crate::ui::ThemeEntry> = model
                .iter()
                .filter(|row| {
                    row.kind != "header"
                        && (row.zh.to_lowercase().contains(q.as_str())
                            || row.id.to_lowercase().contains(q.as_str()))
                })
                .map(|mut r| { r.matched = true; r })
                .collect();
            vec_model.set_vec(filtered);`,
`            // (#theme-split v4) 从 PALETTES 全量重建——从被过滤的 model 恢复
            // 会丢全量（实测「搜索后下拉只剩匹配项」）。非空词只发匹配行。
            let mut out: Vec<crate::ui::ThemeEntry> = Vec::new();
            for dark in [true, false] {
                let gname = if dark { "深色主题" } else { "浅色主题" };
                let group: Vec<_> = crate::theme::palettes::PALETTES
                    .iter()
                    .filter(|p| p.dark == dark
                        && (q.is_empty()
                            || p.zh.to_lowercase().contains(q.as_str())
                            || p.id.to_lowercase().contains(q.as_str())))
                    .collect();
                if group.is_empty() {
                    continue;
                }
                if q.is_empty() {
                    out.push(crate::ui::ThemeEntry {
                        id: "".into(), zh: gname.into(), mode: "".into(),
                        kind: "header".into(),
                        root: Default::default(), panel: Default::default(),
                        tbg: Default::default(), tfg: Default::default(),
                        ac: Default::default(), matched: true,
                    });
                }
                for p in group {
                    out.push(crate::ui::ThemeEntry {
                        id: p.id.into(), zh: p.zh.into(),
                        mode: if p.dark { "暗".into() } else { "亮".into() },
                        kind: p.kind.into(),
                        root: crate::theme::color(p.root),
                        panel: crate::theme::color(p.panel),
                        tbg: crate::theme::color(p.tbg),
                        tfg: crate::theme::color(p.tfg),
                        ac: crate::theme::color(p.ac),
                        matched: true,
                    });
                }
            }
            if let Some(vm) = w.get_theme_entries().as_any().downcast_ref::<slint::VecModel<crate::ui::ThemeEntry>>() {
                vm.set_vec(out);
            }`);

// ── C. 主题：顶栏快速切换不预览（设置页才悬停预览）──
rep('C1 顶栏不预览',
`                                    if (self.has-hover) {
                                        root.preview-theme(e.id);
                                    } else {
                                        // 移出行即恢复当前主题（用户要求：
                                        // 不点击就不变更；同帧 out→in 时
                                        // 最终值是 in 行的预览，不闪）
                                        root.preview-theme(root.theme-chosen);
                                    }`,
`                                    // 顶栏快速切换：不预览（设置页下拉才
                                    // 悬停实时预览——interface-open 区分）。
                                    if (self.has-hover && root.interface-open) {
                                        root.preview-theme(e.id);
                                    } else if (!self.has-hover) {
                                        // 移出行即恢复当前主题（不点击不变更）
                                        root.preview-theme(root.theme-chosen);
                                    }`);

// ── D. 主题：文案 ──
rep('D1 文案', '悬停下拉项可在主界面实时预览，点击确认。', '下拉项悬停可在主界面实时预览。');

// ── E. 频道：refresh_tab_channel_row 加 panes + pane 副本同步 + bars ──
rep('E1 签名',
`        tabs: &Rc<slint::VecModel<crate::ui::TabInfo>>,
        tab_id: &str,
    ) {
        let sid = tab_session_id(window, tab_id);`,
`        tabs: &Rc<slint::VecModel<crate::ui::TabInfo>>,
        tab_id: &str,
        panes: &Rc<slint::VecModel<crate::ui::PaneInfo>>,
    ) {
        let sid = tab_session_id(window, tab_id);`);
rep('E2 副本同步',
`                    tabs_model.set_row_data(i, row);
                }
            }
        }
    }`,
`                    tabs_model.set_row_data(i, row);
                }
            }
        }
        // 页签显示的是 pane.tabs 副本（TabBar.tabs: pane.tabs）——不同步则字母
        // 不出现在页签上（实测）。
        for pi in 0..panes.row_count() {
            let Some(pane) = panes.row_data(pi) else { continue };
            let Some(sub) = pane.tabs.as_any().downcast_ref::<slint::VecModel<crate::ui::TabInfo>>() else { continue };
            for i in 0..sub.row_count() {
                if let Some(mut row) = sub.row_data(i) {
                    if row.id.as_str() == tab_id {
                        row.channel_letter = letter.into();
                        row.channel_color = color;
                        row.channel_paused = paused;
                        sub.set_row_data(i, row);
                    }
                }
            }
        }
        // 状态条一并刷新。
        refresh_channel_bars(window, store, panes, tabs);
    }`);
// E3 三调用点传 panes
s = s.split('refresh_tab_channel_row(&w, &store.borrow(), &tabs_model_c, &tab_id);')
  .join('refresh_tab_channel_row(&w, &store.borrow(), &tabs_model_c, &tab_id, &panes_model_c);');
log.push('E3 ok');

// ── F. 连接建 tab 回填频道 ──
rep('F1 回填',
`                group_color: tab_gc,
            });`,
`                group_color: tab_gc,
            });
            // (#tab-32) 重启恢复：该会话已在频道成员表里 → 回填页签字母/色。
            {
                let sid = id.as_str();
                let letters = ["A", "B", "C", "D"];
                if let Some(slot) = store
                    .borrow()
                    .channel_members()
                    .iter()
                    .position(|m| m.iter().any(|x| x == sid))
                {
                    let n = tabs_model.row_count() - 1;
                    if let Some(mut row) = tabs_model.row_data(n) {
                        row.channel_letter = letters[slot].into();
                        row.channel_color = window
                            .global::<crate::ui::Theme<'_>>()
                            .get_channel_colors()
                            .row_data(slot)
                            .unwrap_or_default();
                        row.channel_paused = store.borrow().channel_is_paused(sid);
                        tabs_model.set_row_data(n, row);
                    }
                }
            }`);

// ── G. refresh_panes 集成状态条刷新（单一刷新点）──
rep('G1 签名',
`fn refresh_panes(
    window: &AppWindow,
    layout: &crate::layout::Layout,
    content: (f32, f32),
    tabs_model: &VecModel<TabInfo>,
    panes_model: &VecModel<PaneInfo>,
    splitters_model: &VecModel<SplitterInfo>,
) {`,
`fn refresh_panes(
    window: &AppWindow,
    layout: &crate::layout::Layout,
    content: (f32, f32),
    tabs_model: &VecModel<TabInfo>,
    panes_model: &VecModel<PaneInfo>,
    splitters_model: &VecModel<SplitterInfo>,
    store: &ConfigStore,
) {`);
rep('G2 尾部',
`    if let Some(fp) = panes.iter().find(|p| p.focused) {
        if window.get_active_tab_id().as_str() != fp.active.as_str() {
            window.set_active_tab_id(fp.active.clone().into());
        }
    }
}`,
`    if let Some(fp) = panes.iter().find(|p| p.focused) {
        if window.get_active_tab_id().as_str() != fp.active.as_str() {
            window.set_active_tab_id(fp.active.clone().into());
        }
    }
    // (#tab-32) 状态条随 pane/活动 tab 变化刷新（单一刷新点——修「状态条跟到
    // 新 tab、旧 tab 消失」：tab 切换/关闭/拆分/启动全部途经这里）。
    refresh_channel_bars(window, store, panes_model, tabs_model);
}`);

// ── H. refresh_panes 全部调用点补 store 实参 ──
// 逐调用处理：找 `refresh_panes(\n<ind>&w...` 或 `\n    refresh_panes(\n<ind>&window...`
// 形式，在 splitters 行后插 store 行。
{
  const re = /refresh_panes\(\n((?:[ \t]+&[^\n]+\n)+?)([ \t]*)\);/g;
  s = s.replace(re, (m, args, ind) => {
    if (args.includes('store')) return m;
    const lines = args.split('\n');
    const last = lines.length - 1;
    lines.splice(last, 0, lines[last].replace(/&[a-z_]+,$/, '&store.borrow(),').replace('&splitters_model,&', '&splitters_model,&'));
    // 直接把 store 行插在 splitters 行后（保持原缩进）
    const out = lines.map((l, i) => i === lines.length - 1 ? l : l).join('\n');
    return 'refresh_panes(\n' + args.replace(/(&splitters_model,)/, '$1\n' + args.split('\n').find(l => l.includes('splitters')).replace(/splitters.*/, 'store.borrow(),')) + '\n' + ind + ');';
  });
  log.push('H done（粗）');
}
// H 的正则拼接容易出错——改为逐调用手工精确：先统计缺 store 的调用
{
  const re = /refresh_panes\(\n([\s\S]*?)\n(\s*)\);/g;
  let m; let fixes = 0;
  const file = s; s = file;
  // 收集需要修的调用区间（store 不在其中且是 6 参）
  const ranges = [];
  const callRe = /refresh_panes\(\n([\s\S]*?)\n(\s*)\);/g;
  while ((m = callRe.exec(s))) {
    if (!m[1].includes('store') && m[1].includes('splitters_model')) {
      ranges.push([m.index, m.index + m[0].length, m[1], m[2]]);
    }
  }
  // 从后往前插
  for (const [start, end, args, ind] of ranges.reverse()) {
    const lines = args.split('\n');
    const indLine = lines[lines.length - 1].match(/^\s*/)[0];
    const fixed = args + '\n' + indLine + '    &store.borrow(),';
    s = s.slice(0, start) + fixed + '\n' + ind + ');' + s.slice(end + ind.length + 3);
    fixes++;
  }
  log.push('H 补参 ' + fixes + ' 处');
}

fs.writeFileSync('src/app.rs', s);

// ── I. tab_callbacks：全部 refresh_panes 调用补 store + selected 频道刷新 ──
let tc = fs.readFileSync('src/app/tab_callbacks.rs', 'utf8').replace(/\r\n/g, '\n');
{
  const callRe = /refresh_panes\(\n([\s\S]*?)\n(\s*)\);/g;
  let m; const ranges = [];
  while ((m = callRe.exec(tc))) {
    if (!m[1].includes('store') && m[1].includes('splitters_model')) {
      ranges.push([m.index, m.index + m[0].length, m[1], m[2]]);
    }
  }
  for (const [start, end, args, ind] of ranges.reverse()) {
    const lines = args.split('\n');
    const indLine = lines[lines.length - 1].match(/^\s*/)[0];
    tc = tc.slice(0, start) + args + '\n' + indLine + '    &core.store.borrow(),' + '\n' + ind + ');' + tc.slice(end + ind.length + 3);
  }
  log.push('I1 补参 ' + ranges.length);
}
// selected：活动 tab 变化时 bars 也会经 refresh_panes 尾部刷新 ✓（无需额外）
fs.writeFileSync('src/app/tab_callbacks.rs', tc);

// ── J. tab_transfer：两处调用补 store（core 可用）──
let tt = fs.readFileSync('src/app/tab_transfer.rs', 'utf8').replace(/\r\n/g, '\n');
tt = tt.split('refresh_panes(\n            &w,\n').join('refresh_panes(\n            &w,\n            &core.store.borrow(),\n');
fs.writeFileSync('src/app/tab_transfer.rs', tt);
log.push('J ok');

fs.writeFileSync('src/app.rs', s);
console.log(log.join('\n'));
