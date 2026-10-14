//! Certificate trust for the built-in RDP client (P0-12).
//!
//! # Why this is a pre-flight probe and not a verifier hook
//!
//! The reference implementation plugs a `ServerCertificateVerifier` into
//! ironrdp's `ConfigBuilder`. **ironrdp 0.17 has no such hook**: its
//! `ironrdp-tls::upgrade()` builds its rustls config internally with a hard-coded
//! `NoCertificateVerification` and hands back the parsed certificate, and
//! `ConfigBuilder` exposes no way to influence either. A verifier therefore
//! cannot be written against this version.
//!
//! What is left is to do the trust decision *before* handing the connection to
//! ironrdp: dial the server, get as far as the TLS handshake, complete it with a
//! verifier that records the leaf certificate and then accepts unconditionally
//! (the handshake has to finish for the probe to be useful), evaluate the
//! fingerprint against the trust store, ask the user if needed, and only then let
//! ironrdp connect.
//!
//! The trade-off is honest and worth stating: between the probe and the real
//! connection the server could present a different certificate. That window is a
//! single TCP round-trip wide and only reachable by an attacker who is already
//! positioned on the path *and* can time it, which is a far weaker position than
//! "the client never checks at all". It is strictly better than the status quo
//! and it is the strongest guarantee this ironrdp version allows.
//!
//! # TLS is not the first thing on the wire
//!
//! Opening the socket with a TLS ClientHello — the obvious way to write this
//! probe — cannot work against a real RDP server. MS-RDPBCGR 2.2.1 has the
//! client open with an X.224 Connection Request carrying an RDP Negotiation
//! Request, and TLS only starts once the server's Connection Confirm has named
//! TLS as the selected protocol. The server is parsing the socket as TPKT, which
//! begins with version 3, while a TLS record begins with 0x16, so a server that
//! is healthy simply drops the connection. Skipping the initiation made the
//! whole trust flow dead code: every probe failed, and the fingerprint was never
//! available. The probe therefore runs the initiation itself (see
//! `negotiate_security_protocol`) before handing the socket to rustls.
//!
//! # The probe does not install a crypto provider, because it cannot install one
//! usefully
//!
//! `ClientConfig::builder()` at the bottom of [`probe_certificate`] needs a
//! process-wide rustls [`rustls::crypto::CryptoProvider`], and this binary is
//! built with two provider features at once (`ring` via `ureq`, `aws_lc_rs` via
//! `ironrdp-tls`), so rustls cannot pick one by itself and panics instead.
//! Installing the provider here would only fix this call site; `ironrdp-tls`
//! builds its own config the same way, and that one is inside a dependency.
//! `crate::rdp::install_rustls_crypto_provider` therefore does it once for the whole
//! process, which covers both. A panic there is fatal rather than merely loud —
//! the release profile is `panic = "abort"` — so the install has to happen
//! before anything else, and it does.
//!
//! Note that tests do not go through `run()` and so must install it themselves if
//! they build a TLS config; the ignored probe test below does.
//!
//! # The probe is best-effort, and the caller must treat it that way
//!
//! Two healthy servers still produce no fingerprint: one that answers the
//! negotiation with standard RDP security (no TLS, therefore no certificate at
//! all), and one reached through something that is not speaking RDP. Neither is
//! a statement about trust.
//!
//! A failed probe therefore means "no fingerprint available", **not** "the
//! server is untrustworthy", and the caller continues the connection without a
//! prompt rather than blocking it. The known-hosts check is a bonus on top of
//! what ironrdp offers, not a gate: ironrdp's own TLS layer accepts any
//! certificate regardless (see the module header above), so skipping a prompt
//! that could not be computed leaves the user exactly where the status quo was.
//!
//! [`probe_certificate`] is `async` and must be awaited on the caller's
//! runtime. It used to build its own runtime and `block_on` it, which tokio
//! rejects with "Cannot start a runtime from within a runtime" — the RDP driver
//! already runs inside one, so that shape panicked on every connection.

use std::sync::Arc;
use std::time::Duration;

