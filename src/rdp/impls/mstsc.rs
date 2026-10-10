// Windows：通过 mstsc 打开 —— 无保存密码时命令行直连（沿用 mstsc 自己的
// 凭据）；有保存密码时生成 .rdp 文件（DPAPI 加密密码，免提示）。
use super::{Account, RDP_DEFAULT_PORT};
use crate::i18n::t;
use std::process::Command;

/// Path of the per-session `.rdp` file handed to `mstsc`. One file per session,
/// overwritten on every connect, so repeated use never piles up temp files.
fn rdp_file_path(session_id: &str) -> std::path::PathBuf {
    // Session ids are UUIDs, so they are safe as a file name.
    std::env::temp_dir().join(format!("mterm-rdp-{session_id}.rdp"))
}

/// Build the `.rdp` payload for `account`.
fn rdp_file_contents(account: &Account<'_>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "full address:s:{}:{}\r\n",
        account.host, account.port
    ));
    if !account.user.is_empty() {
        out.push_str(&format!("username:s:{}\r\n", account.user));
    }
    if !account.domain.is_empty() {
        out.push_str(&format!("domain:s:{}\r\n", account.domain));
    }
    match protected_password(account.password) {
        Some(blob) => {
            out.push_str(&format!("password 51:b:{blob}\r\n"));
            // Credentials are complete: do not stop on mstsc's prompt.
            out.push_str("prompt for credentials:i:0\r\n");
        }
        // If DPAPI fails, let mstsc ask instead of passing an invalid blob.
        // Unsaved passwords bypass the .rdp file in start_client.
        None => out.push_str("prompt for credentials:i:1\r\n"),
    }
    // Display: either full screen — mstsc then uses the local monitor
    // resolution, which is the sharpest option — or a window at the configured
    // desktop size, scaled when the window is resized instead of scrolling.
    if account.fullscreen {
        out.push_str("screen mode id:i:2\r\n");
    } else {
        out.push_str("screen mode id:i:1\r\n");
        out.push_str(&format!("desktopwidth:i:{}\r\n", account.width));
        out.push_str(&format!("desktopheight:i:{}\r\n", account.height));
        out.push_str("smart sizing:i:1\r\n");
    }
    // Local clipboard shared (what a launcher is expected to give you), and no
    // certificate warning for servers whose RDP certificate is self-signed —
    // the usual case for a standalone Windows box.
    out.push_str("redirectclipboard:i:1\r\n");
    out.push_str("authentication level:i:0\r\n");
    out.push_str("session bpp:i:32\r\n");
    out.push_str("compression:i:1\r\n");
    out
}

/// Encrypt `plain` into the hex-encoded DPAPI blob an `.rdp` file expects.
/// `None` means "no password to pass" (empty, or DPAPI is unavailable).
fn protected_password(plain: &str) -> Option<String> {
    if plain.is_empty() {
        return None;
    }
    let mut wide: Vec<u16> = plain.encode_utf16().collect();
    // A terminating NUL is what a C-style reader expects; mstsc accepts both.
    wide.push(0);

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            p_data_in: *const DataBlob,
            sz_data_descr: *const u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut core::ffi::c_void,
            p_prompt_struct: *mut core::ffi::c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;
        fn LocalFree(h_mem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    }

    let input = DataBlob {
        cb_data: u32::try_from(wide.len()).ok()? * 2,
        pb_data: wide.as_mut_ptr().cast::<u8>(),
    };
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: std::ptr::null_mut(),
    };
    // SAFETY: `input` describes a live buffer, `output` is only written by the
    // call, and both pointers stay valid for its duration. No entropy and no
    // LOCAL_MACHINE flag: that is exactly what mstsc uses when it decrypts.
    let ok = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            &mut output,
        )
    };
    if ok == 0 || output.pb_data.is_null() {
        return None;
    }
    // SAFETY: the call succeeded, so `output` owns `cb_data` readable bytes.
    let bytes = unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize) };
    let hex: String = bytes.iter().map(|byte| format!("{byte:02X}")).collect();
    // SAFETY: the buffer came from CryptProtectData, which requires LocalFree.
    unsafe {
        LocalFree(output.pb_data.cast::<core::ffi::c_void>());
    }
    Some(hex)
}

fn mstsc_args_without_password(account: &Account<'_>) -> Vec<String> {
    let target = if account.port == RDP_DEFAULT_PORT {
        account.host.to_string()
    } else {
        format!("{}:{}", account.host, account.port)
    };
    let mut args = vec![format!("/v:{target}")];
    if account.fullscreen {
        args.push("/f".to_string());
    } else {
        args.push(format!("/w:{}", account.width));
        args.push(format!("/h:{}", account.height));
    }
    args
}

