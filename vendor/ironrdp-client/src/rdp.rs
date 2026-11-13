use core::net::SocketAddr;
use core::num::NonZeroU16;
use core::time::Duration;
use std::sync::Arc;

use ironrdp_bulk::BulkCompressor;
use ironrdp_connector::connection_activation::ConnectionActivationState;
use ironrdp_connector::{ConnectionResult, ConnectorResult};
use ironrdp_core::WriteBuf;
use ironrdp_displaycontrol::client::DisplayControlClient;
use ironrdp_displaycontrol::pdu::MonitorLayoutEntry;
#[cfg(all(windows, feature = "dvc-com-plugin"))]
use ironrdp_dvc::DvcProcessor as _;
use ironrdp_echo::client::EchoClient;
use ironrdp_graphics::image_processing::PixelFormat;
use ironrdp_graphics::pointer::DecodedPointer;
use ironrdp_pdu::input::MousePdu;
use ironrdp_pdu::input::fast_path::FastPathInputEvent;
use ironrdp_pdu::input::mouse::PointerFlags;
#[cfg(any(feature = "dvc-pipe-proxy", all(windows, feature = "dvc-com-plugin")))]
use ironrdp_pdu::pdu_other_err;
use ironrdp_session::image::DecodedImage;
use ironrdp_session::{ActiveStage, ActiveStageBuilder, ActiveStageOutput, GracefulDisconnectReason, SessionResult, fast_path};
use ironrdp_svc::SvcMessage;
use ironrdp_tokio::reqwest::ReqwestNetworkClient;
use ironrdp_tokio::{FramedWrite, single_sequence_step_read, split_tokio_framed};
use smallvec::SmallVec;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
#[cfg(any(feature = "clipboard", all(windows, feature = "dvc-com-plugin")))]
use tracing::error;
use tracing::{debug, info, trace, warn};

#[cfg(feature = "clipboard")]
use crate::config::ClipboardType;
#[cfg(feature = "clipboard")]
use ironrdp_cliprdr::backend::{ClipboardMessage, CliprdrBackendFactory};
#[cfg(all(windows, feature = "dvc-com-plugin"))]
use ironrdp_dvc_com_plugin::load_dvc_plugin;
#[cfg(feature = "dvc-pipe-proxy")]
use ironrdp_dvc_pipe_proxy::DvcNamedPipeProxy;
#[cfg(feature = "sound")]
use ironrdp_rdpsnd_native::cpal;

use crate::config::{Config, RDCleanPathConfig, Transport};

// ── Public event types ────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum RdpOutputEvent {
    Image {
        buffer: Vec<u32>,
        width: NonZeroU16,
        height: NonZeroU16,
    },
    ConnectionFailure(ironrdp_connector::ConnectorError),
    PointerDefault,
    PointerHidden,
    PointerPosition {
        x: u16,
        y: u16,
    },
    PointerBitmap(Arc<DecodedPointer>),
    Terminated(SessionResult<GracefulDisconnectReason>),
}

#[derive(Debug)]
pub enum RdpInputEvent {
    Resize {
        width: u16,
        height: u16,
        scale_factor: u32,
        /// Physical display size in millimetres (width, height).
        physical_size: Option<(u32, u32)>,
    },
    FastPath(SmallVec<[FastPathInputEvent; 2]>),
    Close,
    #[cfg(feature = "clipboard")]
    Clipboard(ClipboardMessage),
    SendDvcMessages {
        channel_id: u32,
        messages: Vec<SvcMessage>,
    },
}

// ── RdpClient ─────────────────────────────────────────────────────────────────

pub struct RdpClient {
    config: Config,
    output_event_sender: mpsc::Sender<RdpOutputEvent>,
    input_event_sender: mpsc::UnboundedSender<RdpInputEvent>,
    input_event_receiver: mpsc::UnboundedReceiver<RdpInputEvent>,
    /// (MTerm patch, #rdp-clipboard 2026-11-13) A clipboard backend supplied by
    /// the host application.
    ///
    /// The library's own `WinClipboard` needs a window with a message pump on
    /// the thread that owns it, which an embedding application running the
    /// client on a worker thread cannot provide. A host-supplied factory
    /// takes over the local side of the CLIPRDR channel.
    #[cfg(feature = "clipboard")]
    cliprdr_backend_factory: Option<Box<dyn CliprdrBackendFactory + Send>>,
}

impl RdpClient {
    pub fn new(config: Config, output_event_sender: mpsc::Sender<RdpOutputEvent>) -> Self {
        let (input_event_sender, input_event_receiver) = mpsc::unbounded_channel();
        Self {
            config,
            output_event_sender,
            input_event_sender,
            input_event_receiver,
            #[cfg(feature = "clipboard")]
            cliprdr_backend_factory: None,
        }
    }

    /// (MTerm patch, #rdp-clipboard 2026-11-13) Supplies the local side of the
    /// clipboard channel.
    ///
    /// The backend is created by the host, which owns the local clipboard and
    /// the thread it lives on.
    #[cfg(feature = "clipboard")]
    #[must_use]
    pub fn with_cliprdr_backend_factory(
        mut self,
        factory: Box<dyn CliprdrBackendFactory + Send>,
    ) -> Self {
        self.cliprdr_backend_factory = Some(factory);
        self
    }


    /// Return a clone of the input-event sender for injecting keyboard, mouse, and clipboard
    /// events from the GUI thread.
    pub fn input_sender(&self) -> mpsc::UnboundedSender<RdpInputEvent> {
        self.input_event_sender.clone()
    }

