//! The Windows Credential Manager, as the built-in RDP client sees it
//! (#credential-store 2026-10-13).
//!
//! The system client has always reached a password-protected host "without a
//! password", and the reason was never the host: with a blank password field
//! MTerm starts `mstsc /v:host` (see `crate::rdp`), and `mstsc` looks up
//! `TERMSRV/<host>` in the Credential Manager on its own. That entry — what a
//! "remember my credentials" tick, or a `cmdkey /generic:TERMSRV/<host>` line,
//! leaves behind — is where the password actually lives. It is why a host can
//! look like it has none while `mstsc` never prompts.
//!
//! The built-in client only ever saw the session's password field, so a blank
//! field really was a blank password: no NLA, and therefore a stop at the
//! server's own logon screen. This module closes that gap — a blank field asks
//! for the same entry `mstsc` would have used.
//!
//! Only `CRED_TYPE_GENERIC` entries ("普通" in the Windows UI) can be read this
//! way. An entry of type `CRED_TYPE_DOMAIN_PASSWORD` keeps its blob encrypted
//! for the authentication stack, so a host whose entry is of *that* type keeps
//! the old blank-password behaviour.
//!
//! Nothing here is fatal: a missing, unreadable or empty entry returns `None`
//! and the caller falls back to what it did before.

// `targets` and `decode_blob` have no non-test caller off Windows (there is no
// store to read), but the tests below still exercise them everywhere, so
// silence the dead-code lint rather than hiding them behind `cfg`.
#![cfg_attr(not(windows), allow(dead_code))]

/// Targets to try for `host`, in the order `mstsc` itself would have written
/// them: the address as typed, then `host:port` when the port is not the RDP
/// default. `mstsc` stores the entry under whatever the user typed, so both
/// spellings have to be tried — the session knows the port, the store does not.
fn targets(host: &str, port: u16) -> Vec<String> {
    let mut out = vec![format!("TERMSRV/{host}")];
    if port != crate::rdp::RDP_DEFAULT_PORT {
        out.push(format!("TERMSRV/{host}:{port}"));
    }
    out
}

/// Decode a generic credential's blob into a password.
///
/// The blob is an opaque byte string — whatever the writer handed to
/// `CredWriteW` — and Windows' own writers (`mstsc`, `cmdkey`) store the
/// password as **wide** text while other tools store raw bytes. The shape
/// decides which reading applies instead of a coin flip: an even-length blob
/// whose high bytes are all zero can only be UTF-16LE, and one containing a NUL
/// byte read as raw text can only be UTF-16LE read the wrong way. Everything
/// else is read as UTF-8. A trailing NUL is a terminator, not part of the
/// password.
fn decode_blob(blob: &[u8]) -> Option<String> {
    if blob.is_empty() {
        return None;
    }
    let text = if is_wide(blob) {
        let units: Vec<u16> = blob
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(blob).into_owned()
    };
    let text = text.trim_end_matches('\0');
    (!text.is_empty()).then(|| text.to_string())
}

/// Whether `blob` holds UTF-16LE text rather than raw bytes.
fn is_wide(blob: &[u8]) -> bool {
    blob.len() % 2 == 0 && (blob.chunks_exact(2).all(|pair| pair[1] == 0) || blob.contains(&0))
}

#[cfg(windows)]
mod store {
    use super::{decode_blob, targets};
    use windows::core::PCWSTR;
    use windows::Win32::Security::Credentials::{
        CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
    };

    /// The password Windows has stored for this host, if one is readable.
    pub(crate) fn stored_password(host: &str, port: u16) -> Option<String> {
        targets(host, port)
            .into_iter()
            .find_map(|target| read_raw(&target))
            .as_deref()
            .and_then(decode_blob)
    }

