// Entry point. Wires the Slint UI to the config store, system sampler and
// SSH session manager.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod allocator;

#[global_allocator]
static GLOBAL: allocator::Allocator = allocator::Allocator;
mod app;
mod automation;
mod cli;
mod config;
mod datastore;
mod i18n;
mod layout;
mod logging;
mod mcp;
mod picker;
mod rdp;
mod remote;
mod resource;
mod session;
mod sftp;
mod ssh;
mod terminal;
mod theme;
mod tunnel;
mod ui;
mod vnc;
mod wallpaper;
mod webdav;

enum StartMode {
    Mcp,
    Cli,
    App,
    Version,
}

impl StartMode {
    fn detect(args: &[String]) -> Self {
        match args.get(1).map(String::as_str) {
            Some("mcp") if args.get(2).is_some_and(|arg| arg == "serve") => Self::Mcp,
            Some("cli") => Self::Cli,
            _ if args.iter().any(|arg| arg == "--version" || arg == "-V") => Self::Version,
            _ => Self::App,
        }
    } 
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let mode = StartMode::detect(&args);
    if matches!(mode, StartMode::Version) {
        println!("MTerm {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    init_tracing();
    install_rustls_crypto_provider();

    match mode {
        StartMode::Mcp => mcp::run_stdio(),
        StartMode::Cli => cli::run(&args),
        StartMode::App => {
            // macOS defaults to Slint's CPU renderer. FemtoVG and Skia remain available
            // in Settings -> Interface -> Rendering for users who prefer GPU rendering.
            //
            // History: 0.4.10 force-set SLINT_BACKEND=winit-skia to work around femtovg's
            // CoreText font lookup failing on macOS 26 / Tahoe (all text vanished, #108).
            // That fix shipped without on-device verification and turned out to *break* a
            // different set of Macs (Apple Silicon M5 / 26.5): Skia couldn't resolve the
            // "PingFang SC" UI font and all text vanished there instead (#129). Icons
            // survived in both cases because Material Icons is an embedded font.
            //
            // Neither GPU renderer works for every macOS machine, so software rendering
            // is the compatibility default. Users can select FemtoVG or Skia under
            // Settings -> Interface -> Rendering. The
            // SLINT_BACKEND=winit-skia diagnostic override remains available and takes
            // precedence over the saved setting. The renderer-skia feature is compiled in
            // on macOS (see Cargo.toml), so switching does not require a rebuild.

            // ── IME policy ───────────────────────────────────────────────────────────
            // NOTE: We deliberately DO **NOT** call `ImmDisableIME` here.
            //
            // An earlier version disabled the IME for the whole Slint event-loop thread
            // to work around a vim `:q!` glitch (Chinese IMEs intercept letter keys and,
            // on a Shift press, discard the in-flight pinyin).  But disabling the IME
            // also makes 中文输入 completely impossible — there is no composition window
            // at all, which is exactly the "无法输入任何中文" bug.
            //
            // Chinese input now flows through the hidden `ime-input` TextInput in
            // terminal_view.slint: composition happens there, and committed text is
            // forwarded to the PTY via the `edited` callback.  The vim/Shift side-effects
            // are handled instead by the C0-marker + 3-layer Backspace filters in
            // `app::on_send_key`, so we no longer need (and must not use) ImmDisableIME.
            let intent = app::launch::parse(&args);
            app::run(intent)
        }
        StartMode::Version => unreachable!("handled above"),
    }
}

/// Pick the crypto provider rustls uses for the whole process, before anything
/// can ask for a `ClientConfig`.
///
/// This binary ends up with **both** of rustls's provider features turned on:
/// `ring`, because this crate and `ureq` ask for it, and `aws_lc_rs`, because
/// `ironrdp = ["client", .., "rustls"]` pulls in `ironrdp-tls`, which asks for
/// that one instead. With two features enabled rustls refuses to choose:
/// `CryptoProvider::from_crate_features()` returns `None`, and every
/// `ClientConfig::builder()` panics with "Could not automatically determine the
/// process-level CryptoProvider from Rustls crate features".
///
/// That panic is fatal here in a way it would not be in a debug build. The
/// release profile sets `panic = "abort"`, so clicking an RDP session killed the
/// process before a window, a state or a log line existed — the whole
/// application simply vanished, which is exactly how it was reported.
///
/// `install_default()` is the switch rustls expects for this: once a provider is
/// installed, every `builder()` uses it, no matter who is calling — our own
/// certificate probe (`rdp::embedded::verifier`), `ironrdp-tls`'s internal
/// config, `tokio-rustls`, or `ureq` for WebDAV. Installing it at the one place
/// that runs before all of them is what makes that true; patching only our own
/// call site would leave ironrdp's TLS — the part of the chain we cannot
/// change — still able to take the process down.
///
/// `ring` is the provider to pick: it is the one the WebDAV path already used
/// (`webdav`'s `builder_with_provider` call), so this keeps the process on a
/// single provider rather than introducing a second one.
///
/// A failure is ignored on purpose. The only way `install_default` fails is that
/// a provider was already installed, which is precisely the state this function
/// exists to produce — `main` and the tests can both reach it.
fn install_rustls_crypto_provider() {
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        tracing::debug!("a rustls crypto provider was already installed");
    }
}

/// Set up tracing: stderr (honours RUST_LOG, default info) **plus** a capped
/// `error.log` file at WARN and above so users can send diagnostics — e.g. a
/// bastion disconnect reason — without setting RUST_LOG (#86).
fn init_tracing() {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    // Third-party noise routed through `log` → tracing: ICU4X data-error warnings
    // (icu_provider dependency) and fontdb's "malformed font" warning for fonts it
    // can't parse but harmlessly skips (e.g. Windows' mstmc.ttf). Silence on every
    // layer; keep fontdb at `error` so genuine failures still surface.
    fn quiet_noise(mut f: EnvFilter) -> EnvFilter {
        for d in [
            "icu_provider=off",
            "icu_segmenter=off",
            "icu_normalizer=off",
            "fontdb=error",
        ] {
            if let Ok(dir) = d.parse() {
                f = f.add_directive(dir);
            }
        }
        f
    }

    let env_filter =
        quiet_noise(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")));
    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(env_filter);

    // One file, capped at 50 MiB, auto-overwriting when full (5 MiB was too
    // small to diagnose anything useful).
    let file_layer = logging::path()
        .and_then(|p| logging::CappedFile::open(p, 50 * 1024 * 1024).ok())
        .map(|cf| {
            fmt::layer()
                .with_ansi(false)
                .with_writer(logging::CappedWriter::new(cf))
                .with_filter(quiet_noise(EnvFilter::new("warn")))
        });

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(file_layer)
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_start_mode() {
        let mcp = vec![
            "MTerm".to_string(),
            "mcp".to_string(),
            "serve".to_string(),
        ];
        assert!(matches!(StartMode::detect(&mcp), StartMode::Mcp));

        let cli = vec![
            "MTerm".to_string(),
            "cli".to_string(),
            "sessions".to_string(),
        ];
        assert!(matches!(StartMode::detect(&cli), StartMode::Cli));

        let version = vec!["MTerm".to_string(), "--version".to_string()];
        assert!(matches!(StartMode::detect(&version), StartMode::Version));

        let app = vec!["MTerm".to_string()];
        assert!(matches!(StartMode::detect(&app), StartMode::App));
    }

    /// The provider has to be installed for `ClientConfig::builder()` to work at
    /// all, and `builder()` is what both of our TLS paths reach without naming a
    /// provider.
    ///
    /// This runs the install itself rather than trusting `main` to have done it,
    /// because the test binary never calls `main` — which is also why every test
    /// that builds a TLS config has to ask for the provider explicitly.
    #[test]
    fn rustls_gets_a_default_crypto_provider() {
        install_rustls_crypto_provider();
        assert!(
            rustls::crypto::CryptoProvider::get_default().is_some(),
            "no provider was installed, so any ClientConfig::builder() would panic"
        );
        // Building one is the call that panics when rustls cannot pick a
        // provider; reaching the next line is the assertion.
        let _ = rustls::ClientConfig::builder();
        // A second install must stay quiet instead of undoing the first.
        install_rustls_crypto_provider();
        let _ = rustls::ClientConfig::builder();
    }
}
