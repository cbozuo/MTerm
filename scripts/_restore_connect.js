// 一次性：用 f0b0ceb 的完好 connect 段替换现状损坏段，并重插频道回填
const fs = require('fs');
const { execSync } = require('child_process');

const f0 = execSync('git show f0b0ceb:src/app.rs', { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }).split('\n');
let s = fs.readFileSync('src/app.rs', 'utf8');

// f0 版完整 connect 段
const st0 = f0.findIndex(l => l.includes('window.on_connect_session'));
let paren = 1, en0 = -1;
for (let i = st0; i < f0.length; i++) {
  for (const ch of f0[i]) { if (ch === '(') paren++; if (ch === ')') paren--; }
  if (paren === 0) { en0 = i; break; }
}
if (en0 < 0) { console.error('f0 闭合没找到'); process.exit(1); }
const good = f0.slice(st0, en0 + 1).join('\n');

// 现状文件：找 connect 段闭合
const st = s.indexOf('        window.on_connect_session(move |id: SharedString| {');
let paren2 = 1, en = -1;
for (let i = st; i < s.length; i++) {
  for (const ch of s[i]) { if (ch === '(') paren2++; if (ch === ')') paren2--; }
  if (paren2 === 0) { en = i; break; }
}
if (en < 0) { console.error('现状闭合没找到'); process.exit(1); }

// 替换为 f0 完好段
s = s.slice(0, st) + good + s.slice(en + 1);

// 重插频道回填（push 尾之后）
const pushTail = '                group_color: tab_gc,\n            });';
if (s.split(pushTail).length !== 2) { console.error('pushTail 锚没找到'); process.exit(1); }
const backfill = pushTail + `
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
                        row.channel_color = {
                            let tid = store.borrow().theme().to_string();
                            let follow = store.borrow().follow_system();
                            let pal = crate::theme::palette_or_default(&tid, follow);
                            crate::theme::color(pal.channel[slot])
                        };
                        row.channel_paused = store.borrow().channel_is_paused(sid);
                        tabs_model.set_row_data(n, row);
                    }
                }
            }`;
s = s.replace(pushTail, backfill);

fs.writeFileSync('src/app.rs', s);
console.log('connect segment restored from f0 + backfill re-inserted');