    pub async fn run(mut self) {
        // ── Clipboard initialisation (compile-time gated) ─────────────────────
        //
        // On Windows the WinClipboard object must outlive the entire connection loop, so we
        // keep it alive via `_win_clipboard`.  On non-Windows a StubClipboard backend is used
        // and its ownership can be released immediately after the factory is extracted.
        #[cfg(all(windows, feature = "clipboard"))]
        #[expect(
            clippy::collection_is_never_read,
            reason = "binding owns the Windows clipboard so it stays alive for the connection's lifetime"
        )]
        let _win_clipboard;

        #[cfg(feature = "clipboard")]
        let cliprdr_factory: Option<Box<dyn CliprdrBackendFactory + Send>>;


        #[cfg(feature = "clipboard")]
        {
            // (MTerm patch, #rdp-clipboard 2026-11-13) A host-supplied backend
            // wins over the library's own: it is the only way for an
            // application that runs the client on a worker thread to own the
            // local clipboard.
            let host_factory = self.cliprdr_backend_factory.take();
            match (self.config.channels.clipboard, host_factory) {
                (ClipboardType::Enable, Some(factory)) => {
                    cliprdr_factory = Some(factory);
                    #[cfg(windows)]
                    {
                        _win_clipboard = None;
                    }
                }
                (ClipboardType::Disable, _) => {
                    cliprdr_factory = None;
                    #[cfg(windows)]
                    {
                        _win_clipboard = None;
                    }
                }
                (ClipboardType::Stub, _) => {
                    use ironrdp_cliprdr_native::StubClipboard;
                    let stub = StubClipboard::new();
                    cliprdr_factory = Some(stub.backend_factory());
                    #[cfg(windows)]
                    {
                        _win_clipboard = None;
                    }
                }
                (ClipboardType::Enable, None) => {
                    #[cfg(windows)]
                    {
                        use crate::clipboard::ClientClipboardMessageProxy;
                        use ironrdp_cliprdr_native::WinClipboard;
                        match WinClipboard::new(ClientClipboardMessageProxy::new(self.input_event_sender.clone())) {
                            Ok(win_cb) => {
                                cliprdr_factory = Some(win_cb.backend_factory());
                                _win_clipboard = Some(win_cb);
                            }
                            Err(e) => {
                                let _ = self
                                    .output_event_sender
                                    .send(RdpOutputEvent::ConnectionFailure(ironrdp_connector::custom_err!(
                                        "Windows clipboard initialization",
                                        e
                                    )))
                                    .await;
                                return;
                            }
                        }
                    }

                    #[cfg(not(windows))]
                    {
                        use ironrdp_cliprdr_native::StubClipboard;
                        let stub = StubClipboard::new();
                        cliprdr_factory = Some(stub.backend_factory());
                    }
                }
            }
        }

        // Resolve the per-connection cliprdr factory reference once.  `Option<&dyn …>` is `Copy`,
        // so it can be threaded into every connect attempt across reconnects.
        #[cfg(feature = "clipboard")]
        let cliprdr_factory: CliprdrFactoryRef<'_> = cliprdr_factory.as_deref();
        #[cfg(not(feature = "clipboard"))]
        let cliprdr_factory: CliprdrFactoryRef<'_> = core::marker::PhantomData;

        // ── Connection + session loop ─────────────────────────────────────────
        //
        // (#rdp-resize-reconnect 2026-11-13) A reconnect that follows a
        // desktop resize races the server's release of the previous session:
        // the old connection is only torn down once the transport notices, and
        // a new connection arriving in that window is refused ("another
        // session is logged in"). Such a reconnect is therefore retried a few
        // times before the failure is reported.
        let mut resize_reconnect = false;
        let mut resize_reconnect_attempts = 0u32;

        loop {
            let connect_result = match &self.config.transport {
                Transport::Direct => {
                    connect_direct(&self.config, &self.input_event_sender, cliprdr_factory).await
                }

                #[cfg(feature = "gateway")]
                Transport::Gateway(gw) => {
                    connect_gateway(&self.config, gw, &self.input_event_sender, cliprdr_factory).await
                }

                Transport::RDCleanPath(rdcp) => {
                    connect_rdcleanpath_transport(&self.config, rdcp, &self.input_event_sender, cliprdr_factory).await
                }
            };

            let (connection_result, framed) = match connect_result {
                Ok(result) => result,
                Err(e) => {
                    if resize_reconnect && resize_reconnect_attempts < RESIZE_RECONNECT_ATTEMPTS {
                        resize_reconnect_attempts += 1;
                        let delay = resize_reconnect_delay(resize_reconnect_attempts);
                        warn!(
                            error = %e,
                            attempt = resize_reconnect_attempts,
                            delay_ms = delay.as_millis(),
                            "The reconnect after a desktop resize was refused; retrying"
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    let _ = self
                        .output_event_sender
                        .send(RdpOutputEvent::ConnectionFailure(e))
                        .await;
                    break;
                }
            };

            match active_session(
                framed,
                connection_result,
                &self.output_event_sender,
                &mut self.input_event_receiver,
                self.config.fake_events_interval,
            )
            .await
            {
                Ok(RdpControlFlow::ReconnectWithNewSize { width, height }) => {
                    self.config.connector.desktop_size.width = width;
                    self.config.connector.desktop_size.height = height;
                    // The next connection attempt is the one that may be
                    // refused, so it is the one that gets retried.
                    resize_reconnect = true;
                    resize_reconnect_attempts = 0;
                }
                Ok(RdpControlFlow::TerminatedGracefully(reason)) => {
                    let _ = self
                        .output_event_sender
                        .send(RdpOutputEvent::Terminated(Ok(reason)))
                        .await;
                    break;
                }
                Err(e) => {
                    let _ = self.output_event_sender.send(RdpOutputEvent::Terminated(Err(e))).await;
                    break;
                }
            }
        }
    }
}

// ── Connector builder ─────────────────────────────────────────────────────────

/// Reference to the cliprdr backend factory threaded into the connect helpers.
///
/// Collapses to a zero-sized placeholder when the `clipboard` feature is disabled, so the
/// connect-helper signatures don't need `#[cfg]` on this parameter.
#[cfg(feature = "clipboard")]
type CliprdrFactoryRef<'a> = Option<&'a (dyn CliprdrBackendFactory + Send)>;
#[cfg(not(feature = "clipboard"))]
type CliprdrFactoryRef<'a> = core::marker::PhantomData<&'a ()>;


