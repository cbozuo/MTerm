// FreeRDP 协议方言与命令行（跨平台编译：Windows 下仅测试引用，运行时走 mstsc）。
use super::Account;

/// Which command line dialect a FreeRDP build speaks. FreeRDP 3 added the
/// optional `force` value to `/from-stdin` — read the credentials before
/// connecting instead of when the server asks for them — and FreeRDP 2
/// rejects that value, so the two cannot share one flag.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Dialect {
    /// FreeRDP 2.x: the `xfreerdp` binary on older distributions.
    V2,
    /// FreeRDP 3.x: `xfreerdp3`, and what the Flatpak bundle carries.
    V3,
}

/// Client binaries to try, in order.
///
/// `PATH` comes first, which is what someone who installed FreeRDP by hand
/// expects and where Flatpak puts the bundled client (`/app/bin`). The
/// absolute paths cover GUI launches: those can inherit a bare `PATH` that
/// misses `/usr/local/bin`, and macOS GUI apps never see `/opt/homebrew`.
pub(super) const CLIENTS: [&str; 9] = [
    "xfreerdp3",
    "xfreerdp",
    "/app/bin/xfreerdp3",
    "/usr/bin/xfreerdp3",
    "/usr/local/bin/xfreerdp3",
    "/opt/homebrew/bin/xfreerdp3",
    "/usr/bin/xfreerdp",
    "/usr/local/bin/xfreerdp",
    "/opt/homebrew/bin/xfreerdp",
];

/// Read the major version out of a `--version` banner such as
/// "This is FreeRDP version 3.21.0 (3.21.0)". `None` means the text says
/// nothing usable; the caller then assumes the older, always-valid flag.
pub(super) fn parse_dialect(version_output: &str) -> Option<Dialect> {
    let after = version_output.split_once("version")?.1;
    let major: u32 = after
        .trim_start()
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    Some(if major >= 3 { Dialect::V3 } else { Dialect::V2 })
}

/// Ask a client binary for its version. `None` also means "no such binary",
/// which is how the caller finds the next candidate.
pub(super) fn probe_dialect(client: &str) -> Option<Dialect> {
    for flag in ["--version", "/version"] {
        let Ok(output) = std::process::Command::new(client)
            .arg(flag)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
        else {
            return None;
        };
        if let Some(dialect) = parse_dialect(&String::from_utf8_lossy(&output.stdout)) {
            return Some(dialect);
        }
    }
    None
}

/// The command line for a FreeRDP connect.
///
/// The password is deliberately absent — it goes over stdin, see
/// [`credential_lines`]. User name and domain are not secrets: they sit in
/// `mstsc`'s `.rdp` file on Windows and in the session list in the UI.
pub(super) fn args(account: &Account<'_>, dialect: Dialect) -> Vec<String> {
    let mut args = vec![
        format!("/v:{}:{}", account.host, account.port),
        "/cert:ignore".to_string(),
        "/clipboard".to_string(),
        // Let the session follow the window when it is resized. FreeRDP
        // refuses to start when `+smart-sizing` is given as well — "Smart
        // sizing and dynamic resolution are mutually exclusive options" —
        // and following the window keeps the picture pixel sharp instead of
        // scaling it up, so this is the one we ask for.
        "+dynamic-resolution".to_string(),
    ];
    // Omitted when empty: `/u:` with no value does not mean "no user name".
    if !account.user.is_empty() {
        args.push(format!("/u:{}", account.user));
    }
    if !account.domain.is_empty() {
        args.push(format!("/d:{}", account.domain));
    }
    // Same display choice as the Windows path.
    if account.fullscreen {
        args.push("/f".to_string());
    } else {
        args.push(format!("/w:{}", account.width));
        args.push(format!("/h:{}", account.height));
    }
    args.push(match dialect {
        // `:force` reads stdin up front. Without it FreeRDP 3 only reads
        // when the server asks for credentials, which is where its
        // `/from-stdin` handling regressed (FreeRDP issue #10217).
        Dialect::V3 => "/from-stdin:force".to_string(),
        Dialect::V2 => "/from-stdin".to_string(),
    });
    args
}

/// The lines the client reads from stdin, in the order it asks for them.
///
/// FreeRDP only prompts for what the command line left out, so with a user
/// name in the arguments this is the password alone — no reliance on an
/// undocumented order of user name, password and domain.
pub(super) fn credential_lines(account: &Account<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    if account.user.is_empty() {
        lines.push(account.user.to_string());
    }
    lines.push(account.password.to_string());
    lines
}

