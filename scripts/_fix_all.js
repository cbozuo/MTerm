// 一次性：修复全部剩余编译错误
const fs = require('fs');

// ── app.rs ──
let s = fs.readFileSync('src/app.rs', 'utf8').replace(/\r\n/g, '\n');

// ① refresh_channel_bars 改 &VecModel（refresh_panes 内传的是 &VecModel）
{
  const a = `pub(crate) fn refresh_channel_bars(
    window: &AppWindow,
    store: &ConfigStore,
    panes: &Rc<slint::VecModel<crate::ui::PaneInfo>>,
    tabs: &Rc<slint::VecModel<crate::ui::TabInfo>>,
) {`;
  const n = `pub(crate) fn refresh_channel_bars(
    window: &AppWindow,
    store: &ConfigStore,
    panes: &slint::VecModel<crate::ui::PaneInfo>,
    tabs: &slint::VecModel<crate::ui::TabInfo>,
) {`;
  if (s.split(a).length !== 2) { console.error('① miss', s.split(a).length - 1); process.exit(1); }
  s = s.replace(a, n);
}

// ② welcome-as-sidebar 闭包 store move 问题：内层 Timer 前 clone store
{
  const a = `            let weak = weak.clone();
            let layout = layout.clone();
            let content_size = content_size.clone();
            let tabs_model = tabs_model.clone();
            let panes_model = panes_model.clone();
            let splitters_model = splitters_model.clone();
            slint::Timer::single_shot(std::time::Duration::ZERO, move || {`;
  const n = `            let weak = weak.clone();
            let store = store.clone();
            let layout = layout.clone();
            let content_size = content_size.clone();
            let tabs_model = tabs_model.clone();
            let panes_model = panes_model.clone();
            let splitters_model = splitters_model.clone();
            slint::Timer::single_shot(std::time::Duration::ZERO, move || {`;
  if (s.split(a).length !== 2) { console.error('② miss', s.split(a).length - 1); process.exit(1); }
  s = s.replace(a, n);
}

fs.writeFileSync('src/app.rs', s);

// ── tab_callbacks.rs：全部 refresh_panes 调用补 store（normalize + 插入）──
let tc = fs.readFileSync('src/app/tab_callbacks.rs', 'utf8').replace(/\r\n/g, '\n');
// 在每个 &splitters_model, 后插 store 行（缩进与 splitters 一致）
tc = tc.replace(/^( +)&splitters_model,$/gm, '$1&splitters_model,\n$1&core.store.borrow(),');
// 删连续重复（防止多次应用）
tc = tc.replace(/(&core\.store\.borrow\(\),\n)\1+/g, '$1');
fs.writeFileSync('src/app/tab_callbacks.rs', tc);

// ── tab_transfer.rs：同样处理 ──
let tt = fs.readFileSync('src/app/tab_transfer.rs', 'utf8').replace(/\r\n/g, '\n');
tt = tt.replace(/^( +)&splitters_model,$/gm, '$1&splitters_model,\n$1&core.store.borrow(),');
tt = tt.replace(/(&core\.store\.borrow\(\),\n)\1+/g, '$1');
fs.writeFileSync('src/app/tab_transfer.rs', tt);

console.log('all fixed');