/// Build a fully wired [`ironrdp_connector::ClientConnector`] with all feature-gated channels attached.
///
/// This helper is used by all transport paths. The cliprdr backend is (re)built here, per
/// connection, from `cliprdr_factory`.
fn build_connector(
    config: &Config,
    client_addr: SocketAddr,
    input_sender: &mpsc::UnboundedSender<RdpInputEvent>,
    cliprdr_factory: CliprdrFactoryRef<'_>,
) -> ironrdp_connector::ClientConnector {
    // `input_sender` is only consumed by the optional DVC wirings below, and `cliprdr_factory`
    // only by the optional CLIPRDR attachment; discard them explicitly when those are compiled out.
    #[cfg(not(any(feature = "dvc-pipe-proxy", all(windows, feature = "dvc-com-plugin"))))]
    let _ = input_sender;
    #[cfg(not(feature = "clipboard"))]
    let _ = cliprdr_factory;

    let mut drdynvc = ironrdp_dvc::DrdynvcClient::new()
        .with_dynamic_channel(DisplayControlClient::new(|_| Ok(Vec::new())))
        .with_dynamic_channel(EchoClient::new());

    // Attach DVC pipe proxies.
    #[cfg(feature = "dvc-pipe-proxy")]
    for proxy in &config.dvc_pipe_proxies {
        let channel_name = proxy.channel_name.clone();
        let pipe_name = proxy.pipe_name.clone();
        trace!(%channel_name, %pipe_name, "Creating DVC pipe proxy");
        let sender = input_sender.clone();
        drdynvc = drdynvc.with_dynamic_channel(DvcNamedPipeProxy::new(
            &channel_name,
            &pipe_name,
            move |channel_id, messages| {
                sender
                    .send(RdpInputEvent::SendDvcMessages { channel_id, messages })
                    .map_err(|_| pdu_other_err!("send DVC messages to the event loop"))?;
                Ok(())
            },
        ));
    }

    // Load DVC COM plugins (Windows + dvc-com-plugin feature).
    #[cfg(all(windows, feature = "dvc-com-plugin"))]
    {
        for plugin_path in &config.dvc_plugins {
            info!(dll = %plugin_path.display(), "Loading DVC COM plugin");
            let sender_clone = input_sender.clone();
            match load_dvc_plugin(plugin_path, move || {
                let sender = sender_clone.clone();
                Box::new(move |channel_id, messages| {
                    sender
                        .send(RdpInputEvent::SendDvcMessages { channel_id, messages })
                        .map_err(|_| pdu_other_err!("send COM DVC messages to the event loop"))?;
                    Ok(())
                })
            }) {
                Ok(channels) => {
                    for channel in channels {
                        info!(channel_name = %channel.channel_name(), "Registered COM DVC channel");
                        drdynvc = drdynvc.with_dynamic_channel(channel);
                    }
                }
                Err(e) => {
                    error!(dll = %plugin_path.display(), error = %e, "Failed to load DVC COM plugin");
                }
            }
        }
    }

    // Attach user-defined DVC channels from the extension registry.
    for attach_dvc in &config.extensions.dvc_channels {
        attach_dvc(&mut drdynvc, &config.properties);
    }

    // Clone the connector config so we can apply runtime overrides before handing it to the
    // connector.  We want to set `enable_audio_playback` consistently with `channels.sound`.
    let mut connector_config = config.connector.clone();

    // If sound is disabled at runtime (or the feature is off) ensure the connector doesn't
    // advertise audio support, which would confuse the server.
    #[cfg(not(feature = "sound"))]
    {
        connector_config.enable_audio_playback = false;
    }
    #[cfg(feature = "sound")]
    if !config.channels.sound {
        connector_config.enable_audio_playback = false;
    }

    // Honor the runtime QOI/QOIZ codec toggles. Both codecs are compiled in and advertised by
    // default, but can be disabled at runtime; when disabled we drop them from the advertised
    // bitmap codec list so the server won't negotiate them.
    #[cfg(any(feature = "qoi", feature = "qoiz"))]
    if let Some(bitmap) = connector_config.bitmap.as_mut() {
        use ironrdp_pdu::rdp::capability_sets::CodecProperty;

        bitmap.codecs.0.retain(|codec| match codec.property {
            #[cfg(feature = "qoi")]
            CodecProperty::Qoi => config.channels.qoi,
            #[cfg(feature = "qoiz")]
            CodecProperty::QoiZ => config.channels.qoiz,
            _ => true,
        });
    }

    let mut connector =
        ironrdp_connector::ClientConnector::new(connector_config, client_addr).with_static_channel(drdynvc);

    // Attach RDPSND (audio).
    #[cfg(feature = "sound")]
    if config.channels.sound {
        connector = connector.with_static_channel(ironrdp_rdpsnd::client::Rdpsnd::new(Box::new(
            cpal::RdpsndBackend::new(),
        )));
    }

    // Attach RDPDR (device redirection).
    #[cfg(feature = "rdpdr")]
    if config.channels.rdpdr.enabled {
        #[cfg_attr(
            not(feature = "smartcard"),
            expect(
                unused_mut,
                reason = "rdpdr_channel is only reassigned when the smartcard feature is enabled"
            )
        )]
        let mut rdpdr_channel =
            ironrdp_rdpdr::Rdpdr::new(Box::new(ironrdp_rdpdr::NoopRdpdrBackend), "IronRDP".to_owned());
        #[cfg(feature = "smartcard")]
        if config.channels.rdpdr.smartcard {
            rdpdr_channel = rdpdr_channel.with_smartcard(0);
        }
        connector = connector.with_static_channel(rdpdr_channel);
    }

    // Attach CLIPRDR (clipboard redirection). The backend is built fresh per connection.
    #[cfg(feature = "clipboard")]
    if let Some(factory) = cliprdr_factory {
        let backend = factory.build_cliprdr_backend();
        connector.attach_static_channel(ironrdp_cliprdr::Cliprdr::new(backend));
    }

    // Attach user-defined static channels from the extension registry.
    for attach_sc in &config.extensions.static_channels {
        attach_sc(&mut connector, &config.properties);
    }

    connector
}

// ── Transport-specific connect helpers ────────────────────────────────────────

trait AsyncReadWrite: AsyncRead + AsyncWrite {}
impl<T> AsyncReadWrite for T where T: AsyncRead + AsyncWrite {}
type UpgradedFramed = ironrdp_tokio::TokioFramed<Box<dyn AsyncReadWrite + Unpin + Send + Sync>>;

/// Direct TCP → TLS connection (no gateway).
async fn connect_direct(
    config: &Config,
    input_sender: &mpsc::UnboundedSender<RdpInputEvent>,
    cliprdr_factory: CliprdrFactoryRef<'_>,
) -> ConnectorResult<(ConnectionResult, UpgradedFramed)> {
    let dest = config.destination.to_string();
    let stream = TcpStream::connect(&dest)
        .await
        .map_err(|e| ironrdp_connector::custom_err!("TCP connect", e))?;
    let client_addr = stream
        .local_addr()
        .map_err(|e| ironrdp_connector::custom_err!("get socket local address", e))?;
    let framed = ironrdp_tokio::TokioFramed::new(stream);

    let connector = build_connector(config, client_addr, input_sender, cliprdr_factory);

    tls_handshake_and_finalize(framed, connector, config).await
}

