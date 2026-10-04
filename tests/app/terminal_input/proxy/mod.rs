use super::super::*;

// (#conn-path 2026-10-04) 旧单串代理 ↔ 五元组回填 + 四字段拼装。

#[test]
fn split_proxy_parts_recognises_schemes() {
    let (p, proto, h, port, u, pw) = split_proxy_parts("");
    assert_eq!((p.as_str(), proto.as_str(), h.as_str(), port.as_str(), u.as_str(), pw.as_str()),
        ("direct", "socks5", "", "", "", ""));

    assert_eq!(
        split_proxy_parts("http://10.0.0.1:1022"),
        ("proxy".into(), "http".into(), "10.0.0.1".into(), "1022".into(), "".into(), "".into())
    );
    assert_eq!(
        split_proxy_parts("socks5://127.0.0.1:1080"),
        ("proxy".into(), "socks5".into(), "127.0.0.1".into(), "1080".into(), "".into(), "".into())
    );
    // socks5h / socks / https 旧前缀兼容
    assert_eq!(split_proxy_parts("socks5h://h:1").1, "socks5");
    assert_eq!(split_proxy_parts("socks://h:1").1, "socks5");
    assert_eq!(split_proxy_parts("https://h:1").1, "http");
    // 无前缀旧配置 → 当作 SOCKS5（与 proxy.rs parse 默认一致）
    assert_eq!(
        split_proxy_parts("127.0.0.1:1080"),
        ("proxy".into(), "socks5".into(), "127.0.0.1".into(), "1080".into(), "".into(), "".into())
    );
}

#[test]
fn split_proxy_parts_keeps_special_chars_in_password() {
    // userinfo 从最后一个 @ 切、user 取首个 : 前：密码里的 @/: 都能还原。
    let (_, _, _, _, u, pw) = split_proxy_parts("socks5://admin:p@ss:word@10.0.0.8:1080");
    assert_eq!(u, "admin");
    assert_eq!(pw, "p@ss:word");
}

#[test]
fn assemble_proxy_direct_is_empty() {
    let d = draft_with_proxy("direct", "socks5", "10.0.0.8", "1080", "", "");
    assert_eq!(assemble_proxy(&d, ""), "");
}

#[test]
fn assemble_proxy_anonymous_and_authed() {
    let d = draft_with_proxy("proxy", "socks5", "10.0.0.8", "1080", "", "");
    assert_eq!(assemble_proxy(&d, ""), "socks5://10.0.0.8:1080");

    let d = draft_with_proxy("proxy", "http", "10.0.0.8", "8080", "admin", "secret");
    assert_eq!(assemble_proxy(&d, ""), "http://admin:secret@10.0.0.8:8080");

    // 端口留空 → 省略 :port
    let d = draft_with_proxy("proxy", "socks5", "10.0.0.8", "", "", "");
    assert_eq!(assemble_proxy(&d, ""), "socks5://10.0.0.8");
}

#[test]
fn assemble_proxy_blank_pass_reuses_saved_one() {
    // #10 惯例:编辑时代理密码不回显,留空 = 保留旧值。
    let mut old = Session::new_empty();
    old.proxy = "socks5://admin:old-secret@10.0.0.8:1080".into();
    let d = draft_with_proxy("proxy", "socks5", "10.0.0.8", "1080", "admin", "");
    assert_eq!(assemble_proxy(&d, old.proxy.as_str()), "socks5://admin:old-secret@10.0.0.8:1080");
}

#[test]
fn assemble_proxy_round_trips_through_split() {
    let d = draft_with_proxy("proxy", "socks5", "10.0.0.8", "1080", "admin", "p@ss");
    let url = assemble_proxy(&d, "");
    let (path, proto, host, port, user, pass) = split_proxy_parts(&url);
    assert_eq!((path.as_str(), proto.as_str(), host.as_str(), port.as_str(), user.as_str(), pass.as_str()),
        ("proxy", "socks5", "10.0.0.8", "1080", "admin", "p@ss"));
}

/// 组一个只填代理字段的 draft。
fn draft_with_proxy(
    path_type: &str,
    proto: &str,
    host: &str,
    port: &str,
    user: &str,
    pass: &str,
) -> SessionDraft {
    SessionDraft {
        id: "t".into(),
        name: "".into(),
        kind: "ssh".into(),
        host: "srv".into(),
        port: 22,
        user: "".into(),
        auth: "password".into(),
        password: "".into(),
        private_key_path: "".into(),
        private_key_inline: "".into(),
        private_key_inline_mode: false,
        proxy: "".into(),
        path_type: path_type.into(),
        proxy_type: proto.into(),
        proxy_host: host.into(),
        proxy_port: port.into(),
        proxy_user: user.into(),
        proxy_pass: pass.into(),
        group: "".into(),
        serial_port: "".into(),
        baud_rate: 115200,
        data_bits: 8,
        stop_bits: 1,
        parity: "none".into(),
        flow_control: "none".into(),
        rdp_domain: "".into(),
        rdp_resolution: "1280x720".into(),
        rdp_width: 1280,
        rdp_height: 720,
        encoding: "UTF-8".into(),
        vt100_drawing: true,
        disable_shell_integration: false,
        note: "".into(),
        jump_session_id: "".into(),
    }
}
