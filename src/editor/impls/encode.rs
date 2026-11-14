// (#editor-hifi) 编码检测与解码:UTF-8 优先 → BOM 识别 UTF-16 → chardetng 回退
// GBK/GB18030/Shift-JIS/Latin-1。中文服务器日志常见 GBK,直接 UTF-8 解码会乱码。


/// 二进制探测:前 8KB 出现 NUL 即视为二进制(拒绝打开,提示走 SFTP 面板下载)。
pub(crate) fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8192).any(|b| *b == 0)
}

/// 解码 → (文本, 编码显示名)。UTF-8 严格校验失败才走 chardetng。
pub(crate) fn decode(bytes: &[u8]) -> (String, &'static str) {
    // BOM 族(确定性,直接解码)
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (t, had_err) = encoding_rs::UTF_8.decode_without_bom_handling(bytes);
        if !had_err {
            return (t.trim_start_matches('\u{feff}').into(), "UTF-8 BOM");
        }
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (t, _, _) = encoding_rs::UTF_16LE.decode(bytes);
        return (t.into(), "UTF-16LE");
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (t, _, _) = encoding_rs::UTF_16BE.decode(bytes);
        return (t.into(), "UTF-16BE");
    }
    // UTF-8 严格验证:全 ASCII/UTF-8 合法 → UTF-8
    if std::str::from_utf8(bytes).is_ok() {
        return (String::from_utf8_lossy(bytes).into(), "UTF-8");
    }
    // chardetng 猜测(已排除合法 UTF-8,猜测结果大概率是 GB 系)
    let mut det = chardetng::EncodingDetector::new();
    det.feed(bytes, true);
    let enc = det.guess(None, true);
    let (t, _, had_err) = enc.decode(bytes);
    (
        t.into(),
        if had_err { "替换字符" } else { enc.name() },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_passthrough() {
        let (t, e) = decode("hello 你好".as_bytes());
        assert_eq!(e, "UTF-8");
        assert_eq!(t, "hello 你好");
    }

    #[test]
    fn gbk_fallback() {
        // "中文" 的 GBK 编码(0xD6 0xD0 0xCE 0xC4),非法 UTF-8
        let gbk: &[u8] = &[0xD6, 0xD0, 0xCE, 0xC4];
        let (t, e) = decode(gbk);
        assert!(e.contains("GB") || e.contains("EUC"), "实际猜到: {e}");
        assert_eq!(t, "中文");
    }

    #[test]
    fn utf16_bom() {
        // "ab" UTF-16LE BOM
        let le: &[u8] = &[0xFF, 0xFE, 0x61, 0x00, 0x62, 0x00];
        let (t, e) = decode(le);
        assert_eq!(e, "UTF-16LE");
        assert_eq!(t, "ab");
    }

    #[test]
    fn binary_detect() {
        assert!(is_binary(&[0x50, 0x4B, 0x00, 0x01]));
        assert!(!is_binary(b"plain text log\n"));
    }
}
