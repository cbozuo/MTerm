//! Persistent trust decisions for remote-desktop servers (P0-12).
//!
//! A built-in RDP/VNC client necessarily accepts a self-signed certificate to
//! connect at all, so the interesting question is not "is this certificate
//! valid" but "is this the same certificate we saw last time". The store keys
//! on `(protocol, host, port)` and holds the SHA-256 fingerprint, which makes
//! three outcomes distinguishable:
//!
//! * never seen -> ask, and remember only if the user says to;
//! * seen with the same fingerprint -> connect silently;
//! * seen with a **different** fingerprint -> ask again and say so loudly,
//!   because that is either a re-imaged host or a man in the middle, and the
//!   two deserve different reactions.
//!
//! A mismatch is never downgraded to "connect anyway": the decision function
//! returns `Reject` and the client aborts unless the user explicitly accepts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// One remembered server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedPeer {
    /// SHA-256 of the server certificate (RDP) or key (VNC), as lowercase hex.
    pub fingerprint: String,
    /// Subject / issuer strings, kept only so the prompt can show what changed.
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub issuer: Option<String>,
    /// Unix seconds, or 0 when the source did not expose validity.
    #[serde(default)]
    pub valid_from: i64,
    #[serde(default)]
    pub valid_to: i64,
}

/// The on-disk shape. Versioned so a future format change can be detected
/// rather than silently mis-parsed.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustFile {
    version: u32,
    peers: HashMap<String, TrustedPeer>,
}

/// What the client should do about the certificate it just saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustOutcome {
    /// Fingerprint matches what we remembered: connect without asking.
    Trusted,
    /// First time we have seen this host: show the prompt.
    PromptUnknown,
    /// We have seen this host but the fingerprint differs: show the prompt and
    /// say the certificate changed.
    PromptChanged,
    /// Policy forbids connecting (strict mode with an unknown host, or a
    /// fingerprint mismatch under a policy that does not allow overriding).
    Reject,
}

/// In-memory trust store plus its persistence file.
///
/// Loading is best-effort: a missing or corrupt file is treated as "nothing
/// trusted yet", which produces a prompt rather than a wrong connection.
#[derive(Debug, Clone)]
pub struct TrustStore {
    peers: HashMap<String, TrustedPeer>,
    path: Option<PathBuf>,
}

const TRUST_FILE_VERSION: u32 = 1;

impl TrustStore {
    /// An in-memory store with no file behind it (used by tests and by the
    /// "don't remember" path).
    pub fn in_memory() -> Self {
        Self {
            peers: HashMap::new(),
            path: None,
        }
    }

