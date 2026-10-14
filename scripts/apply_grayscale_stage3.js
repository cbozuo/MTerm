// #theme-dedupe 阶段三：A 类图标染色收敛（高保真 §3.5）
// 只替换「整行裸 tint」：`tint: Theme.text-secondary;` → `tint: Theme.text-primary;`
// 不动：三元 tint（彩色/hover 反馈）、background/stroke 借用、B 类（另行手工）。
// 用法: node scripts/apply_grayscale_stage3.js [--write]
const fs = require('fs');
const path = require('path');
const DIR = path.join(__dirname, '..', 'ui');
const WRITE = process.argv.includes('--write');
let total = 0, files = 0;
for (const f of fs.readdirSync(DIR).filter((x) => x.endsWith('.slint')).sort()) {
  const p = path.join(DIR, f);
  const src = fs.readFileSync(p, 'utf8');
  const wasCRLF = src.includes('\r\n');
  const lines = src.split(/\r?\n/);
  let n = 0;
  const out = lines.map((l) => {
    // 子串级精确替换：三元形式（`tint: cond ? A : Theme.text-secondary;`）不含
    // 紧邻的 `tint: Theme.text-secondary;`，不会被误伤。
    if (l.includes('tint: Theme.text-secondary;')) {
      n++;
      return l.replaceAll('tint: Theme.text-secondary;', 'tint: Theme.text-primary;');
    }
    return l;
  });
  if (n) {
    console.log(`${f}: ${n} 处`);
    files++; total += n;
    if (WRITE) fs.writeFileSync(p, out.join(wasCRLF ? '\r\n' : '\n'), 'utf8');
  }
}
console.log(`—— 合计 ${total} 处 @ ${files} 文件 ${WRITE ? '（已写入）' : '（dry-run，加 --write 执行）'}`);