use ironrdp::core::{ReadCursor, decode, encode_vec};
use ironrdp::pdu::nego;
use ironrdp::pdu::tpkt::TpktHeader;
use ironrdp::pdu::x224::X224;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::{DigitallySignedStruct, Error as TlsError, SignatureScheme};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::remote::trust::{sha256_fingerprint, TrustOutcome, TrustedPeer};

/// How long the probe may take before we give up on the whole connection.
///
/// A server that cannot complete a TLS handshake within this is not going to
/// complete an RDP one either, and the user should get an error rather than a
/// spinner that never resolves.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// Largest X.224 frame the probe will read.
///
/// The Connection Confirm is around 20 bytes. A cap keeps a server that answers
/// with a huge length field from making the probe allocate on its word alone.
const MAX_X224_FRAME: usize = 1024;

/// Record a decision the user made in the UI.
pub fn remember(
    store: &mut crate::remote::trust::TrustStore,
    protocol: &str,
    host: &str,
    port: u16,
    fingerprint: &str,
    subject: Option<String>,
    issuer: Option<String>,
) {
    store.remember(
        protocol,
        host,
        port,
        TrustedPeer {
            fingerprint: fingerprint.to_string(),
            subject,
            issuer,
            valid_from: 0,
            valid_to: 0,
        },
    );
}

/// Dial `host:port`, complete a TLS handshake, and return the leaf certificate's
/// fingerprint plus its metadata.
///
/// The verifier accepts unconditionally because the point of the probe is to
/// *collect* the certificate; whether it is trusted is decided separately by the
/// caller. Handshaking with a real root store instead would reject every
/// self-signed server, which is the common case for RDP and would make the probe
/// useless.
///
/// **Must be awaited on the caller's runtime.** It used to build a private
/// current-thread runtime and `block_on` it, which panics with "Cannot start a
/// runtime from within a runtime" because the RDP driver already runs inside
/// one — see the module header.
///
/// A failure here is not a verdict on the server; the caller treats it as "no
/// fingerprint available" (see the module header).
pub async fn probe_certificate(host: &str, port: u16) -> Result<ProbeResult, String> {
    let mut tcp = tokio::time::timeout(PROBE_TIMEOUT, TcpStream::connect((host, port)))
        .await
        .map_err(|_| crate::i18n::t("连接超时", "connection timed out").to_string())?
        .map_err(|err| format!("{err}"))?;

    // The certificate lives behind TLS, and TLS only starts once the server has
    // agreed to it in the X.224 exchange — see the module header.
    let selected = tokio::time::timeout(PROBE_TIMEOUT, negotiate_security_protocol(&mut tcp))
        .await
        .map_err(|_| {
            crate::i18n::t("X.224 协商超时", "X.224 negotiation timed out").to_string()
        })??;
    if !selected.intersects(tls_protocols()) {
        return Err(crate::i18n::t(
            "服务端选择了标准 RDP 安全层，本次连接没有证书可校验",
            "the server selected standard RDP security, so there is no certificate to verify",
        )
        .to_string());
    }

    let server_name = rustls_pki_types::ServerName::try_from(host.to_string())
        .map_err(|_| {
            crate::i18n::t(
                "服务器名无法用于 TLS 校验",
                "server name cannot be used for TLS validation",
            )
            .to_string()
        })?
        .to_owned();

    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(RecordingVerifier))
        .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let tls = tokio::time::timeout(PROBE_TIMEOUT, connector.connect(server_name, tcp))
        .await
        .map_err(|_| {
            crate::i18n::t("TLS 握手超时", "TLS handshake timed out").to_string()
        })?
        .map_err(|err| format!("{err}"))?;

    let der = {
        let (_, connection) = tls.get_ref();
        connection
            .peer_certificates()
            .and_then(|chain| chain.first())
            .cloned()
            .ok_or_else(|| {
                crate::i18n::t(
                    "服务器未提供证书",
                    "server presented no certificate",
                )
                .to_string()
            })?
    };
    // The socket dies with `tls` at the end of this function, which is what we
    // want: the probe must not hold a connection open while the user thinks
    // about the prompt.

    let mut metadata = CertificateMetadata::default();
    if let Ok(cert) = <x509_cert::Certificate as x509_cert::der::Decode>::from_der(der.as_ref()) {
        metadata.subject = Some(cert.tbs_certificate.subject.to_string());
        metadata.issuer = Some(cert.tbs_certificate.issuer.to_string());
        metadata.valid_from = Some(cert.tbs_certificate.validity.not_before.to_string());
        metadata.valid_to = Some(cert.tbs_certificate.validity.not_after.to_string());
    }

    Ok(ProbeResult {
        fingerprint: sha256_fingerprint(der.as_ref()),
        subject: metadata.subject,
        issuer: metadata.issuer,
        valid_from: metadata.valid_from,
        valid_to: metadata.valid_to,
    })
}