    /// One `CredReadW` lookup, before decoding.
    ///
    /// Every failure — no such entry, or an entry this process may not read —
    /// means the same thing to the caller ("use what the session said"), so
    /// errors are collapsed into `None` rather than reported.
    pub(crate) fn read_raw(target: &str) -> Option<Vec<u8>> {
        let wide: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
        let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: `wide` is NUL-terminated and alive for the call. On success
        // `CredReadW` hands over an allocation we own, `CredFree` releases it
        // exactly once, and the blob is copied out before that happens.
        unsafe {
            CredReadW(PCWSTR(wide.as_ptr()), CRED_TYPE_GENERIC, 0, &mut credential).ok()?;
            let credential = &*credential;
            let blob = std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
            .to_vec();
            CredFree((credential as *const CREDENTIALW).cast());
            Some(blob)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::rdp::embedded::credentials::is_wide;
        use windows::core::PWSTR;
        use windows::Win32::Security::Credentials::{
            CredDeleteW, CredWriteW, CRED_PERSIST_LOCAL_MACHINE,
        };

        /// RFC 5737 TEST-NET-1: an address that can never belong to a real host,
        /// so an entry left behind by a crashed run is obviously scratch.
        const SCRATCH_HOST: &str = "192.0.2.9";

        /// Removes the scratch entry however the test ends — an assertion
        /// failure must not leave anything in the user's store.
        struct Scratch(String);

        impl Drop for Scratch {
            fn drop(&mut self) {
                let wide: Vec<u16> = self.0.encode_utf16().chain(std::iter::once(0)).collect();
                // SAFETY: NUL-terminated wide string, alive for the call.
                unsafe {
                    let _ = CredDeleteW(PCWSTR(wide.as_ptr()), CRED_TYPE_GENERIC, 0);
                }
            }
        }

        #[test]
        fn a_password_written_the_way_windows_writes_it_is_read_back() {
            let target = targets(SCRATCH_HOST, 3389).remove(0);
            let _scratch = Scratch(target.clone());

            let target_wide: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
            let user_wide: Vec<u16> = "mterm-test"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            // Wide, exactly like `mstsc` / `cmdkey`.
            let blob: Vec<u8> = "s3cret"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            let credential = CREDENTIALW {
                Type: CRED_TYPE_GENERIC,
                TargetName: PWSTR(target_wide.as_ptr() as *mut u16),
                CredentialBlobSize: blob.len() as u32,
                CredentialBlob: blob.as_ptr() as *mut u8,
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                UserName: PWSTR(user_wide.as_ptr() as *mut u16),
                ..Default::default()
            };
            // SAFETY: every pointer in `credential` points at a live local.
            unsafe { CredWriteW(&credential, 0) }.expect("writing the scratch credential");

            assert_eq!(
                stored_password(SCRATCH_HOST, 3389),
                Some("s3cret".to_string())
            );
        }

        /// A host with nothing stored is the common case (most sessions keep
        /// their password in the field), and it must read as "no password"
        /// rather than as an error.
        #[test]
        fn a_host_without_an_entry_has_no_password() {
            assert_eq!(stored_password("192.0.2.10", 3389), None);
        }

        /// Reads a real entry off this machine and prints **only its size and
        /// shape** — never the password. Ignored by default:
        ///
        /// ```text
        /// MTERM_CRED_PROBE=TERMSRV/192.168.0.69 cargo test --bin MTerm -- \
        ///     --ignored --nocapture a_real_entry_can_be_read
        /// ```
        #[test]
        #[ignore]
        fn a_real_entry_can_be_read() {
            let Some(target) = std::env::var("MTERM_CRED_PROBE").ok() else {
                eprintln!("MTERM_CRED_PROBE is not set (e.g. TERMSRV/192.168.0.69)");
                return;
            };
            let Some(blob) = read_raw(&target) else {
                println!("{target}: no generic entry (or it is not readable)");
                return;
            };
            println!(
                "{target}: {} bytes, high bytes all zero = {}, contains a NUL = {}, \
                 reads as wide = {}, all printable ASCII bytes = {}",
                blob.len(),
                blob.chunks_exact(2).all(|pair| pair[1] == 0),
                blob.contains(&0),
                is_wide(&blob),
                blob.iter().all(|b| b.is_ascii_graphic() || *b == b' '),
            );
        }
    }
}

#[cfg(windows)]
pub(crate) use store::stored_password;

/// Off Windows there is no such store; the caller keeps its blank-password
/// behaviour.
#[cfg(not(windows))]
pub(crate) fn stored_password(_host: &str, _port: u16) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::{decode_blob, targets};

    /// The entry is stored under whatever the user typed, so both spellings the
    /// session can imply have to be tried — and the plain address comes first,
    /// because that is what a default-port session would have used.
    #[test]
    fn target_names_cover_what_mstsc_could_have_written() {
        assert_eq!(
            targets("192.168.0.69", 3389),
            vec!["TERMSRV/192.168.0.69".to_string()]
        );
        assert_eq!(
            targets("192.168.0.69", 13389),
            vec![
                "TERMSRV/192.168.0.69".to_string(),
                "TERMSRV/192.168.0.69:13389".to_string()
            ]
        );
    }

    #[test]
    fn a_wide_blob_decodes_to_its_password() {
        let wide: Vec<u8> = "hunter2".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode_blob(&wide), Some("hunter2".to_string()));
    }

    #[test]
    fn a_terminator_is_not_part_of_the_password() {
        let mut wide: Vec<u8> = "hunter2".encode_utf16().flat_map(u16::to_le_bytes).collect();
        wide.extend_from_slice(&[0, 0]);
        assert_eq!(decode_blob(&wide), Some("hunter2".to_string()));
    }

    /// An odd length cannot be wide text at all, so it is bytes.
    #[test]
    fn an_odd_length_blob_is_read_as_bytes() {
        assert_eq!(decode_blob(b"abc"), Some("abc".to_string()));
    }

    /// The one shape both readings fit: even length, no NUL anywhere (a
    /// non-ASCII password stored as bytes). Windows' own writers always leave
    /// evidence behind — a wide ASCII password has a zero in every high byte —
    /// so the byte reading wins here, and this test says so out loud.
    #[test]
    fn an_ambiguous_blob_is_read_as_bytes_rather_than_invented_text() {
        assert_eq!(decode_blob(b"\xc3\xa9\xc3\xa9"), Some("éé".to_string()));
    }

    /// What a blank field must mean when the store has nothing: no password,
    /// not an empty one that would still count as "set".
    #[test]
    fn an_empty_blob_is_not_a_password() {
        assert_eq!(decode_blob(&[]), None);
        assert_eq!(decode_blob(&[0, 0]), None);
    }
}
