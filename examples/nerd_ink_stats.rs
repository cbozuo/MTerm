#!/usr/bin/env cargo run --release --example nerd_ink_stats
//! (#nerd-icon-unify) 诊断：Nerd Font 图标墨迹(w/h, em)逐字形实测。
//!
//! 背景：`#nerd-icon-scale` 用统一倍数 1.15 放大所有私用区图标，但不同图标
//! 族（FontAwesome / Material Design / Octicons / Seti…）在同一 em 方框里的
//! 墨迹占幅差异极大（文档类 ~0.77em，文件夹/齿轮类 ~0.50em），统一倍数救不
//! 了矮个子 —— 用户看到「有的图标正常、有的显小」。要做逐字形归一化，先拿
//! 真实字体的逐字形墨迹数据定目标值/上限。
//!
//! 输出：
//!   ① 每个含 PUA 图标的 family：图标字形数 + 墨迹高度分位数；
//!   ② 常见 lsd/eza 图标码位的 w/h em 与拟采用的归一化倍数。
//!
//! 用法：`cargo run --release --example nerd_ink_stats`

use fontdb::{Database, Source};
use skrifa::charmap::Charmap;
use skrifa::metrics::GlyphMetrics;
use skrifa::{FontRef};

/// 拟定常数（与实现保持一致，先在这里看数据再定稿）。
const TARGET_H: f32 = 0.80; // 统一后的墨迹高度（em）
const CAP_H: f32 = 0.92; // 墨迹高度上限（em），防上下溢出行
const MIN_S: f32 = 1.0; // 不低于现状倍数（高图标不回缩）
const MAX_S: f32 = 1.6; // 兜底上限
/// 宽度上限：墨迹宽 ≤ 1.5 个格宽（NFM 格宽 = 0.6em），防横向压到文件名。
const CAP_W_EM: f32 = 0.96;

/// lsd/eza 常见图标码位（nf-fa-* / nf-md-*）。
const CURATED: &[(u32, &str)] = &[
    (0xF024B, "md-folder"),
    (0xF07B, "fa-folder"),
    (0xF07C, "fa-folder-open"),
    (0xF115, "fa-folder-open-o"),
    (0xF013, "fa-cog"),
    (0xF0493, "md-cog"),
    (0xF021, "fa-refresh"),
    (0xF15C, "fa-file-text-o"),
    (0xF0F6, "fa-file-o"),
    (0xF016, "fa-file-text"),
    (0xF1C9, "fa-file-archive-o"),
    (0xF121, "fa-code"),
    (0xF06A9, "md-language-java"),
    (0xE75D, "dev-java"),
    (0xF0727, "md-file"),
    (0xF0A30, "fa-database"),
    (0xF1C0, "fa-database-alt"),
    (0xF0493, "md-settings"),
];

fn face_bytes(db: &Database, id: fontdb::ID) -> Option<Vec<u8>> {
    let face = db.face(id)?;
    match &face.source {
        Source::Binary(b) => Some(b.as_ref().as_ref().to_vec()),
        Source::SharedFile(_, b) => Some(b.as_ref().as_ref().to_vec()),
        Source::File(p) => std::fs::read(p).ok(),
    }
}

fn is_pua(cp: u32) -> bool {
    (0xE000..=0xF8FF).contains(&cp) || (0xF0000..=0xFFFFD).contains(&cp)
}

/// 返回 (family, 图标数, 高度分位, 若干样本)
fn probe(db: &Database, fam: &str) -> Option<()> {
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
    let gm = GlyphMetrics::new(&font, skrifa::instance::Size::new(1.0), skrifa::instance::LocationRef::default());
    let mut hs: Vec<f32> = Vec::new();
    let mut ws: Vec<f32> = Vec::new();
    for (cp, gid) in cmap.mappings() {
        if !is_pua(cp) || (0xE0A0..=0xE0D4).contains(&cp) {
            continue;
        }
        let Some(b) = gm.bounds(gid) else { continue };
        let w = b.x_max - b.x_min;
        let h = b.y_max - b.y_min;
        if w > 0.001 && h > 0.001 {
            ws.push(w);
            hs.push(h);
        }
    }
    if hs.is_empty() {
        return None;
    }
    hs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ws.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |v: &[f32], t: f32| -> f32 { v[((v.len() as f32 * t) as usize).min(v.len() - 1)] };
    println!(
        "{:<40} 图标 {:>5}  h p10 {:.2} p50 {:.2} p90 {:.2}  w p50 {:.2} p90 {:.2}",
        fam,
        hs.len(),
        q(&hs, 0.10),
        q(&hs, 0.50),
        q(&hs, 0.90),
        q(&ws, 0.50),
        q(&ws, 0.90),
    );

    // 策划码位明细（只对第一个命中的 family 打，避免刷屏）
    if fam.contains("Maple") || fam.contains("maple") || fam.to_lowercase().contains("nerd") {
        println!("    ── 常见图标（h=墨高 w=墨宽 → 归一化倍数）──");
        for &(cp, name) in CURATED {
            let Some(c) = char::from_u32(cp) else { continue };
            let Some(gid) = cmap.map(c) else { continue };
            let Some(b) = gm.bounds(gid) else { continue };
            let w = b.x_max - b.x_min;
            let h = b.y_max - b.y_min;
            if h <= 0.001 {
                continue;
            }
            let s_h = (TARGET_H / h).clamp(MIN_S, MAX_S);
            let s = s_h
                .min(CAP_H / h)
                .min(CAP_W_EM / w.max(0.01))
                .min(MAX_S);
            println!(
                "      U+{:05X} {:<16} h {:.2}  w {:.2}  → scale {:.2}  (墨高 {:.2}em 墨宽 {:.2}em)",
                cp, name, h, w, s, h * s, w * s
            );
        }
    }
    Some(())
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
    println!("=== 含 PUA 图标的字体族 ===");
    for fam in &fams {
        let l = fam.to_lowercase();
        if l.contains("nerd") || l.contains("nf") || l.contains("maple") || l.contains("mono") {
            probe(&db, fam);
        }
    }
}