/// RDS gateway TCP → gateway auth → TLS connection.
#[cfg(feature = "gateway")]
async fn connect_gateway(
    config: &Config,
    gw: &crate::config::GatewayConfig,
    input_sender: &mpsc::UnboundedSender<RdpInputEvent>,
    cliprdr_factory: CliprdrFactoryRef<'_>,
) -> ConnectorResult<(ConnectionResult, UpgradedFramed)> {
    use ironrdp_mstsgu::GwConnectTarget;

    // Build the GwConnectTarget.  `server` is the RDP target derived from `config.destination`.
    // TODO: preserve the destination port; ironrdp-mstsgu may currently hard-code 3389.
    let gw_target = GwConnectTarget {
        gw_endpoint: gw.endpoint.clone(),
        gw_user: gw.username.clone(),
        gw_pass: gw.password.clone(),
        server: config.destination.name().to_owned(),
    };

    let (gw_stream, client_addr) = ironrdp_mstsgu::GwClient::connect(&gw_target, &config.connector.client_name)
        .await
        .map_err(|e| ironrdp_connector::custom_err!("GW connect", e))?;

    let framed = ironrdp_tokio::TokioFramed::new(gw_stream);

    let connector = build_connector(config, client_addr, input_sender, cliprdr_factory);

    tls_handshake_and_finalize(framed, connector, config).await
}

/// RDCleanPath WebSocket → RDCleanPath handshake connection.
async fn connect_rdcleanpath_transport(
    config: &Config,
    rdcp: &RDCleanPathConfig,
    input_sender: &mpsc::UnboundedSender<RdpInputEvent>,
    cliprdr_factory: CliprdrFactoryRef<'_>,
) -> ConnectorResult<(ConnectionResult, UpgradedFramed)> {
    let hostname = rdcp
        .url
        .host_str()
        .ok_or_else(|| ironrdp_connector::general_err!("host missing from the URL"))?;
    let port = rdcp.url.port_or_known_default().unwrap_or(443);

    let socket = TcpStream::connect((hostname, port))
        .await
        .map_err(|e| ironrdp_connector::custom_err!("TCP connect", e))?;
    socket
        .set_nodelay(true)
        .map_err(|e| ironrdp_connector::custom_err!("set TCP_NODELAY", e))?;
    let client_addr = socket
        .local_addr()
        .map_err(|e| ironrdp_connector::custom_err!("get socket local address", e))?;

    let (ws, _) = tokio_tungstenite::client_async_tls(rdcp.url.as_str(), socket)
        .await
        .map_err(|e| ironrdp_connector::custom_err!("WS connect", e))?;
    let ws = crate::ws::websocket_compat(ws);
    let mut framed = ironrdp_tokio::TokioFramed::new(ws);

    let mut connector = build_connector(config, client_addr, input_sender, cliprdr_factory);

    let destination = config.destination.to_string();
    let (upgraded, server_public_key) =
        rdcleanpath_handshake(&mut framed, &mut connector, destination, rdcp.auth_token.clone(), None).await?;

    let connection_result = ironrdp_tokio::connect_finalize(
        upgraded,
        connector,
        &mut framed,
        &mut ReqwestNetworkClient::new(),
        (&config.destination).into(),
        server_public_key,
        config.kerberos_config.clone(),
    )
    .await?;

    let (ws, leftover_bytes) = framed.into_inner();
    let erased_stream: Box<dyn AsyncReadWrite + Unpin + Send + Sync> = Box::new(ws);
    let upgraded_framed = ironrdp_tokio::TokioFramed::new_with_leftover(erased_stream, leftover_bytes);

    Ok((connection_result, upgraded_framed))
}

// ── Shared TLS handshake ──────────────────────────────────────────────────────

async fn tls_handshake_and_finalize<S>(
    mut framed: ironrdp_tokio::TokioFramed<S>,
    mut connector: ironrdp_connector::ClientConnector,
    config: &Config,
) -> ConnectorResult<(ConnectionResult, UpgradedFramed)>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    let should_upgrade = ironrdp_tokio::connect_begin(&mut framed, &mut connector).await?;

    debug!("TLS upgrade");

    let (initial_stream, leftover_bytes) = framed.into_inner();

    let (tls_stream, tls_cert) = ironrdp_tls::upgrade(initial_stream, config.destination.name())
        .await
        .map_err(|e| ironrdp_connector::custom_err!("TLS upgrade", e))?;

    let upgraded = ironrdp_tokio::mark_as_upgraded(should_upgrade, &mut connector);

    let erased_stream: Box<dyn AsyncReadWrite + Unpin + Send + Sync> = Box::new(tls_stream);
    let mut upgraded_framed = ironrdp_tokio::TokioFramed::new_with_leftover(erased_stream, leftover_bytes);

    let server_public_key = ironrdp_tls::extract_tls_server_public_key(&tls_cert)
        .ok_or_else(|| ironrdp_connector::general_err!("unable to extract tls server public key"))?
        .to_owned();

    let connection_result = ironrdp_tokio::connect_finalize(
        upgraded,
        connector,
        &mut upgraded_framed,
        &mut ReqwestNetworkClient::new(),
        (&config.destination).into(),
        server_public_key,
        config.kerberos_config.clone(),
    )
    .await?;

    Ok((connection_result, upgraded_framed))
}

// ── RDCleanPath handshake ─────────────────────────────────────────────────────

