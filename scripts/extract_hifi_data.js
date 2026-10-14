// 从高保真 HTML 提取 const D = {...} 目标数据（落地验收基准）
// 用法: node scripts/extract_hifi_data.js <html> <out.json>
const fs = require('fs');
const html = fs.readFileSync(process.argv[2], 'utf8');
const start = html.indexOf('const D = ');
if (start < 0) { console.error('未找到 const D'); process.exit(1); }
let i = html.indexOf('{', start);
let depth = 0, inStr = false, esc = false, end = -1;
for (let p = i; p < html.length; p++) {
  const ch = html[p];
  if (inStr) {
    if (esc) { esc = false; }
    else if (ch === '\\') { esc = true; }
    else if (ch === '"') { inStr = false; }
    continue;
  }
  if (ch === '"') { inStr = true; continue; }
  if (ch === '{') depth++;
  else if (ch === '}') { depth--; if (depth === 0) { end = p + 1; break; } }
}
if (end < 0) { console.error('括号不配平'); process.exit(1); }
const raw = html.slice(i, end);
const D = JSON.parse(raw);
fs.writeFileSync(process.argv[3], JSON.stringify(D, null, 2), 'utf8');
console.log('keep:', D.keep.length, 'drop:', Object.keys(D.drop).length,
            'dropIds:', D.dropIds.length, 'stats:', JSON.stringify(D.stats));
console.log('keep ids:', D.keep.map(t => t.id).join(', '));
console.log('写入:', process.argv[3]);
