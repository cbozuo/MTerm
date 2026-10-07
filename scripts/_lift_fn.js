// 一次性：频道收尾——提函数到顶层、启动行移位、selected/close 接线
const fs = require('fs');
let s = fs.readFileSync('src/app.rs', 'utf8');

// 1) 把嵌套 fn refresh_channel_bars 剪到顶层（紧贴 wire 函数之前不行——它在
//    wire 函数体内。剪出全文，从原位置删除，插入到「fn wire_」出现前的顶层区）
const fnStart = s.indexOf('    fn refresh_channel_bars(');
const fnEndMarker = '        window.set_pane_channel_bars(ModelRc::from(Rc::new(VecModel::from(bars))));\n    }';
const fnEnd = s.indexOf(fnEndMarker, fnStart);
if (fnStart < 0 || fnEnd < 0) { console.error('fn 边界没找到', fnStart, fnEnd); process.exit(1); }
const fnBody = s.slice(fnStart, fnEnd + fnEndMarker.length);
// 提为顶层：去一层缩进（4 空格）+ 加 pub(crate)
const fnTop = fnBody
    .split('\n')
    .map((l) => (l.startsWith('    ') ? l.slice(4) : l))
    .join('\n')
    .replace('fn refresh_channel_bars(', 'pub(crate) fn refresh_channel_bars(');
s = s.slice(0, fnStart) + s.slice(fnEnd + fnEndMarker.length);
// 顶层插入点：'fn wire' 不存在——插到 'enum StartMode' 之前的空行区。
// 更稳：插到 'mod tests' 前没有意义（要在使用前不必须，Rust 项顺序无关）。
// 直接插到第一个 'pub(crate) fn ' 或 'fn ' 顶层项前——找 '\nfn ' 或文件里
// 任何顶层锚。用 'pub(crate) fn reapply' 不存在；用 'fn tab_session_id' 也在嵌套。
// 最稳：插到 'pub mod' 不行。插到 impl AppWindow 之前——找 '\nimpl AppWindow'。
const implAt = s.indexOf('\nimpl AppWindow');
if (implAt < 0) { console.error('impl AppWindow 没找到'); process.exit(1); }
s = s.slice(0, implAt) + '\n' + fnTop + s.slice(implAt);

// 2) 启动行移位：删错位行，插到 set_quick_theme_entries 后
{
  const bad = '    // (#tab-32) 启动时初始化频道状态条数据（已在频道的会话建 tab 后显示）。\n    refresh_channel_bars(&window, &store.borrow(), &panes_model, &tabs_model);\n';
  s = s.split(bad).join('');
  const anchor = '        window.set_quick_theme_entries(ModelRc::from(Rc::new(VecModel::from(quick))));';
  if (s.split(anchor).length !== 2) { console.error('quick 锚没找到', s.split(anchor).length - 1); process.exit(1); }
  s = s.replace(
    anchor,
    anchor +
      '\n        // (#tab-32) 启动时初始化频道状态条数据（已在频道的会话建 tab 后显示）。\n        refresh_channel_bars(&window, &store.borrow(), &panes_model, &tabs_model);',
  );
}

fs.writeFileSync('src/app.rs', s);
console.log('lifted + startup moved');