pub(super) fn start_client(account: &Account<'_>, session_id: &str) -> Result<String, String> {
    let mut command = Command::new("mstsc");
    if account.password.is_empty() {
        // Like `mstsc /v:host`: do not force a credential prompt or override
        // the user's saved account/domain with fields from a fresh .rdp file.
        command.args(mstsc_args_without_password(account));
    } else {
        let path = rdp_file_path(session_id);
        std::fs::write(&path, rdp_file_contents(account)).map_err(|err| {
            format!(
                "{} {}: {err}",
                t("无法写入连接文件", "cannot write the connection file"),
                path.display()
            )
        })?;
        command.arg(path);
    }
    command.spawn().map_err(|err| {
        format!(
            "{} mstsc: {err}",
            t("无法启动系统远程桌面", "cannot start the system remote desktop client")
        )
    })?;
    Ok(format!("{}:{} (mstsc)", account.host, account.port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_password_uses_mstsc_address_without_overriding_credentials() {
        let mut account = Account {
            host: "192.168.0.69",
            port: 3389,
            user: "Administrator",
            domain: "LAB",
            password: "",
            fullscreen: false,
            width: 1920,
            height: 1080,
        };
        assert_eq!(
            mstsc_args_without_password(&account),
            vec![
                "/v:192.168.0.69".to_string(),
                "/w:1920".to_string(),
                "/h:1080".to_string(),
            ]
        );
        account.port = 3390;
        account.fullscreen = true;
        assert_eq!(
            mstsc_args_without_password(&account),
            vec!["/v:192.168.0.69:3390".to_string(), "/f".to_string()]
        );
    }

    #[test]
    fn rdp_file_carries_saved_password_and_account() {
        let account = Account {
            host: "192.168.1.10",
            port: 3389,
            user: "alice",
            domain: "CONTOSO",
            password: "s3cret",
            fullscreen: false,
            width: 1920,
            height: 1080,
        };
        let contents = rdp_file_contents(&account);
        assert!(contents.contains("full address:s:192.168.1.10:3389\r\n"));
        assert!(contents.contains("username:s:alice\r\n"));
        assert!(contents.contains("domain:s:CONTOSO\r\n"));
        assert!(contents.contains("prompt for credentials:i:0\r\n"));
        assert!(contents.contains("password 51:b:"));
        // Windowed at the configured desktop size.
        assert!(contents.contains("screen mode id:i:1\r\n"));
        assert!(contents.contains("desktopwidth:i:1920\r\n"));
        assert!(contents.contains("desktopheight:i:1080\r\n"));
    }

    /// Full screen lets mstsc use the local monitor resolution, so no desktop
    /// size may be requested — otherwise the session would be scaled.
    #[test]
    fn full_screen_does_not_request_a_desktop_size() {
        let account = Account {
            host: "server",
            port: 3389,
            user: "",
            domain: "",
            password: "",
            fullscreen: true,
            width: 1280,
            height: 720,
        };
        let contents = rdp_file_contents(&account);
        assert!(contents.contains("screen mode id:i:2\r\n"));
        assert!(!contents.contains("desktopwidth:i:"));
        assert!(!contents.contains("desktopheight:i:"));
        // An empty user name must not be written as `username:s:`.
        assert!(!contents.contains("username:s:"));
    }

    /// A stored password must produce a blob mstsc can decrypt, which also
    /// proves the DPAPI call itself is wired up correctly.
    #[test]
    fn stored_password_round_trips_through_dpapi() {
        #[repr(C)]
        struct DataBlob {
            cb_data: u32,
            pb_data: *mut u8,
        }

        #[link(name = "crypt32")]
        extern "system" {
            fn CryptUnprotectData(
                p_data_in: *const DataBlob,
                ppsz_data_descr: *mut *mut u16,
                p_optional_entropy: *const DataBlob,
                pv_reserved: *mut core::ffi::c_void,
                p_prompt_struct: *mut core::ffi::c_void,
                dw_flags: u32,
                p_data_out: *mut DataBlob,
            ) -> i32;
            fn LocalFree(h_mem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
        }

        let account = Account {
            host: "127.0.0.1",
            port: 3389,
            user: "alice",
            domain: "",
            password: "p@ssw0rd-示例",
            fullscreen: false,
            width: 1280,
            height: 720,
        };
        let contents = rdp_file_contents(&account);
        let line = contents
            .lines()
            .find_map(|line| line.strip_prefix("password 51:b:"))
            .expect("a stored password must be written");
        assert!(contents.contains("prompt for credentials:i:0\r\n"));

        let blob: Vec<u8> = (0..line.len() / 2)
            .map(|i| u8::from_str_radix(&line[i * 2..i * 2 + 2], 16).expect("hex blob"))
            .collect();
        let input = DataBlob {
            cb_data: blob.len() as u32,
            pb_data: blob.as_ptr() as *mut u8,
        };
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: std::ptr::null_mut(),
        };
        // SAFETY: same contract as the encryption side.
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut output,
            )
        };
        assert_ne!(ok, 0, "the blob must be decryptable by this user");
        // SAFETY: the call succeeded, so the buffer is readable.
        let plain = unsafe {
            std::slice::from_raw_parts(output.pb_data, output.cb_data as usize)
        };
        let wide: Vec<u16> = plain
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let decoded = String::from_utf16_lossy(&wide);
        // SAFETY: the buffer came from CryptUnprotectData.
        unsafe {
            LocalFree(output.pb_data.cast::<core::ffi::c_void>());
        }
        assert_eq!(
            decoded.trim_end_matches('\0'),
            "p@ssw0rd-示例",
            "the round-tripped password must match"
        );
    }
}