async fn rdcleanpath_handshake<S>(
    framed: &mut ironrdp_tokio::Framed<S>,
    connector: &mut ironrdp_connector::ClientConnector,
    destination: String,
    proxy_auth_token: String,
    pcb: Option<String>,
) -> ConnectorResult<(ironrdp_tokio::Upgraded, Vec<u8>)>
where
    S: ironrdp_tokio::FramedRead + FramedWrite,
{
    use ironrdp_connector::Sequence as _;
    use x509_cert::der::Decode as _;

    #[derive(Clone, Copy, Debug)]
    struct RDCleanPathHint;
    const RDCLEANPATH_HINT: RDCleanPathHint = RDCleanPathHint;

    impl ironrdp_pdu::PduHint for RDCleanPathHint {
        fn find_size(&self, bytes: &[u8]) -> ironrdp_core::DecodeResult<Option<(bool, usize)>> {
            match ironrdp_rdcleanpath::RDCleanPathPdu::detect(bytes) {
                ironrdp_rdcleanpath::DetectionResult::Detected { total_length, .. } => Ok(Some((true, total_length))),
                ironrdp_rdcleanpath::DetectionResult::NotEnoughBytes => Ok(None),
                ironrdp_rdcleanpath::DetectionResult::Failed => {
                    Err(ironrdp_core::other_err!("RDCleanPathHint", "detection failed"))
                }
            }
        }
    }

    let mut buf = WriteBuf::new();
    info!("Begin RDCleanPath connection procedure");

    // Send X224 + RDCleanPath request.
    {
        let ironrdp_connector::ClientConnectorState::ConnectionInitiationSendRequest = connector.state else {
            return Err(ironrdp_connector::general_err!(
                "invalid connector state (send request)"
            ));
        };
        debug_assert!(connector.next_pdu_hint().is_none());
        let written = connector.step_no_input(&mut buf)?;
        let x224_pdu_len = written.size().expect("written size");
        debug_assert_eq!(x224_pdu_len, buf.filled_len());
        let x224_pdu = buf.filled().to_vec();

        let rdcleanpath_req =
            ironrdp_rdcleanpath::RDCleanPathPdu::new_request(x224_pdu, destination, proxy_auth_token, pcb)
                .map_err(|e| ironrdp_connector::custom_err!("new RDCleanPath request", e))?;
        debug!(message = ?rdcleanpath_req, "Send RDCleanPath request");
        let rdcleanpath_req = rdcleanpath_req
            .to_der()
            .map_err(|e| ironrdp_connector::custom_err!("RDCleanPath request encode", e))?;
        framed
            .write_all(&rdcleanpath_req)
            .await
            .map_err(|e| ironrdp_connector::custom_err!("couldn't write RDCleanPath request", e))?;
    }

    // Read RDCleanPath response.
    {
        let rdcleanpath_res = framed
            .read_by_hint(&RDCLEANPATH_HINT)
            .await
            .map_err(|e| ironrdp_connector::custom_err!("read RDCleanPath response", e))?;
        let rdcleanpath_res = ironrdp_rdcleanpath::RDCleanPathPdu::from_der(&rdcleanpath_res)
            .map_err(|e| ironrdp_connector::custom_err!("RDCleanPath response decode", e))?;
        debug!(message = ?rdcleanpath_res, "Received RDCleanPath PDU");

        let (x224_connection_response, server_cert_chain) = match rdcleanpath_res
            .into_enum()
            .map_err(|e| ironrdp_connector::custom_err!("invalid RDCleanPath PDU", e))?
        {
            ironrdp_rdcleanpath::RDCleanPath::Request { .. } => {
                return Err(ironrdp_connector::general_err!(
                    "received unexpected RDCleanPath type (request)"
                ));
            }
            ironrdp_rdcleanpath::RDCleanPath::Response {
                x224_connection_response,
                server_cert_chain,
                server_addr: _,
            } => (x224_connection_response, server_cert_chain),
            ironrdp_rdcleanpath::RDCleanPath::GeneralErr(error) => {
                return Err(ironrdp_connector::custom_err!("received RDCleanPath error", error));
            }
            ironrdp_rdcleanpath::RDCleanPath::NegotiationErr {
                x224_connection_response,
            } => {
                if let Ok(x224_confirm) = ironrdp_core::decode::<
                    ironrdp_pdu::x224::X224<ironrdp_pdu::nego::ConnectionConfirm>,
                >(&x224_connection_response)
                {
                    if let ironrdp_pdu::nego::ConnectionConfirm::Failure { code } = x224_confirm.0 {
                        let negotiation_failure = ironrdp_connector::NegotiationFailure::from(code);
                        return Err(ironrdp_connector::ConnectorError::new(
                            "RDP negotiation failed",
                            ironrdp_connector::ConnectorErrorKind::Negotiation(negotiation_failure),
                        ));
                    }
                }
                return Err(ironrdp_connector::general_err!(
                    "received RDCleanPath negotiation error"
                ));
            }
        };

        let ironrdp_connector::ClientConnectorState::ConnectionInitiationWaitConfirm { .. } = connector.state else {
            return Err(ironrdp_connector::general_err!(
                "invalid connector state (wait confirm)"
            ));
        };
        debug_assert!(connector.next_pdu_hint().is_some());

        buf.clear();
        let written = connector.step(x224_connection_response.as_bytes(), &mut buf)?;
        debug_assert!(written.is_nothing());

        let server_cert = server_cert_chain
            .into_iter()
            .next()
            .ok_or_else(|| ironrdp_connector::general_err!("server cert chain missing from rdcleanpath response"))?;

        let cert = x509_cert::Certificate::from_der(server_cert.as_bytes())
            .map_err(|e| ironrdp_connector::custom_err!("server cert decode", e))?;

        let server_public_key = cert
            .tbs_certificate
            .subject_public_key_info
            .subject_public_key
            .as_bytes()
            .ok_or_else(|| ironrdp_connector::general_err!("subject public key BIT STRING is not aligned"))?
            .to_owned();

        let should_upgrade = ironrdp_tokio::skip_connect_begin(connector);
        let upgraded = ironrdp_tokio::mark_as_upgraded(should_upgrade, connector);

        Ok((upgraded, server_public_key))
    }
}

// ── Desktop resize ───────────────────────────────────────────────────────────
//
// (#rdp-resize-defer 2026-11-13) A desktop resize has two ways to happen.
//
// The in-session way asks the server to re-layout the desktop over the
// Display Control virtual channel, which makes the server run the
// "Deactivation-Reactivation Sequence" and keep the connection alive. It needs
// that channel to be *open*, and the server opens it a moment after the
// session is established.
//
// The fallback way ends the session and runs the whole connection again at the
// new size.
//
// The old code took the fallback way whenever the channel was not open yet —
// which is exactly what a resize arriving right after the connection does. The
// fallback then raced the server's release of the previous session and was
// refused, so the tab died on the first resize. Instead the request is held
// until the channel is up (or the hold times out), and only then does the
// fallback kick in.

/// How long a resize may wait for the Display Control channel to come up.
const DISPLAY_CONTROL_READY_TIMEOUT: Duration = Duration::from_secs(3);

/// How often a held resize is retried.
const RESIZE_RETRY_INTERVAL: Duration = Duration::from_millis(200);

/// How long the client waits for the server to release the old session before
/// reconnecting at the new size.
const RESIZE_RECONNECT_GRACE: Duration = Duration::from_millis(300);

/// How many times a reconnect that follows a resize is retried.
const RESIZE_RECONNECT_ATTEMPTS: u32 = 3;

/// A resize request waiting for the Display Control channel.
struct PendingResize {
    width: u16,
    height: u16,
    scale_factor: u32,
    physical_size: Option<(u32, u32)>,
    deadline: tokio::time::Instant,
}

/// What came out of trying to put a resize request on the wire.
enum ResizeOutcome {
    /// The request was encoded and is ready to be sent.
    Encoded(Vec<u8>),
    /// The Display Control channel is not usable yet; the request can be
    /// retried.
    NotReady,
    /// The request cannot be encoded. The session is fine — only the resize is
    /// lost — so this must not end the session.
    Failed(String),
}