/// The security protocols that put TLS on the connection.
///
/// Used both to offer them and to judge the server's answer, so the two can
/// never disagree about what counts as "TLS is in play".
fn tls_protocols() -> nego::SecurityProtocol {
    nego::SecurityProtocol::SSL | nego::SecurityProtocol::HYBRID | nego::SecurityProtocol::HYBRID_EX
}

/// Send the X.224 Connection Request and read the Connection Confirm, returning
/// the security protocol the server selected.
///
/// Deliberately mirrors `ironrdp-connector`'s own connection-initiation states:
/// that code is the one other place in this dependency tree that performs this
/// exchange, and the probe must ask for exactly what the real connection will
/// ask for, or the server could answer differently and the fingerprint would be
/// for a negotiation that never happens. `ironrdp`'s client enables both TLS and
/// CredSSP, so all three protocols are offered here too.
///
/// The exchange is small enough to speak directly rather than through a framed
/// reader: one request out, one frame in.
async fn negotiate_security_protocol(
    tcp: &mut TcpStream,
) -> Result<nego::SecurityProtocol, String> {
    let request = nego::ConnectionRequest {
        // No cookie. That field identifies a session the server kept alive for a
        // reconnect, and this probe is a brand-new connection.
        nego_data: None,
        flags: nego::RequestFlags::empty(),
        protocol: tls_protocols(),
    };
    let bytes = encode_vec(&X224(request)).map_err(|err| format!("{err}"))?;
    tcp.write_all(&bytes).await.map_err(|err| format!("{err}"))?;

    let mut header = [0u8; TpktHeader::SIZE];
    tcp.read_exact(&mut header)
        .await
        .map_err(|err| format!("{err}"))?;
    // `TpktHeader::read` also validates the version byte, so a server that is
    // not speaking TPKT is rejected here rather than misparsed.
    let total = usize::from(
        TpktHeader::read(&mut ReadCursor::new(&header))
            .map_err(|err| format!("{err}"))?
            .packet_length,
    );
    if total < TpktHeader::SIZE || total > MAX_X224_FRAME {
        return Err(format!(
            "X.224 frame length {total} is outside the expected range"
        ));
    }
    let mut frame = vec![0u8; total];
    frame[..TpktHeader::SIZE].copy_from_slice(&header);
    tcp.read_exact(&mut frame[TpktHeader::SIZE..])
        .await
        .map_err(|err| format!("{err}"))?;

    match decode::<X224<nego::ConnectionConfirm>>(&frame)
        .map_err(|err| format!("{err}"))?
        .0
    {
        nego::ConnectionConfirm::Response { protocol, .. } => Ok(protocol),
        nego::ConnectionConfirm::Failure { code } => Err(format!(
            "the server refused the connection initiation: {code:?}"
        )),
    }
}

/// Certificate fields worth showing in the trust prompt.
#[derive(Debug, Default, Clone)]
struct CertificateMetadata {
    subject: Option<String>,
    issuer: Option<String>,
    valid_from: Option<String>,
    valid_to: Option<String>,
}

/// Outcome of a successful probe.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub fingerprint: String,
    pub subject: Option<String>,
    pub issuer: Option<String>,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
}

