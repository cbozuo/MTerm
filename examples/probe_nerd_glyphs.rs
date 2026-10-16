#!/usr/bin/env cargo run --release --example probe_nerd_glyphs
//! (#nerd-glyph-probe) 诊断：终端里的 Nerd Font 图标为什么渲染得偏小/不对。
//!
//! 用户现象（同一台服务器、同样 13px，MTerm 里的 `ls` 图标比 iShell Pro 小）：
//! 已确认终端字体选的是 **JetBrainsMono Nerd Font**（本身就含Nerd Font 图标字形），
//! 所以「字体没装」不是原因。本探针回答三个问题：
//!
//!   ①系统里到底装了哪些 Nerd Font 变体（三个相似名字并存）；
//!   ② 每个变体**是否真的含私用区（PUA）图标字形** —— Nerd Font 的图标都在
//!      U+E000~U+F8FF；普通等宽字体（Consolas / Cascadia）**一个都没有**；
//!   ③ MTerm 的 `MONO_PROBE`（`A g 0 ─ │`）能否区分它们 —— 它验的是ASCII +
//!      制表符，**对 PUA 图标完全不敏感**，所以「通过了字体探针」不等于
//!      「有图标」。
//!
//! 用法：`cargo run --release --example probe_nerd_glyphs`
//! 输出：每个候选 family 的 PUA 覆盖数 + MONO_PROBE 结果 + 结论。

use fontdb::{Database, Source};
use skrifa::charmap::Charmap;
use skrifa::FontRef;

/// 与 `src/app.rs::MONO_PROBE` 保持一致（必须一致，否则探针说的不是真实门槛）。
const MONO_PROBE: &[char] = &['A', 'g', '0', '─', '│'];

/// Nerd Font 图标的实际码位区间（v3 区间）。
const NERD_RANGES: &[(u32, u32)] = &[(0xE000, 0xF8FF)];

/// ls --color 常用的几个图标（Nerd Font 文件图标区，约 U+F000 段）。
const COMMON_ICONS: &[(char, &str)] = &[
    ('\u{f015}', "folder"),           // nf-fa-folder
    ('\u{f00c}', "file"),             // nf-fa-file
    ('\u{f0c5}', "cog / config"),     // nf-fa-cog
    ('\u{f0c7}', "cloud / sql"),      // nf-fa-cloud
    ('\u{f1c9}', "archive / sql"),    // nf-fa-file_archive
    ('\u{f0ae}', "task / executable"), // nf-fa-task
];

fn face_bytes(db: &Database, id: fontdb::ID) -> Option<Vec<u8>> {
    let face = db.face(id)?;
    match &face.source {
        Source::Binary(b) => Some(b.as_ref().as_ref().to_vec()),
        Source::SharedFile(_, b) => Some(b.as_ref().as_ref().to_vec()),
        Source::File(p) => std::fs::read(p).ok(),
    }
}

fn probe_family(db: &Database, fam: &str) -> Option<Probe> {
    // 与 `src/app.rs::find_regular_face` **完全同口径**：regular weight /
    // normal stretch / normal style 的 query，而不是遍历 db.faces()。
    let id = db.query(&fontdb::Query {
        families: &[fontdb::Family::Name(fam)],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    })?;
    let bytes = face_bytes(db, id)?;
    let face = db.face(id)?;
    let font = FontRef::from_index(&bytes, face.index).ok()?;
    let cmap = Charmap::new(&font);

    let mono = MONO_PROBE.iter().all(|&c| cmap.map(c).is_some());

    // PUA 覆盖：区间内有多少码位有字形
    let mut pua_total = 0usize;
    let mut pua_mapped = 0usize;
    for &(lo, hi) in NERD_RANGES {
        for cp in lo..=hi {
            pua_total += 1;
            if cmap.map(char::from_u32(cp).unwrap()).is_some() {
                pua_mapped += 1;
            }
        }
    }
    let icons: Vec<(char, &str, bool)> = COMMON_ICONS
        .iter()
        .map(|&(c, n)| (c, n, cmap.map(c).is_some()))
        .collect();
    Some(Probe { mono, pua_mapped, pua_total, icons })
}

struct Probe {
    mono: bool,
    pua_mapped: usize,
    pua_total: usize,
    icons: Vec<(char, &'static str, bool)>,
}

fn main() {
    let mut db = Database::new();
    db.load_system_fonts();

    let mut fams: Vec<String> = db
        .faces()
        .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
        .collect();
    fams.sort();
    fams.dedup();

    println!("系统共载入 {} 个 family。\n", fams.len());

    // 1) 候选：名字里带 Mono / Nerd 的等宽族
    let cands: Vec<&String> = fams
        .iter()
        .filter(|f| {
            let l = f.to_lowercase();
            (l.contains("mono") || l.contains("nerd")) && !l.contains("propo")
        })
        .collect();

    println!("=== 候选等宽 / Nerd 字体族（{} 个）===", cands.len());
    println!(
        "{:<38} {:<10} {:>9}  图标",
        "family", "MONO_PROBE", "PUA 覆盖"
    );
    let mut with_icons: Vec<&str> = Vec::new();
    for fam in &cands {
        let Some(p) = probe_family(&db, fam) else { continue };
        let pct = 100.0 * p.pua_mapped as f64 / p.pua_total as f64;
        let has = p.icons.iter().any(|(_, _, ok)| *ok);
        if has {
            with_icons.push(fam.as_str());
        }
        println!(
            "{:<38} {:<10} {:>8.2}%  {}",
            fam,
            if p.mono { "pass" } else { "FAIL" },
            pct,
            if has { "有" } else { "无"}
        );
    }

    println!("\n=== 常用图标字形逐个检查（只看有图标的 family）===");
    for fam in &with_icons {
        let Some(p) = probe_family(&db, fam) else { continue };
        let detail: Vec<String> = p
            .icons
            .iter()
            .map(|&(_c, n, ok)| format!("{}:{}", n, if ok { "OK" } else { "缺" }))
            .collect();
        println!("  {:<38} {}", fam, detail.join("  "));
    }

    // 2) 关键结论：MONO_PROBE 能否区分「有图标」与「无图标」
    println!("\n=== 门槛对比：MONO_PROBE 能否识别图标能力 ===");
    let pass_no_icon: Vec<&str> = cands
        .iter()
        .filter(|f| {
            probe_family(&db, f)
                .map(|p| p.mono && !p.icons.iter().any(|(_, _, ok)| *ok))
                .unwrap_or(false)
        })
        .map(|s| s.as_str())
        .collect();
    println!(
        "通过 MONO_PROBE 但**没有任何 Nerd 图标字形**的 family：{} 个",
        pass_no_icon.len()
    );
    for f in pass_no_icon.iter().take(12) {
        println!("  - {f}");
    }
    if !pass_no_icon.is_empty() {
        println!(
            "\n=> 结论：MONO_PROBE 只验ASCII + 制表符，**无法**区分这些字体。\
             \n   所以「字体选择器里选到了某个 family」并不保证它带图标 —— \
             fallback 到它们时图标会缺字形/被替换。\n   探针里画的图标的码位（U+F0xx）\
             属于 v3 区段；若字体图标在别的区段，请把NERD_RANGES 调宽再跑。"
        );
    }
}