/// Whether the Display Control channel can carry a resize right now.
///
/// * `None` — the server has not opened the channel yet.
/// * `Some(false)` — the channel is open, but the server's capabilities have
///   not arrived. A monitor layout sent now is ignored by the server ("silent
///   failure"), so the request has to wait for them.
/// * `Some(true)` — the channel is ready.
///
/// The three states are what separates a resize that works from one that is
/// silently dropped: upstream only knows "the channel is open", so it sends
/// the layout into a channel that is not operational yet.
fn display_control_ready(active_stage: &mut ActiveStage) -> Option<bool> {
    let dvc = active_stage.get_dvc::<DisplayControlClient>()?;
    let processor = dvc.channel_processor_downcast_ref::<DisplayControlClient>()?;
    Some(processor.ready())
}

/// Encode a resize request for the Display Control channel.
fn try_encode_resize(
    active_stage: &mut ActiveStage,
    width: u16,
    height: u16,
    scale_factor: u32,
    physical_size: Option<(u32, u32)>,
) -> ResizeOutcome {
    // Wait for the channel to be open *and* ready: a monitor layout that
    // arrives before the server's capabilities is ignored by the server, and
    // the resize would be lost without a word.
    match display_control_ready(active_stage) {
        Some(true) => {}
        Some(false) | None => return ResizeOutcome::NotReady,
    }

    let width = u32::from(width);
    let height = u32::from(height);
    // Per MS-RDPEDISP the size has to fit the protocol's bounds and the width
    // has to be even; `adjust_display_size` does both.
    let (width, height) = MonitorLayoutEntry::adjust_display_size(width, height);
    match active_stage.encode_resize(width, height, Some(scale_factor), physical_size) {
        Some(Ok(frame)) => ResizeOutcome::Encoded(frame),
        Some(Err(err)) => ResizeOutcome::Failed(format!("{err:?}")),
        // The Display Control channel is not connected (yet).
        None => ResizeOutcome::NotReady,
    }
}

/// Backoff between the attempts of a resize-triggered reconnect.
fn resize_reconnect_delay(attempt: u32) -> Duration {
    match attempt {
        1 => Duration::from_millis(300),
        2 => Duration::from_millis(900),
        _ => Duration::from_millis(1800),
    }
}

// ── Active session ────────────────────────────────────────────────────────────

enum RdpControlFlow {
    ReconnectWithNewSize { width: u16, height: u16 },
    TerminatedGracefully(GracefulDisconnectReason),
}