/// True inside a Flatpak sandbox, where the client comes with the bundle in
/// `/app` rather than with the host.
pub(super) fn in_flatpak_sandbox() -> bool {
    std::env::var_os("FLATPAK_ID").is_some() || std::path::Path::new("/.flatpak-info").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> Account<'static> {
        Account {
            host: "10.0.0.5",
            port: 3389,
            user: "alice",
            domain: "CONTOSO",
            password: "s3cret",
            fullscreen: false,
            width: 1600,
            height: 900,
        }
    }

    #[test]
    fn dialect_follows_the_reported_major_version() {
        assert_eq!(
            parse_dialect("This is FreeRDP version 3.21.0 (3.21.0)"),
            Some(Dialect::V3)
        );
        assert_eq!(
            parse_dialect("This is FreeRDP version 2.11.5 (2.11.0)"),
            Some(Dialect::V2)
        );
        // Nothing usable → the caller falls back to the older dialect.
        assert_eq!(parse_dialect(""), None);
        assert_eq!(parse_dialect("xfreerdp: command not found"), None);
    }

    #[test]
    fn the_password_never_reaches_the_command_line() {
        for dialect in [Dialect::V2, Dialect::V3] {
            let args = args(&account(), dialect);
            assert!(
                !args.iter().any(|arg| arg.contains("s3cret")),
                "{dialect:?} leaked the password: {args:?}"
            );
            assert!(args.contains(&"/u:alice".to_string()));
            assert!(args.contains(&"/d:CONTOSO".to_string()));
            assert!(args.contains(&"/v:10.0.0.5:3389".to_string()));
        }
    }

    #[test]
    fn v3_forces_the_stdin_read_and_v2_must_not() {
        assert!(args(&account(), Dialect::V3).contains(&"/from-stdin:force".to_string()));
        let v2 = args(&account(), Dialect::V2);
        assert!(v2.contains(&"/from-stdin".to_string()));
        // FreeRDP 2 rejects a value there and would refuse to start.
        assert!(!v2.iter().any(|arg| arg.starts_with("/from-stdin:")));
    }

    /// FreeRDP aborts with "Smart sizing and dynamic resolution are
    /// mutually exclusive options" when both are on the command line, which
    /// is how the very first Deepin test run failed.
    #[test]
    fn only_one_resize_strategy_is_requested() {
        for dialect in [Dialect::V2, Dialect::V3] {
            let args = args(&account(), dialect);
            assert!(args.contains(&"+dynamic-resolution".to_string()));
            assert!(
                !args.iter().any(|arg| arg.contains("smart-sizing")),
                "{dialect:?} asked for both resize strategies: {args:?}"
            );
        }
    }

    #[test]
    fn windowed_and_full_screen_sizes_are_mutually_exclusive() {
        let mut account = account();
        let windowed = args(&account, Dialect::V3);
        assert!(windowed.contains(&"/w:1600".to_string()));
        assert!(windowed.contains(&"/h:900".to_string()));
        assert!(!windowed.contains(&"/f".to_string()));

        account.fullscreen = true;
        let full = args(&account, Dialect::V3);
        assert!(full.contains(&"/f".to_string()));
        assert!(!full
            .iter()
            .any(|arg| arg.starts_with("/w:") || arg.starts_with("/h:")));
    }

    #[test]
    fn an_empty_user_or_domain_is_not_sent_at_all() {
        let mut account = account();
        account.user = "";
        account.domain = "";
        let args = args(&account, Dialect::V3);
        assert!(!args
            .iter()
            .any(|arg| arg.starts_with("/u:") || arg.starts_with("/d:")));
    }

    #[test]
    fn credentials_are_only_the_fields_missing_from_the_command_line() {
        // User name (and domain) came with the arguments, so the password is
        // the only thing the client asks for.
        assert_eq!(credential_lines(&account()), vec!["s3cret".to_string()]);

        // Without a user name the client asks for it too, and an empty line
        // is the answer.
        let mut anonymous = account();
        anonymous.user = "";
        assert_eq!(
            credential_lines(&anonymous),
            vec![String::new(), "s3cret".to_string()]
        );
    }

    #[test]
    fn the_flatpak_bundle_is_looked_up_by_name_and_by_path() {
        assert!(CLIENTS.contains(&"xfreerdp3"));
        assert!(CLIENTS.contains(&"/app/bin/xfreerdp3"));
    }
}