    /// Load from `path`, creating nothing on disk until a decision is saved.
    pub fn load(path: &Path) -> Self {
        let peers = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<TrustFile>(&text).ok())
            .filter(|file| file.version == TRUST_FILE_VERSION)
            .map(|file| file.peers)
            .unwrap_or_default();
        Self {
            peers,
            path: Some(path.to_path_buf()),
        }
    }

    /// Decide what to do with a certificate the server just presented.
    ///
    /// `policy` is `"prompt"` (ask about anything untrusted), `"accept-new"`
    /// (ask only for hosts never seen) or `"strict"` (refuse anything not
    /// already trusted).
    pub fn evaluate(&self, protocol: &str, host: &str, port: u16, fingerprint: &str, policy: &str) -> TrustOutcome {
        match self.peers.get(&key(protocol, host, port)) {
            Some(known) if known.fingerprint.eq_ignore_ascii_case(fingerprint) => {
                TrustOutcome::Trusted
            }
            Some(_) => {
                // A changed fingerprint is a security event, never a silent
                // pass, in every policy.
                if policy == "strict" {
                    TrustOutcome::Reject
                } else {
                    TrustOutcome::PromptChanged
                }
            }
            None => {
                if policy == "strict" {
                    TrustOutcome::Reject
                } else {
                    TrustOutcome::PromptUnknown
                }
            }
        }
    }

    /// Record (or replace) the fingerprint for a peer.
    ///
    /// Replacing on a changed fingerprint is what makes "trust this new
    /// certificate" a real decision: the old one is gone, so a later change
    /// back to it prompts again.
    pub fn remember(&mut self, protocol: &str, host: &str, port: u16, peer: TrustedPeer) {
        self.peers.insert(key(protocol, host, port), peer);
        self.save();
    }

    /// Forget a peer, so the next connection prompts again.
    pub fn forget(&mut self, protocol: &str, host: &str, port: u16) {
        self.peers.remove(&key(protocol, host, port));
        self.save();
    }

    /// True when this exact fingerprint is remembered for this peer.
    pub fn is_trusted(&self, protocol: &str, host: &str, port: u16, fingerprint: &str) -> bool {
        self.peers
            .get(&key(protocol, host, port))
            .map(|p| p.fingerprint.eq_ignore_ascii_case(fingerprint))
            .unwrap_or(false)
    }

    /// Persist, if this store has a file. Failures are logged, never fatal: the
    /// connection has already been decided, and losing the record only costs
    /// one extra prompt next time.
    fn save(&self) {
        let Some(path) = self.path.as_ref() else {
            return;
        };
        let file = TrustFile {
            version: TRUST_FILE_VERSION,
            peers: self.peers.clone(),
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(err) = std::fs::write(path, serde_json::to_string_pretty(&file).unwrap_or_default())
        {
            tracing::warn!("could not save remote-desktop trust store: {err:#}");
        }
    }
}

/// Storage key. The protocol is part of the key on purpose: an RDP server and a
/// VNC server on the same host:port are different services with different
/// certificates, and trusting one must not vouch for the other.
fn key(protocol: &str, host: &str, port: u16) -> String {
    format!("{protocol}://{host}:{port}")
}

/// Where the trust store lives.
///
/// A separate file rather than a field in `sessions.json`: trust decisions are
/// machine-wide facts about servers, not properties of one saved session, and a
/// per-session field would make the same server prompt again from another entry.
pub fn trust_store_path() -> PathBuf {
    // `datastore::config_dir` is what sessions.json uses, so the trust store
    // lands in the same place a user would look for it.
    crate::datastore::config_dir().join("remote-trust.json")
}

/// A pending decision, keyed by request id.
///
/// The record has to outlive the prompt's own call frame — the driver raises it,
/// hands it to the UI, and then blocks on the answer — so it lives here rather
/// than on the stack. It also carries everything needed to write the store
/// entry when the user says "remember", which is what keeps the write on the
/// decision path instead of on a UI callback that may never run.
#[derive(Debug, Clone)]
struct PendingDecision {
    protocol: String,
    host: String,
    port: u16,
    peer: TrustedPeer,
}

/// Pending decisions, keyed by request id. A `OnceLock` rather than a plain
/// `static` because `HashMap::new` is not a `const fn`.
static PENDING: std::sync::OnceLock<Mutex<std::collections::HashMap<u64, PendingDecision>>> =
    std::sync::OnceLock::new();

fn pending() -> &'static Mutex<std::collections::HashMap<u64, PendingDecision>> {
    PENDING.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// The process-wide trust store, loaded once.
///
/// Every session in every window consults the same file, so a server trusted
/// from one tab is trusted from another — which is the whole point of keying on
/// `(protocol, host, port)` rather than on a session id.
fn global() -> &'static Mutex<TrustStore> {
    static STORE: std::sync::OnceLock<Mutex<TrustStore>> = std::sync::OnceLock::new();
    STORE.get_or_init(|| {
        Mutex::new(TrustStore::load(&trust_store_path()))
    })
}

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Decide whether a server may be connected to, without prompting.
///
/// This is the non-interactive half of the decision: it says whether the store
/// already vouches for the fingerprint, or whether the caller must ask. Callers
/// that get [`TrustOutcome::PromptUnknown`] or [`TrustOutcome::PromptChanged`]
/// raise a prompt with [`open_decision`] and wait for [`resolve_decision`].
pub fn authorize(
    protocol: &str,
    host: &str,
    port: u16,
    fingerprint: &str,
    policy: &str,
) -> TrustOutcome {
    match global().lock() {
        Ok(store) => store.evaluate(protocol, host, port, fingerprint, policy),
        // A poisoned lock must never turn into a silent accept.
        Err(_) => TrustOutcome::Reject,
    }
}

