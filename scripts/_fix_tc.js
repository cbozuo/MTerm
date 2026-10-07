// 一次性：tab_callbacks.rs——全部 refresh_panes 调用补 store 参数 + selected 频道刷新
const fs = require('fs');
let s = fs.readFileSync('src/app/tab_callbacks.rs', 'utf8').replace(/\r\n/g, '\n');

// ① refresh_panes( 调用补 &core.store.borrow()（第 3 参；紧随 &w, 之后）
s = s.split('refresh_panes(\n                    &w,\n').join('refresh_panes(\n                    &w,\n                    &core.store.borrow(),\n');

// ② selected 回调：频道状态条随活动 tab 刷新
{
  const a = '        let bufs_tab_sel = bufs.clone();\n        window.on_pane_tab_selected(move |pane_id: i32, id: SharedString| {\n            let id = id.to_string();';
  if (s.split(a).length !== 2) { console.error('② 锚没找到', s.split(a).length - 1); process.exit(1); }
  s = s.replace(a, '        let bufs_tab_sel = bufs.clone();\n        let core_sel = core.clone();\n        window.on_pane_tab_selected(move |pane_id: i32, id: SharedString| {\n            let id = id.to_string();');
}
{
  const a = '            if let Some(w) = weak.upgrade() {\n                refresh_panes(\n                    &w,\n                    &core.store.borrow(),\n                    &layout.borrow(),\n                    content_size.get(),\n                    &tabs_model,\n                    &panes_model,\n                    &splitters_model,\n                );';
  if (s.split(a).length !== 2) { console.error('②b 锚没找到', s.split(a).length - 1); process.exit(1); }
  // selected 的刷新在 refresh_panes 之后补一次 bars（refresh_panes 尾部已统一刷新
  // bars——这里无需重复；仅确认 refresh_panes 尾部刷新已覆盖）。
  log2('② 已由 refresh_panes 尾部统一刷新覆盖');
}
function log2(m) { console.log(m); }

fs.writeFileSync('src/app/tab_callbacks.rs', s);
console.log('tab_callbacks done');
