use super::*;

#[test]
fn inverse_default_colours_paint_a_visible_background() {
    // 默认档 = DEFAULT_DARK（#theme-dedupe 2026-10-15：门面 mterm-dark）。
    // 断言改成"跟随常量 + 具值快照"：以后若再换默认主题，这里会显式提示该改，
    // 而不是给出一盏误导性的红灯（高保真 §6.3 建议）。
    let default_pal = crate::theme::palette_or_default("", true);
    assert_eq!(default_pal.id, crate::theme::palettes::DEFAULT_DARK);
    // 默认档（mterm-dark）的观感回归：tbg=#181A1D / tfg=#BCBEC4
    assert_eq!(default_pal.tbg, 0x181A1D);
    assert_eq!(default_pal.tfg, 0xBCBEC4);
    let (fg, bg) = vt_span_colors(
        vt100::Color::Default,
        vt100::Color::Default,
        false,
        true,
        true,
        default_pal,
    );
    // inverse 交换前景/背景：fg 槽画 tbg(#181A1D)、bg 槽画 tfg(#BCBEC4)
    assert_eq!(fg.as_argb_encoded(), 0xff181a1d);
    assert_eq!(bg.as_argb_encoded(), 0xffbcbec4);

    let mut parser = vt100::Parser::new(3, 30, 0);
    parser.process(b"abc \x1b[7m20260705\x1b[27m end");
    let (_plain, runs, _wrapped) = build_row(parser.screen(), 0, 30);
    let hit = runs
        .iter()
        .find(|span| span.text.contains("20260705"))
        .expect("reverse-video search hit should be a separate span");
    assert!(hit.inverse);
    assert!(matches!(hit.fg, vt100::Color::Default));
    assert!(matches!(hit.bg, vt100::Color::Default));
}

/// MTerm 套（mterm-dark，上游 Islands Dark 移植）的终端 ANSI 回归：索引色
/// 必须原样查表命中该套 ansi16（槽序 0..7 = black red green yellow blue magenta
/// cyan white），不走任何明度改写。默认前景取该套 tfg（#BCBEC4）；默认背景
/// 按设计保持**全透明**（终端自身那层 Theme.term-bg 露出）——查表链路断了
/// 这里会先炸。
#[test]
fn mterm_dark_ansi_indices_come_from_the_theme() {
    let pal = crate::theme::by_id("mterm-dark").expect("MTerm 套必须在色表里");

    // 槽 1/2/4 分别取红/绿/蓝 = 上游 terminal.ansiRed/Green/Blue
    assert_eq!(
        vt_span_colors(vt100::Color::Idx(1), vt100::Color::Default, false, false, true, pal)
            .0
            .as_argb_encoded(),
        0xfff75464
    );
    assert_eq!(
        vt_span_colors(vt100::Color::Idx(2), vt100::Color::Default, false, false, true, pal)
            .0
            .as_argb_encoded(),
        0xff73b00a
    );
    assert_eq!(
        vt_span_colors(vt100::Color::Idx(4), vt100::Color::Default, false, false, true, pal)
            .0
            .as_argb_encoded(),
        0xff548af7
    );
    // 槽 12 = brightBlue
    assert_eq!(
        vt_span_colors(vt100::Color::Idx(12), vt100::Color::Default, false, false, true, pal)
            .0
            .as_argb_encoded(),
        0xff7cacf8
    );
    // 默认前景按该套 tfg 查表；默认背景**有意为全透明**——终端自身那层
    // Theme.term-bg 会露出（见 vt_bg_to_slint 的 Default 分支，字面就是
    // from_argb_u8(0,0,0,0)，不携带颜色）。这里同时钉住「透明」与「该套
    // tfg 的 RGB」两个事实，防止有人把默认底误改成不透明后，在浅色/壁纸
    // 主题下把终端底盖成另一块色。
    let (fg, bg) =
        vt_span_colors(vt100::Color::Default, vt100::Color::Default, false, false, true, pal);
    assert_eq!(fg.as_argb_encoded(), 0xffbcbec4);
    assert_eq!(bg.as_argb_encoded(), 0, "默认终端底必须是全透明、不携带颜色");
}

/// MTerm 亮色档「素纸」（mterm-light —— id 与 mterm-dark 配对，但色值是
/// MTerm 自主设计，非上游移植）的终端 ANSI 回归：索引色原样查表命中该套
/// ansi16，不走明度改写；**默认前景 = t1 = #33302C**（灰族唯一，与暗档
/// #BCBEC4 各一套），默认背景仍全透明。亮底上 bright 档是**更深**的同色相
/// （明度与暗底相反）—— 槽 12 若被改成更亮的色，这条会先炸。
#[test]
fn mterm_light_ansi_indices_come_from_the_theme() {
    let pal = crate::theme::by_id("mterm-light").expect("MTerm · 亮必须在色表里");

    // 槽 1/2/4/6 = 红/绿/蓝/青（青与强调色 #1f766b 同族，是该套的纪律之一）
    for (idx, argb) in [
        (1u8, 0xffb4483fu32),
        (2, 0xff4f7d4a),
        (4, 0xff4a6f96),
        (6, 0xff1c7a72),
    ] {
        assert_eq!(
            vt_span_colors(vt100::Color::Idx(idx), vt100::Color::Default, false, false, true, pal)
                .0
                .as_argb_encoded(),
            argb,
            "ANSI 槽 {idx} 查表值不对"
        );
    }
    // 槽 12 = brightBlue：亮底上是**更深**的同色相（#2f5580 < #4a6f96 明度）
    assert_eq!(
        vt_span_colors(vt100::Color::Idx(12), vt100::Color::Default, false, false, true, pal)
            .0
            .as_argb_encoded(),
        0xff2f5580
    );
    // 默认前景 = 该套 tfg（= t1，灰族唯一）；默认背景仍全透明
    let (fg, bg) =
        vt_span_colors(vt100::Color::Default, vt100::Color::Default, false, false, true, pal);
    assert_eq!(fg.as_argb_encoded(), 0xff33302c);
    assert_eq!(bg.as_argb_encoded(), 0, "默认终端底必须是全透明、不携带颜色");
}