/// Whether this exact fingerprint is remembered for this peer.
pub fn is_trusted(protocol: &str, host: &str, port: u16, fingerprint: &str) -> bool {
    matches!(
        global().lock(),
        Ok(store) if store.is_trusted(protocol, host, port, fingerprint)
    )
}

/// Register a pending decision and return its id.
pub fn open_decision(
    protocol: &str,
    host: &str,
    port: u16,
    fingerprint: &str,
    subject: Option<String>,
    issuer: Option<String>,
) -> u64 {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut map) = pending().lock() {
        map.insert(
            id,
            PendingDecision {
                protocol: protocol.to_string(),
                host: host.to_string(),
                port,
                peer: TrustedPeer {
                    fingerprint: fingerprint.to_string(),
                    subject,
                    issuer,
                    valid_from: 0,
                    valid_to: 0,
                },
            },
        );
    }
    id
}

/// Apply the user's answer to a pending decision.
///
/// `remember` only takes effect together with `accepted`: refusing a certificate
/// must never also write it to the store. Returns `false` for an unknown id,
/// which is the normal outcome when the tab was closed while the prompt was up.
pub fn resolve_decision(request_id: u64, accepted: bool, remember: bool) -> bool {
    let Some(decision) = pending().lock().ok().and_then(|mut m| m.remove(&request_id)) else {
        return false;
    };
    if !accepted || !remember {
        return true;
    }
    let Ok(mut store) = global().lock() else {
        return false;
    };
    store.remember(
        &decision.protocol,
        &decision.host,
        decision.port,
        decision.peer,
    );
    true
}