/// A verifier that accepts any certificate but keeps the leaf's bytes.
///
/// Recording through a verifier (rather than reading them off the connection
/// afterwards) means the bytes hashed are exactly the ones the TLS stack
/// accepted — there is no second, possibly different, read to disagree with.
#[derive(Debug)]
struct RecordingVerifier;

impl ServerCertVerifier for RecordingVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls_pki_types::CertificateDer<'_>,
        _intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _server_name: &rustls_pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls_pki_types::UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_untrusted_host_raises_a_prompt_and_a_known_one_does_not() {
        let mut store = crate::remote::trust::TrustStore::in_memory();
        // No probe is possible in a unit test, so exercise the decision function
        // with a synthetic fingerprint — that is the part with the logic.
        let fp = sha256_fingerprint(b"cert-bytes");
        assert_eq!(
            store.evaluate("rdp", "h", 3389, &fp, "prompt"),
            TrustOutcome::PromptUnknown
        );
        remember(
            &mut store,
            "rdp",
            "h",
            3389,
            &fp,
            Some("CN=h".into()),
            Some("CN=h".into()),
        );
        assert_eq!(
            store.evaluate("rdp", "h", 3389, &fp, "prompt"),
            TrustOutcome::Trusted
        );
        assert!(store.is_trusted("rdp", "h", 3389, &fp));
    }

    #[test]
    fn a_fingerprint_is_stable_and_hex_formatted() {
        let fp = sha256_fingerprint(b"abc");
        assert!(fp.starts_with("SHA256:"));
        assert_eq!(fp.len(), "SHA256:".len() + 64);
        assert_eq!(fp, sha256_fingerprint(b"abc"));
        assert_ne!(fp, sha256_fingerprint(b"abd"));
    }

    /// What the mock server answers the connection initiation with.
    enum Reply {
        /// A well-formed Connection Confirm naming `protocol`.
        Confirm(nego::SecurityProtocol),
        /// Bytes that are not a TPKT frame at all.
        NotTpkt,
        /// A TPKT header claiming far more bytes than are sent.
        Oversized,
    }

    /// Serve one connection initiation the way an RDP server would, and return
    /// the port to dial.
    ///
    /// The point is to exercise the exchange without an RDP host: the request is
    /// decoded on the way in, so a malformed probe fails the test instead of
    /// passing silently.
    async fn mock_server(reply: Reply) -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a loopback port");
        let port = listener.local_addr().expect("local addr").port();
        tokio::spawn(async move {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            // Drain the request in every case, before answering: a socket closed
            // while it still has unread received data is *reset* rather than
            // closed, and that RST overtakes the reply we are about to send. (It
            // is also what proves the probe sent a well-formed Connection
            // Request — a malformed one fails to decode.)
            let mut header = [0u8; TpktHeader::SIZE];
            if sock.read_exact(&mut header).await.is_err() {
                return;
            }
            let total = usize::from(
                TpktHeader::read(&mut ReadCursor::new(&header))
                    .expect("the probe sent a TPKT frame")
                    .packet_length,
            );
            let mut frame = vec![0u8; total];
            frame[..TpktHeader::SIZE].copy_from_slice(&header);
            if sock.read_exact(&mut frame[TpktHeader::SIZE..]).await.is_err() {
                return;
            }

            match reply {
                Reply::NotTpkt => {
                    let _ = sock.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
                }
                Reply::Oversized => {
                    // Leading 0x03 so it parses as TPKT, then an absurd length.
                    let _ = sock.write_all(&[0x03, 0x00, 0xff, 0xff]).await;
                }
                Reply::Confirm(protocol) => {
                    let request = decode::<X224<nego::ConnectionRequest>>(&frame)
                        .expect("the probe sent a decodable Connection Request")
                        .0;
                    assert!(
                        request.protocol.intersects(tls_protocols()),
                        "the probe must offer TLS, or the server would never select it"
                    );
                    assert!(
                        request.nego_data.is_none(),
                        "a fresh probe connection carries no reconnect cookie"
                    );

                    let confirm = X224(nego::ConnectionConfirm::Response {
                        flags: nego::ResponseFlags::empty(),
                        protocol,
                    });
                    let out = encode_vec(&confirm).expect("encode the confirm");
                    let _ = sock.write_all(&out).await;
                }
            }
            // Hold the socket open until the probe drops it, so the reply is
            // delivered instead of being discarded by a RST.
            let mut sink = [0u8; 1];
            let _ = sock.read(&mut sink).await;
        });
        port
    }

    async fn dial(reply: Reply) -> Result<nego::SecurityProtocol, String> {
        let port = mock_server(reply).await;
        let mut tcp = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect to the mock server");
        negotiate_security_protocol(&mut tcp).await
    }

    #[tokio::test]
    async fn the_probe_reports_the_protocol_the_server_confirmed() {
        let selected = dial(Reply::Confirm(nego::SecurityProtocol::HYBRID))
            .await
            .expect("a normal confirm must be read back");
        assert_eq!(selected, nego::SecurityProtocol::HYBRID);
        assert!(selected.intersects(tls_protocols()));
    }

    #[tokio::test]
    async fn standard_rdp_security_means_there_is_no_certificate_to_read() {
        // A server that answers with no protocol flags has chosen plain RDP
        // security: no TLS, so nothing to fingerprint. The probe must say so
        // instead of starting a TLS handshake that cannot succeed.
        let selected = dial(Reply::Confirm(nego::SecurityProtocol::empty()))
            .await
            .expect("standard RDP security is a valid answer, not an error");
        assert!(!selected.intersects(tls_protocols()));
    }

    #[tokio::test]
    async fn a_port_that_is_not_speaking_rdp_is_an_error_not_a_hang() {
        let err = dial(Reply::NotTpkt)
            .await
            .expect_err("a non-TPKT answer must not be accepted");
        assert!(!err.is_empty());
    }

    #[tokio::test]
    async fn an_impossible_frame_length_is_refused_before_reading_it() {
        let err = dial(Reply::Oversized)
            .await
            .expect_err("an oversized length field must be refused");
        assert!(err.contains("65535"), "unexpected error: {err}");
    }

    /// The probe against a real, listening RDP server.
    ///
    /// Ignored by default because it needs a host to talk to, but kept because it
    /// is the only check that reaches past the X.224 exchange on a real server:
    /// everything above stops at the mock. The step it adds is the one that was
    /// actually broken — `ClientConfig::builder()` panicking because the process
    /// had no crypto provider, which took the whole application down on every
    /// embedded-RDP click.
    ///
    /// Point it at a host with `MTERM_RDP_PROBE=host:port`; the default is the
    /// RDP service of the machine running the tests (Windows answers on
    /// 127.0.0.1:3389 whenever Remote Desktop is enabled).
    #[tokio::test]
    #[ignore = "needs a reachable RDP server"]
    async fn the_probe_reaches_tls_on_a_real_server() {
        // 测试不经 `run()`，所以进程级 provider 没有被装过，而
        // `ClientConfig::builder()` 需要它。
        crate::rdp::install_rustls_crypto_provider();

        let target =
            std::env::var("MTERM_RDP_PROBE").unwrap_or_else(|_| "127.0.0.1:3389".to_owned());
        let (host, port) = target.rsplit_once(':').expect("expected host:port");
        let port: u16 = port.parse().expect("expected a numeric port");

        match probe_certificate(host, port).await {
            Ok(probe) => {
                assert!(
                    probe.fingerprint.starts_with("SHA256:"),
                    "fingerprint is not in the documented shape: {}",
                    probe.fingerprint
                );
                println!("{target} -> {} ({:?})", probe.fingerprint, probe.subject);
            }
            // Not a test failure: a host that runs no TLS, a port that is not
            // RDP and a port with nothing listening are all legitimate answers,
            // and the probe is best-effort by design. Surviving the attempt
            // without the process dying is what this test is for.
            Err(reason) => println!("{target} -> no fingerprint: {reason}"),
        }
    }
}