async fn active_session(
    framed: UpgradedFramed,
    connection_result: ConnectionResult,
    output_event_sender: &mpsc::Sender<RdpOutputEvent>,
    input_event_receiver: &mut mpsc::UnboundedReceiver<RdpInputEvent>,
    fake_events_interval: Option<Duration>,
) -> SessionResult<RdpControlFlow> {
    let (mut reader, mut writer) = split_tokio_framed(framed);
    let desktop_size = connection_result.desktop_size;
    let mut image = DecodedImage::new(PixelFormat::RgbA32, desktop_size.width, desktop_size.height);

    // We retain the factory to drive the Deactivation-Reactivation Sequence locally.
    let activation_factory = connection_result.activation_factory;

    // (MTerm patch, #rdp-resize-decompressor) Keep a copy of the negotiated bulk
    // compression type. The Deactivation-Reactivation Sequence below rebuilds the
    // fast-path processor, and a processor without the matching decompressor
    // hands the server's compressed FastPath updates to the decoder as plaintext,
    // which fails and terminates the session. The type is fixed at MCS connect
    // time and does not change across reactivation, so the value captured here
    // stays valid for the life of the connection. Upstream IronRDP fixed this by
    // moving the decompressor into `ActiveStage` (unreleased as of 0.1.0).
    let compression_type = connection_result.compression_type;

    let mut active_stage = ActiveStageBuilder {
        static_channels: connection_result.static_channels,
        user_channel_id: connection_result.user_channel_id,
        io_channel_id: connection_result.io_channel_id,
        message_channel_id: connection_result.message_channel_id,
        share_id: connection_result.share_id,
        compression_type: connection_result.compression_type,
        enable_server_pointer: connection_result.enable_server_pointer,
        pointer_software_rendering: connection_result.pointer_software_rendering,
    }
    .build();

    // Timer interval for driving clipboard lock timeouts.
    let mut cleanup_interval = tokio::time::interval(Duration::from_secs(5));

    // (#rdp-resize-defer 2026-11-13) A resize waiting for the Display Control
    // channel to come up, and the timer that retries it. Only one request is
    // kept: a window drag produces a whole series of sizes and only the last
    // one is worth sending.
    let mut pending_resize: Option<PendingResize> = None;
    let mut resize_retry = tokio::time::interval(RESIZE_RETRY_INTERVAL);
    resize_retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Anti-idle: track the time of the last real input and the last known mouse position so we can
    // synthesize a no-op mouse move when the session has been idle for too long. Default to the
    // middle of the screen so a synthetic move before any real input doesn't snap the pointer to a
    // corner.
    let mut last_input = tokio::time::Instant::now();
    let mut last_mouse_pos = (desktop_size.width / 2, desktop_size.height / 2);
    let mut fake_events_interval =
        fake_events_interval.map(|interval| tokio::time::interval(core::cmp::max(interval, Duration::from_secs(1))));

    let disconnect_reason = 'outer: loop {
        let outputs = tokio::select! {
            frame = reader.read_pdu() => {
                let (action, payload) = frame.map_err(|e| ironrdp_session::custom_err!("read frame", e))?;
                trace!(?action, frame_length = payload.len(), "Frame received");
                active_stage.process(&mut image, action, &payload)?
            }
            input_event = input_event_receiver.recv() => {
                let input_event = input_event.ok_or_else(|| ironrdp_session::general_err!("GUI is stopped"))?;

                last_input = tokio::time::Instant::now();

                match input_event {
                    RdpInputEvent::Resize { width, height, scale_factor, physical_size } => {
                        trace!(width, height, "Resize event");
                        match try_encode_resize(&mut active_stage, width, height, scale_factor, physical_size) {
                            ResizeOutcome::Encoded(frame) => {
                                pending_resize = None;
                                vec![ActiveStageOutput::ResponseFrame(frame)]
                            }
                            ResizeOutcome::Failed(error) => {
                                // A resize that cannot be encoded is a lost
                                // resize, not a lost session: the old code
                                // propagated the error and killed the tab.
                                warn!(width, height, %error, "Could not encode the desktop resize");
                                pending_resize = None;
                                Vec::new()
                            }
                            ResizeOutcome::NotReady => {
                                // The Display Control channel is not open yet.
                                // Hold the request instead of ending the session
                                // and reconnecting; the retry below sends it as
                                // soon as the channel is up.
                                debug!(width, height, "Display Control is not ready; holding the resize");
                                pending_resize = Some(PendingResize {
                                    width,
                                    height,
                                    scale_factor,
                                    physical_size,
                                    deadline: tokio::time::Instant::now() + DISPLAY_CONTROL_READY_TIMEOUT,
                                });
                                Vec::new()
                            }
                        }
                    }
                    RdpInputEvent::FastPath(events) => {
                        trace!(?events);
                        for event in &events {
                            if let FastPathInputEvent::MouseEvent(mouse) = event {
                                last_mouse_pos = (mouse.x_position, mouse.y_position);
                            }
                        }
                        active_stage.process_fastpath_input(&mut image, &events)?
                    }
                    RdpInputEvent::Close => {
                        active_stage.graceful_shutdown()?
                    }
                    #[cfg(feature = "clipboard")]
                    RdpInputEvent::Clipboard(event) => {
                        if let Some(cliprdr_client) = active_stage.get_svc_processor_mut::<ironrdp_cliprdr::CliprdrClient>() {
                            if let Some(svc_messages) = match event {
                                ClipboardMessage::SendInitiateCopy(formats) => {
                                    Some(cliprdr_client.initiate_copy(&formats)
                                        .map_err(|e| ironrdp_session::custom_err!("CLIPRDR", e))?)
                                }
                                ClipboardMessage::SendInitiateFileCopy(files) => {
                                    Some(cliprdr_client.initiate_file_copy(files)
                                        .map_err(|e| ironrdp_session::custom_err!("CLIPRDR", e))?)
                                }
                                ClipboardMessage::SendFormatData(response) => {
                                    Some(cliprdr_client.submit_format_data(response)
                                        .map_err(|e| ironrdp_session::custom_err!("CLIPRDR", e))?)
                                }
                                ClipboardMessage::SendInitiatePaste(format) => {
                                    Some(cliprdr_client.initiate_paste(format)
                                        .map_err(|e| ironrdp_session::custom_err!("CLIPRDR", e))?)
                                }
                                ClipboardMessage::SendFileContentsRequest(request) => {
                                    Some(cliprdr_client.request_file_contents(request)
                                        .map_err(|e| ironrdp_session::custom_err!("CLIPRDR", e))?)
                                }
                                ClipboardMessage::SendFileContentsResponse(response) => {
                                    Some(cliprdr_client.submit_file_contents(response)
                                        .map_err(|e| ironrdp_session::custom_err!("CLIPRDR", e))?)
                                }
                                ClipboardMessage::Error(e) => {
                                    error!("Clipboard backend error: {}", e);
                                    None
                                }
                            } {
                                let frame = active_stage.process_svc_processor_messages(svc_messages)?;
                                vec![ActiveStageOutput::ResponseFrame(frame)]
                            } else {
                                Vec::new()
                            }
                        } else {
                            warn!("Clipboard event received, but Cliprdr is not available");
                            Vec::new()
                        }
                    }
                    RdpInputEvent::SendDvcMessages { channel_id, messages } => {
                        trace!(channel_id, ?messages, "Send DVC messages");
                        let frame = active_stage.encode_dvc_messages(messages)?;
                        vec![ActiveStageOutput::ResponseFrame(frame)]
                    }
                }
            }
            _ = resize_retry.tick(), if pending_resize.is_some() => {
                // (#rdp-resize-defer 2026-11-13) Retry the held resize. The
                // channel usually opens within a second or two of the session
                // being established, so this is where a resize that arrived too
                // early finally gets sent — with the connection untouched.
                let (width, height, scale_factor, physical_size, deadline) = match pending_resize.as_ref() {
                    Some(pending) => (
                        pending.width,
                        pending.height,
                        pending.scale_factor,
                        pending.physical_size,
                        pending.deadline,
                    ),
                    None => (0, 0, 100, None, tokio::time::Instant::now()),
                };

                if tokio::time::Instant::now() < deadline {
                    match try_encode_resize(&mut active_stage, width, height, scale_factor, physical_size) {
                        ResizeOutcome::Encoded(frame) => {
                            debug!(width, height, "Display Control is ready; sending the held resize");
                            pending_resize = None;
                            vec![ActiveStageOutput::ResponseFrame(frame)]
                        }
                        ResizeOutcome::Failed(error) => {
                            warn!(width, height, %error, "Could not encode the held desktop resize");
                            pending_resize = None;
                            Vec::new()
                        }
                        ResizeOutcome::NotReady => Vec::new(),
                    }
                } else {
                    // The channel never came up: the server does not do
                    // mid-session resizes. Fall back to reconnecting at the
                    // new size — but say goodbye first, so the server releases
                    // the old session and the new connection is not refused
                    // because of it.
                    warn!(width, height, "Display Control did not come up; reconnecting with the new size");
                    let width = u16::try_from(width).expect("always in the range");
                    let height = u16::try_from(height).expect("always in the range");

                    if let Ok(outputs) = active_stage.graceful_shutdown() {
                        for output in outputs {
                            if let ActiveStageOutput::ResponseFrame(frame) = output {
                                let _ = writer.write_all(&frame).await;
                            }
                        }
                    }
                    tokio::time::sleep(RESIZE_RECONNECT_GRACE).await;
                    return Ok(RdpControlFlow::ReconnectWithNewSize { width, height });
                }
            }
            _ = cleanup_interval.tick() => {
                // Drive clipboard lock timeout cleanup.
                #[cfg(feature = "clipboard")]
                if let Some(cliprdr_client) = active_stage.get_svc_processor_mut::<ironrdp_cliprdr::CliprdrClient>() {
                    match cliprdr_client.drive_timeouts() {
                        Ok(svc_messages) => {
                            let frame = active_stage.process_svc_processor_messages(svc_messages)?;
                            if !frame.is_empty() {
                                vec![ActiveStageOutput::ResponseFrame(frame)]
                            } else {
                                Vec::new()
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "Clipboard timeout cleanup failed");
                            Vec::new()
                        }
                    }
                } else {
                    Vec::new()
                }
                #[cfg(not(feature = "clipboard"))]
                Vec::new()
            }
            _ = async { match fake_events_interval.as_mut() {
                Some(interval) => interval.tick().await,
                None => core::future::pending().await,
            }} => {
                // Anti-idle: synthesize a no-op mouse move if the session has been idle for at least
                // the configured interval, keeping the connection alive without user interaction.
                if last_input.elapsed() >= fake_events_interval.as_ref().map_or(Duration::MAX, |i| i.period()) {
                    last_input = tokio::time::Instant::now();
                    let mut events = SmallVec::<[FastPathInputEvent; 2]>::new();
                    events.push(FastPathInputEvent::MouseEvent(MousePdu {
                        flags: PointerFlags::MOVE,
                        number_of_wheel_rotation_units: 0,
                        x_position: last_mouse_pos.0,
                        y_position: last_mouse_pos.1,
                    }));
                    active_stage.process_fastpath_input(&mut image, &events)?
                } else {
                    Vec::new()
                }
            }
        };

        for out in outputs {
            match out {
                ActiveStageOutput::ResponseFrame(frame) => writer
                    .write_all(&frame)
                    .await
                    .map_err(|e| ironrdp_session::custom_err!("write response", e))?,
                ActiveStageOutput::GraphicsUpdate(_region) => {
                    let buffer: Vec<u32> = image
                        .data()
                        .chunks_exact(4)
                        .map(|pixel| {
                            let r = pixel[0];
                            let g = pixel[1];
                            let b = pixel[2];
                            u32::from_be_bytes([0, r, g, b])
                        })
                        .collect();
                    output_event_sender
                        .send(RdpOutputEvent::Image {
                            buffer,
                            width: NonZeroU16::new(image.width())
                                .ok_or_else(|| ironrdp_session::general_err!("width is zero"))?,
                            height: NonZeroU16::new(image.height())
                                .ok_or_else(|| ironrdp_session::general_err!("height is zero"))?,
                        })
                        .await
                        .map_err(|e| ironrdp_session::custom_err!("output_event_sender", e))?;
                }
                ActiveStageOutput::PointerDefault => {
                    output_event_sender
                        .send(RdpOutputEvent::PointerDefault)
                        .await
                        .map_err(|e| ironrdp_session::custom_err!("output_event_sender", e))?;
                }
                ActiveStageOutput::PointerHidden => {
                    output_event_sender
                        .send(RdpOutputEvent::PointerHidden)
                        .await
                        .map_err(|e| ironrdp_session::custom_err!("output_event_sender", e))?;
                }
                ActiveStageOutput::PointerPosition { x, y } => {
                    output_event_sender
                        .send(RdpOutputEvent::PointerPosition { x, y })
                        .await
                        .map_err(|e| ironrdp_session::custom_err!("output_event_sender", e))?;
                }
                ActiveStageOutput::PointerBitmap(pointer) => {
                    output_event_sender
                        .send(RdpOutputEvent::PointerBitmap(pointer))
                        .await
                        .map_err(|e| ironrdp_session::custom_err!("output_event_sender", e))?;
                }
                ActiveStageOutput::DeactivateAll => {
                    // Deactivation-Reactivation Sequence:
                    // https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-rdpbcgr/dfc234ce-481a-4674-9a5d-2a7bafb14432
                    debug!("Executing Deactivation-Reactivation Sequence");
                    let mut connection_activation = activation_factory.create();
                    let mut buf = WriteBuf::new();
                    'activation_seq: loop {
                        let written = single_sequence_step_read(&mut reader, &mut connection_activation, &mut buf)
                            .await
                            .map_err(|e| {
                                ironrdp_session::custom_err!("read deactivation-reactivation sequence step", e)
                            })?;
                        if written.size().is_some() {
                            writer.write_all(buf.filled()).await.map_err(|e| {
                                ironrdp_session::custom_err!("write deactivation-reactivation sequence step", e)
                            })?;
                        }
                        if let ConnectionActivationState::Finalized {
                            desktop_size,
                            share_id,
                            enable_server_pointer,
                            pointer_software_rendering,
                        } = connection_activation.connection_activation_state()
                        {
                            debug!(?desktop_size, "Deactivation-Reactivation Sequence completed");
                            image = DecodedImage::new(PixelFormat::RgbA32, desktop_size.width, desktop_size.height);
                            // (MTerm patch, #rdp-resize-decompressor) Recreate the bulk
                            // decompressor for the compression type negotiated at connect
                            // time instead of dropping it: the server keeps sending
                            // compressed FastPath updates after the reactivation, and a
                            // processor built without one passes those bytes to the
                            // decoder undecoded, which fails and ends the session. See
                            // the comment where `compression_type` is captured above.
                            let bulk_decompressor = compression_type.and_then(|ct| {
                                let bulk_type = match ct {
                                    ironrdp_pdu::rdp::client_info::CompressionType::K8 => {
                                        ironrdp_bulk::CompressionType::Rdp4
                                    }
                                    ironrdp_pdu::rdp::client_info::CompressionType::K64 => {
                                        ironrdp_bulk::CompressionType::Rdp5
                                    }
                                    ironrdp_pdu::rdp::client_info::CompressionType::Rdp6 => {
                                        ironrdp_bulk::CompressionType::Rdp6
                                    }
                                    ironrdp_pdu::rdp::client_info::CompressionType::Rdp61 => {
                                        ironrdp_bulk::CompressionType::Rdp61
                                    }
                                };
                                BulkCompressor::new(bulk_type).ok()
                            });
                            active_stage.set_fastpath_processor(
                                fast_path::ProcessorBuilder {
                                    io_channel_id: connection_activation.io_channel_id(),
                                    user_channel_id: connection_activation.user_channel_id(),
                                    share_id,
                                    enable_server_pointer,
                                    pointer_software_rendering,
                                    bulk_decompressor,
                                }
                                .build(),
                            );
                            active_stage.set_share_id(share_id);
                            active_stage.set_enable_server_pointer(enable_server_pointer);
                            break 'activation_seq;
                        }
                    }
                }
                ActiveStageOutput::MultitransportRequest(pdu) => {
                    debug!(
                        request_id = pdu.request_id,
                        requested_protocol = ?pdu.requested_protocol,
                        "Multitransport request received (UDP transport not implemented)"
                    );
                }
                ActiveStageOutput::AutoDetect(request) => {
                    debug!(?request, "Auto-detect");
                }
                ActiveStageOutput::Terminate(reason) => break 'outer reason,
            }
        }
    };

    Ok(RdpControlFlow::TerminatedGracefully(disconnect_reason))
}