/// Format a SHA-256 digest the way the UI shows it (`SHA256:ab12…`), with the
/// hex doubled so `hex` stays out of the dependency list.
pub fn format_fingerprint(digest: &[u8]) -> String {
    let mut out = String::with_capacity(7 + digest.len() * 2);
    out.push_str("SHA256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// SHA-256 of `data`, for certificate/key fingerprints.
pub fn sha256_fingerprint(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format_fingerprint(&hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prompt_answer_must_echo_the_id_the_prompt_carried() {
        // The contract the RDP driver enforces, and the one the UI glue got
        // wrong: the prompt id starts at 1, the handler answered with a
        // hard-coded 0, and so every "trust and remember" both refused the
        // connection and left the certificate unwritten.
        let id = open_decision("rdp", "10.0.0.9", 3389, "SHA256:bb", None, None);
        assert_ne!(id, 0, "0 must stay reserved for 'no prompt'");
        // `accepted = false` keeps the test out of the real trust store: the
        // point here is the id, not persistence.
        assert!(
            !resolve_decision(0, false, false),
            "a placeholder id must never resolve a prompt"
        );
        assert!(
            resolve_decision(id, false, false),
            "the id from open_decision is the one that resolves"
        );
        assert!(
            !resolve_decision(id, false, false),
            "an id resolves exactly once"
        );
    }

    #[test]
    fn an_unknown_host_asks_once_then_stops_asking() {
        let mut store = TrustStore::in_memory();
        assert_eq!(
            store.evaluate("rdp", "10.0.0.5", 3389, "SHA256:aa", "prompt"),
            TrustOutcome::PromptUnknown
        );
        store.remember(
            "rdp",
            "10.0.0.5",
            3389,
            TrustedPeer {
                fingerprint: "SHA256:aa".into(),
                subject: None,
                issuer: None,
                valid_from: 0,
                valid_to: 0,
            },
        );
        assert_eq!(
            store.evaluate("rdp", "10.0.0.5", 3389, "SHA256:aa", "prompt"),
            TrustOutcome::Trusted
        );
        // Reconnecting repeatedly must not re-prompt.
        assert_eq!(
            store.evaluate("rdp", "10.0.0.5", 3389, "SHA256:aa", "prompt"),
            TrustOutcome::Trusted
        );
    }

    #[test]
    fn a_changed_fingerprint_asks_again_in_prompt_mode() {
        let mut store = TrustStore::in_memory();
        store.remember(
            "rdp",
            "h",
            3389,
            TrustedPeer {
                fingerprint: "SHA256:old".into(),
                subject: None,
                issuer: None,
                valid_from: 0,
                valid_to: 0,
            },
        );
        assert_eq!(
            store.evaluate("rdp", "h", 3389, "SHA256:new", "prompt"),
            TrustOutcome::PromptChanged
        );
        // Accepting the new one replaces the old, so a flip back prompts again.
        store.remember(
            "rdp",
            "h",
            3389,
            TrustedPeer {
                fingerprint: "SHA256:new".into(),
                subject: None,
                issuer: None,
                valid_from: 0,
                valid_to: 0,
            },
        );
        assert_eq!(
            store.evaluate("rdp", "h", 3389, "SHA256:new", "prompt"),
            TrustOutcome::Trusted
        );
        assert_eq!(
            store.evaluate("rdp", "h", 3389, "SHA256:old", "prompt"),
            TrustOutcome::PromptChanged
        );
    }

    #[test]
    fn strict_mode_refuses_anything_it_has_not_remembered() {
        let store = TrustStore::in_memory();
        assert_eq!(
            store.evaluate("rdp", "h", 3389, "SHA256:x", "strict"),
            TrustOutcome::Reject
        );
    }

    #[test]
    fn the_protocol_is_part_of_the_identity() {
        let mut store = TrustStore::in_memory();
        store.remember(
            "rdp",
            "h",
            5900,
            TrustedPeer {
                fingerprint: "SHA256:a".into(),
                subject: None,
                issuer: None,
                valid_from: 0,
                valid_to: 0,
            },
        );
        // Same host:port, different protocol -> still unknown.
        assert_eq!(
            store.evaluate("vnc", "h", 5900, "SHA256:a", "prompt"),
            TrustOutcome::PromptUnknown
        );
        assert_eq!(
            store.evaluate("rdp", "h", 5900, "SHA256:a", "prompt"),
            TrustOutcome::Trusted
        );
    }

    #[test]
    fn fingerprints_are_compared_case_insensitively() {
        let mut store = TrustStore::in_memory();
        store.remember(
            "vnc",
            "h",
            5900,
            TrustedPeer {
                fingerprint: "SHA256:AABB".into(),
                subject: None,
                issuer: None,
                valid_from: 0,
                valid_to: 0,
            },
        );
        assert!(store.is_trusted("vnc", "h", 5900, "sha256:aabb"));
    }

    #[test]
    fn a_corrupt_file_loads_as_nothing_trusted_rather_than_failing() {
        // `crate::test_tmp_dir`: `%TEMP%` 在受限环境可读不可写，直接用它会让
        // 这条测试红在建目录上，看起来像 TrustStore::load 坏了。
        let dir = crate::test_tmp_dir("mterm-trust-test");
        let path = dir.join("broken.json");
        std::fs::write(&path, "{ not json").unwrap();
        let store = TrustStore::load(&path);
        assert_eq!(
            store.evaluate("rdp", "h", 3389, "SHA256:x", "prompt"),
            TrustOutcome::PromptUnknown
        );
        let _ = std::fs::remove_file(&path);
    }
}
