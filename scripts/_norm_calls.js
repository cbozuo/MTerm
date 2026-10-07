// 一次性：统一所有 refresh_panes 调用（补 store 参数 + 参数顺序规整）
const fs = require('fs');

for (const f of ['src/app.rs', 'src/app/tab_callbacks.rs', 'src/app/tab_transfer.rs']) {
  let s = fs.readFileSync(f, 'utf8').replace(/\r\n/g, '\n');
  // 匹配每个 refresh_panes(...) 调用块
  const re = /refresh_panes\(\n([\s\S]*?)\n(\s*)\);/g;
  let n = 0;
  s = s.replace(re, (m, args) => {
    // 拆参数行，去掉空/已有 store 行，收集其余
    const parts = args.split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('&store') && !l.startsWith('store'));
    if (parts.length !== 6) return m; // 非 6 参（fn 定义等）跳过
    // 找 window/w 参数（第一个）与 store 候选
    const win = parts[0];
    const storeArg = parts.find(p => p.includes('store') ) || null;
    const rest = parts.slice(1);
    n++;
    return 'refresh_panes(\n'
      + rest.map(r => '    ' + r).join('\n')
      + (storeArg ? '\n    ' + storeArg : '\n    &store.borrow()')
      + '\n';
  }).replace(/refresh_panes\(\n/g, m => m); // no-op 保读性
  // 重新缩进：把插入的参数行加上原缩进——上面简化丢了缩进，改为不格式化：
  fs.writeFileSync(f, s);
  console.log(f, 'calls fixed:', n);
}
console.log('note: 上面方式缩进会破坏格式——改用逐调用保守方案');
