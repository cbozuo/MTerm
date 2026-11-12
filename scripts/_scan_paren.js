const fs = require('fs');
const lines = fs.readFileSync('src/app.rs', 'utf8').split(/\r?\n/);
let bal = 0, inStr = null, inBlock = 0, inLine = false;
const lineBal = [];
for (let i = 0; i < lines.length; i++) {
  const s = lines[i];
  for (let c = 0; c < s.length; c++) {
    const ch = s[c], nx = s[c + 1];
    if (inBlock) { if (ch === '*' && nx === '/') { inBlock = 0; c++; } continue; }
    if (inStr) { if (ch === '\\') { c++; continue; } if (ch === inStr) { inStr = null; } continue; }
    if (inLine) break;
    if (ch === '/' && nx === '/') { inLine = true; break; }
    if (ch === '/' && nx === '*') { inBlock = 1; c++; continue; }
    if (ch === '"' || ch === "'") { inStr = ch; continue; }
    if (ch === '(') bal++;
    if (ch === ')') bal--;
  }
  inLine = false;
  lineBal.push(bal);
}
console.log('EOF bal =', bal);
for (let i = 9399; i < Math.min(9700, lineBal.length); i++) {
  if (lineBal[i] >= 3) console.log('line', i + 1, 'bal', lineBal[i], '::', lines[i].slice(0, 90));
}
