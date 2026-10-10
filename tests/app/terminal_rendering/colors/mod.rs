use super::*;

#[test]
fn inverse_default_colours_paint_a_visible_background() {
    let (fg, bg) = vt_span_colors(
        vt100::Color::Default,
        vt100::Color::Default,
        false,
        true,
        true,
        // 默认档（graphite-dark）的观感回归：tbg=#111111 / tfg=#eeeeee
        crate::theme::palette_or_default("", true),
    );
    // inverse 交换前景/背景：fg 槽画 tbg(#111111)、bg 槽画 tfg(#eeeeee)
    assert_eq!(fg.as_argb_encoded(), 0xff111111);
    assert_eq!(bg.as_argb_encoded(), 0xffeeeeee);

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

/// MTerm 套（islands-dark，上游 Islands Dark 移植）的终端 ANSI 回归：索引色
/// 必须原样查表命中该套 ansi16（槽序 0..7 = black red green yellow blue magenta
/// cyan white），不走任何明度改写。默认前景取该套 tfg（#BCBEC4）；默认背景
/// 按设计保持**全透明**（终端自身那层 Theme.term-bg 露出）——查表链路断了
/// 这里会先炸。
#[test]
fn islands_dark_ansi_indices_come_from_the_theme() {
    let pal = crate::theme::by_id("islands-dark").expect("MTerm 套必须在色表里");

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
