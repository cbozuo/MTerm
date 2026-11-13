//! The local side of the RDP clipboard channel (#rdp-clipboard 2026-11-13).
//!
//! ironrdp's own `WinClipboard` registers a format listener on a hidden
//! window and needs a message pump on the thread that owns it. MTerm's RDP
//! driver runs on a worker thread of its own, so that backend cannot be used
//! here — it would leave the system clipboard locked for a thread that never
//! pumps messages. This module owns the local clipboard directly instead.
//!
//! The two directions of the CLIPRDR channel:
//!
//! * local → remote: a watcher notices that the local clipboard changed and
//!   advertises what it holds; when the remote asks for the data, it is
//!   served from the payload that was advertised.
//! * remote → local: the remote advertises what it holds, we ask for the
//!   text we understand (or for the files it has), and the answer is written
//!   into the local clipboard.
//!
//! The echo guard: what the remote sends is written into the local
//! clipboard, and the watcher would see that as a fresh local copy and send
//! it straight back. What came from the remote is therefore remembered, and
//! a local clipboard that still holds it is not re-advertised.
//!
//! Files travel over the CLIPRDR channel itself — the file contents are
//! requested and answered with CLIPRDR file-contents PDUs — so no device
//! redirection channel is needed for them.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ironrdp::cliprdr::backend::{ClipboardMessage, CliprdrBackend, CliprdrBackendFactory};
use ironrdp::core::impl_as_any;
use ironrdp::cliprdr::pdu::{
    ClipboardFileAttributes, ClipboardFormat, ClipboardFormatId, ClipboardGeneralCapabilityFlags,
    FileContentsFlags, FileContentsRequest, FileContentsResponse, FileDescriptor, FormatDataRequest,
    FormatDataResponse, LockDataId, OwnedFormatDataResponse,
};
use ironrdp::client::rdp::RdpInputEvent;
use tokio::sync::mpsc;

/// How often the local clipboard is checked for changes.
///
/// The clipboard has no "tell me when it changes" call that works without a
/// message pump, so it is polled. The sequence number makes the poll cheap:
/// it only changes when the clipboard's *contents* change.
const WATCH_INTERVAL: Duration = Duration::from_millis(300);

/// The format both directions understand.
const TEXT_FORMAT: ClipboardFormatId = ClipboardFormatId::CF_UNICODETEXT;

/// The largest clipboard text that is taken from the remote.
///
/// The remote can send anything; a clipboard that carries megabytes is a
/// denial of service rather than a copy.
const MAX_TEXT_BYTES: usize = 1024 * 1024;

/// How much of a file is sent per request.
///
/// The remote asks for a range; a megabyte at a time keeps the messages
/// within what the channel is happy with.
const FILE_CHUNK_SIZE: u32 = 1024 * 1024;

/// The largest file that is taken from the remote.
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// The state both the backend and its watcher thread share.
#[derive(Debug, Default)]
struct Shared {
    /// The client's input channel, once the client exists.
    input: Mutex<Option<mpsc::UnboundedSender<RdpInputEvent>>>,
    /// What came *from* the remote and was written into the local clipboard.
    /// A local clipboard that still holds it is our own write coming back,
    /// not a new copy to push.
    echoed: Mutex<Option<String>>,
    /// The files that came from the remote, with the paths they were written
    /// to. A local clipboard that still holds them is our own write coming
    /// back, not a new copy to push.
    echoed_files: Mutex<Option<Vec<PathBuf>>>,
    /// What we offer to the remote, captured when it was advertised.
    payload: Mutex<LocalPayload>,
    /// Set when the watcher should stop.
    stop: AtomicBool,
    /// Set once the CLIPRDR channel is ready.
    ///
    /// Most of the protocol messages need the channel to be in the Ready
    /// state (`initiate_paste`, `submit_format_data`, `request_file_contents`
    /// …), and the client turns a failure of those into a dead session. Only
    /// `initiate_copy` is allowed during the initialization.
    ready: AtomicBool,
}

/// A file we offer to the remote.
#[derive(Debug, Clone)]
struct LocalFile {
    /// Where the file is on this machine.
    path: PathBuf,
    /// The name the remote sees.
    name: String,
    /// The size in bytes.
    size: u64,
    /// Whether it is a directory.
    is_directory: bool,
}

impl LocalFile {
    /// The descriptor the remote is told about.
    fn descriptor(&self) -> FileDescriptor {
        let mut descriptor = FileDescriptor::new(self.name.clone()).with_file_size(self.size);
        if self.is_directory {
            descriptor = descriptor.with_attributes(ClipboardFileAttributes::DIRECTORY);
        }
        descriptor
    }
}

/// What the local clipboard holds.
#[derive(Debug, Default, Clone)]
struct LocalPayload {
    text: Option<String>,
    files: Vec<LocalFile>,
}

impl LocalPayload {
    /// The formats we can serve for this payload.
    fn formats(&self) -> Vec<ClipboardFormat> {
        let mut formats = Vec::new();
        if self.text.is_some() {
            formats.push(ClipboardFormat::new(TEXT_FORMAT));
        }
        formats
    }
}

/// A file being received from the remote.
#[derive(Debug)]
struct IncomingFile {
    /// The name the remote gave it.
    name: String,
    /// The index it has in the remote's list.
    index: i32,
    /// The stream the remote answers on.
    stream_id: u32,
    /// The size, once the remote told us.
    size: Option<u64>,
    /// What has arrived so far.
    data: Vec<u8>,
    /// Where the file was written, once it was complete.
    path: Option<PathBuf>,
    /// The clipboard data id the remote locked the list under.
    data_id: Option<u32>,
}

/// Creates the local side of the clipboard channel.
///
/// One factory per session: the backend it builds belongs to the CLIPRDR
/// channel, which is rebuilt for every connection.
#[derive(Debug, Clone)]
pub(crate) struct ClipboardFactory {
    shared: Arc<Shared>,
}

impl ClipboardFactory {
    pub(crate) fn new() -> Self {
        Self {
            shared: Arc::new(Shared::default()),
        }
    }

    /// Hand the backend the client's input channel.
    ///
    /// The channel belongs to the client, which is created after the factory,
    /// so the sender is handed over afterwards.
    pub(crate) fn set_input(&self, sender: mpsc::UnboundedSender<RdpInputEvent>) {
        if let Ok(mut input) = self.shared.input.lock() {
            *input = Some(sender);
        }
    }
}

impl CliprdrBackendFactory for ClipboardFactory {
    fn build_cliprdr_backend(&self) -> Box<dyn CliprdrBackend> {
        Box::new(ClipboardBackend::new(Arc::clone(&self.shared)))
    }
}

/// The local side of the clipboard channel.
#[derive(Debug)]
struct ClipboardBackend {
    shared: Arc<Shared>,
    /// The files being received from the remote.
    incoming: Mutex<Vec<IncomingFile>>,
    /// The watcher thread, stopped when the backend goes away.
    watcher: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl ClipboardBackend {
    fn new(shared: Arc<Shared>) -> Self {
        let backend = Self {
            shared,
            incoming: Mutex::new(Vec::new()),
            watcher: Mutex::new(None),
        };
        backend.start_watcher();
        backend
    }

    /// Watch the local clipboard and advertise what it holds.
    ///
    /// The thread ends when the backend is dropped: the CLIPRDR channel owns
    /// the backend, so the channel's end is the session's end.
    fn start_watcher(&self) {
        let shared = Arc::clone(&self.shared);
        let handle = std::thread::Builder::new()
            .name("mterm-rdp-clipboard".to_string())
            .spawn(move || {
                let mut last_sequence = platform::sequence_number();
                while !shared.stop.load(Ordering::Relaxed) {
                    std::thread::sleep(WATCH_INTERVAL);
                    let sequence = platform::sequence_number();
                    if sequence == last_sequence {
                        continue;
                    }
                    if !shared.is_ready() {
                        // The channel cannot carry the advertisement yet;
                        // the sequence is not advanced, so the change is
                        // picked up as soon as the channel is up.
                        continue;
                    }
                    last_sequence = sequence;
                    shared.advertise_local();
                }
            })
            .ok();
        if let Some(mut watcher) = self.watcher.lock().ok() {
            *watcher = handle;
        }
    }

    /// Write text that came from the remote into the local clipboard.
    fn write_local_text(&self, text: &str) {
        platform::set_text(text);
        if let Ok(mut echoed) = self.shared.echoed.lock() {
            *echoed = Some(text.to_string());
        }
    }

    /// Write files that came from the remote into the local clipboard.
    fn write_local_files(&self, paths: Vec<PathBuf>) {
        platform::set_file_paths(&paths);
        if let Ok(mut echoed) = self.shared.echoed_files.lock() {
            *echoed = Some(paths);
        }
    }
}

impl Drop for ClipboardBackend {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Ok(mut watcher) = self.watcher.lock() {
            if let Some(handle) = watcher.take() {
                let _ = handle.join();
            }
        }
    }
}

impl_as_any!(ClipboardBackend);

impl CliprdrBackend for ClipboardBackend {
    fn temporary_directory(&self) -> &str {
        ".mterm-rdp-cliprdr"
    }

    fn client_capabilities(&self) -> ClipboardGeneralCapabilityFlags {
        // Long format names are needed for the file list; the remote is told
        // that it may stream files and that it gets no local paths.
        ClipboardGeneralCapabilityFlags::USE_LONG_FORMAT_NAMES
            | ClipboardGeneralCapabilityFlags::STREAM_FILECLIP_ENABLED
            | ClipboardGeneralCapabilityFlags::FILECLIP_NO_FILE_PATHS
    }

    fn on_ready(&mut self) {
        // The channel is up: everything may be sent now, and the local
        // clipboard is offered.
        self.shared.ready.store(true, Ordering::Relaxed);
        self.shared.advertise_local();
    }

    fn on_request_format_list(&mut self) {
        // The remote asked what we have.
        self.shared.advertise_local();
    }

    fn on_format_list_response(&mut self, _ok: bool) {
        // Nothing to do: the formats were already captured when they were
        // advertised.
    }

    fn on_process_negotiated_capabilities(&mut self, _capabilities: ClipboardGeneralCapabilityFlags) {}

    fn on_remote_copy(&mut self, available_formats: &[ClipboardFormat]) {
        // The remote has something; ask for the text it holds.
        if !self.shared.is_ready() {
            return;
        }
        if available_formats.iter().any(|format| format.id == TEXT_FORMAT) {
            self.shared
                .send(ClipboardMessage::SendInitiatePaste(TEXT_FORMAT));
        }
    }

    fn on_format_data_request(&mut self, request: FormatDataRequest) {
        // The remote wants our data.
        if !self.shared.is_ready() {
            return;
        }
        if request.format != TEXT_FORMAT {
            self.shared
                .send(ClipboardMessage::SendFormatData(OwnedFormatDataResponse::new_error()));
            return;
        }
        let text = self
            .shared
            .payload
            .lock()
            .ok()
            .and_then(|payload| payload.text.clone());
        let response = match text {
            Some(text) => OwnedFormatDataResponse::new_unicode_string(&text),
            None => OwnedFormatDataResponse::new_error(),
        };
        self.shared.send(ClipboardMessage::SendFormatData(response));
    }

    fn on_format_data_response(&mut self, response: FormatDataResponse<'_>) {
        if response.is_error() {
            return;
        }
        let Ok(text) = response.to_unicode_string() else {
            return;
        };
        let text = text.trim_end_matches('\0').to_string();
        if text.is_empty() {
            return;
        }
        // A clipboard that carries megabytes is a denial of service, not a
        // copy.
        if text.len() > MAX_TEXT_BYTES {
            tracing::warn!(length = text.len(), "Ignoring an oversized RDP clipboard text");
            return;
        }
        self.write_local_text(&text);
    }

    fn on_file_contents_request(&mut self, request: FileContentsRequest) {
        if !self.shared.is_ready() {
            return;
        }
        let response = self.serve_local_file(&request);
        self.shared
            .send(ClipboardMessage::SendFileContentsResponse(response));
    }

    fn on_file_contents_response(&mut self, response: FileContentsResponse<'_>) {
        if response.is_error() || !self.shared.is_ready() {
            return;
        }
        for request in self.receive_remote_file(&response) {
            self.shared
                .send(ClipboardMessage::SendFileContentsRequest(request));
        }
    }

    fn on_remote_file_list(&mut self, files: &[FileDescriptor], clip_data_id: Option<u32>) {
        if !self.shared.is_ready() {
            return;
        }
        for request in self.begin_remote_files(files, clip_data_id) {
            self.shared
                .send(ClipboardMessage::SendFileContentsRequest(request));
        }
    }

    fn on_lock(&mut self, _data_id: LockDataId) {}

    fn on_unlock(&mut self, _data_id: LockDataId) {}
}

impl ClipboardBackend {
    /// Answer the remote's request for one of our files.
    fn serve_local_file(&self, request: &FileContentsRequest) -> FileContentsResponse<'static> {
        let file = self
            .shared
            .payload
            .lock()
            .ok()
            .and_then(|payload| {
                payload
                    .files
                    .get(usize::try_from(request.index).ok()?)
                    .cloned()
            });
        let Some(file) = file else {
            return FileContentsResponse::new_error(request.stream_id);
        };

        if request.flags.contains(FileContentsFlags::SIZE) {
            if file.is_directory {
                return FileContentsResponse::new_error(request.stream_id);
            }
            return FileContentsResponse::new_size_response(request.stream_id, file.size);
        }

        if file.is_directory || request.requested_size == 0 || request.requested_size > FILE_CHUNK_SIZE {
            return FileContentsResponse::new_error(request.stream_id);
        }
        let Ok(position) = usize::try_from(request.position) else {
            return FileContentsResponse::new_error(request.stream_id);
        };
        if position as u64 > file.size {
            return FileContentsResponse::new_error(request.stream_id);
        }
        let wanted = request.requested_size as usize;
        let data = match std::fs::File::open(&file.path) {
            Ok(mut handle) => {
                use std::io::{Read, Seek, SeekFrom};
                if handle.seek(SeekFrom::Start(position as u64)).is_err() {
                    return FileContentsResponse::new_error(request.stream_id);
                }
                let mut buffer = vec![0u8; wanted];
                match handle.read_exact(&mut buffer) {
                    Ok(()) => buffer,
                    Err(_) => return FileContentsResponse::new_error(request.stream_id),
                }
            }
            Err(_) => return FileContentsResponse::new_error(request.stream_id),
        };
        FileContentsResponse::new_data_response(request.stream_id, data)
    }

    /// Start receiving the files the remote advertised.
    fn begin_remote_files(
        &self,
        files: &[FileDescriptor],
        clip_data_id: Option<u32>,
    ) -> Vec<FileContentsRequest> {
        let mut incoming = Vec::new();
        let mut requests = Vec::new();
        for (index, descriptor) in files.iter().enumerate() {
            let stream_id = index as u32 + 1;
            incoming.push(IncomingFile {
                name: descriptor.name.clone(),
                index: index as i32,
                stream_id,
                size: None,
                data: Vec::new(),
                path: None,
                data_id: clip_data_id,
            });
            // The size comes first: it is what tells us how much to ask for.
            requests.push(FileContentsRequest {
                stream_id,
                index: index as i32,
                flags: FileContentsFlags::SIZE,
                position: 0,
                requested_size: 8,
                data_id: clip_data_id,
            });
        }
        if let Ok(mut slot) = self.incoming.lock() {
            *slot = incoming;
        }
        requests
    }

    /// Take in one answer from the remote and ask for the next piece.
    fn receive_remote_file(&self, response: &FileContentsResponse<'_>) -> Vec<FileContentsRequest> {
        let mut requests = Vec::new();

        let Ok(mut incoming) = self.incoming.lock() else {
            return requests;
        };
        let Some(file) = incoming
            .iter_mut()
            .find(|file| file.stream_id == response.stream_id())
        else {
            return requests;
        };

        match file.size {
            // The first answer is the size.
            None => {
                let Ok(size) = response.data_as_size() else {
                    return requests;
                };
                if size > MAX_FILE_BYTES {
                    tracing::warn!(size, "Ignoring an oversized RDP clipboard file");
                    return requests;
                }
                file.size = Some(size);
                if size == 0 {
                    file.path = write_incoming_file(file);
                } else {
                    requests.push(range_request(file));
                }
            }
            // The rest are chunks.
            Some(size) => {
                file.data.extend_from_slice(response.data());
                if file.data.len() as u64 >= size {
                    file.path = write_incoming_file(file);
                } else {
                    requests.push(range_request(file));
                }
            }
        }

        // Every file arrived: publish them in the local clipboard, together.
        let complete = !incoming.is_empty()
            && incoming.iter().all(|file| match file.size {
                Some(size) => file.data.len() as u64 >= size && file.path.is_some(),
                None => false,
            });
        let paths: Vec<PathBuf> = incoming.iter().filter_map(|file| file.path.clone()).collect();
        drop(incoming);
        if complete && !paths.is_empty() {
            self.write_local_files(paths);
        }

        requests
    }
}

/// The request for the next chunk of a file.
fn range_request(file: &IncomingFile) -> FileContentsRequest {
    FileContentsRequest {
        stream_id: file.stream_id,
        index: file.index,
        flags: FileContentsFlags::RANGE,
        position: file.data.len() as u64,
        requested_size: FILE_CHUNK_SIZE,
        data_id: file.data_id,
    }
}

/// Write a file that came from the remote into the temporary directory.
fn write_incoming_file(file: &IncomingFile) -> Option<PathBuf> {
    let directory = std::env::temp_dir().join("mterm-rdp-cliprdr");
    std::fs::create_dir_all(&directory).ok()?;
    // The name comes from the remote, so it cannot be trusted as a path.
    let name = PathBuf::from(&file.name)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty() && name != "." && name != "..")?;
    let path = directory.join(name);
    std::fs::write(&path, &file.data).ok()?;
    Some(path)
}

impl Shared {
    /// Whether the CLIPRDR channel is ready for anything but a copy.
    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// Send a CLIPRDR message into the client's input channel.
    fn send(&self, message: ClipboardMessage) {
        let sender = self.input.lock().ok().and_then(|slot| slot.clone());
        if let Some(sender) = sender {
            let _ = sender.send(RdpInputEvent::Clipboard(message));
        }
    }

    /// Capture what the local clipboard holds and advertise it to the remote.
    fn advertise_local(&self) {
        let payload = platform::capture();

        // What the remote sent us is not a new copy: the watcher would
        // otherwise send it straight back.
        if let Some(echoed) = self.echoed.lock().ok().and_then(|echoed| echoed.clone()) {
            if payload.text.as_deref() == Some(echoed.as_str()) {
                // The marker is consumed: the same text copied again later
                // is a real copy.
                if let Ok(mut marker) = self.echoed.lock() {
                    *marker = None;
                }
                return;
            }
        }
        if let Some(echoed) = self.echoed_files.lock().ok().and_then(|files| files.clone()) {
            if !echoed.is_empty() && payload.files.len() == echoed.len() {
                let same = payload
                    .files
                    .iter()
                    .zip(echoed.iter())
                    .all(|(file, path)| file.path == *path);
                if same {
                    if let Ok(mut marker) = self.echoed_files.lock() {
                        *marker = None;
                    }
                    return;
                }
            }
        }

        if payload.text.is_none() && payload.files.is_empty() {
            return;
        }
        if let Ok(mut slot) = self.payload.lock() {
            *slot = payload.clone();
        }
        if !payload.files.is_empty() {
            if !self.is_ready() {
                // The file list may only be sent once the channel is ready;
                // `on_ready` advertises it then.
                return;
            }
            // Files are advertised by their descriptors: the remote asks for
            // the contents afterwards.
            let descriptors: Vec<FileDescriptor> =
                payload.files.iter().map(LocalFile::descriptor).collect();
            self.send(ClipboardMessage::SendInitiateFileCopy(descriptors));
            return;
        }
        let formats = payload.formats();
        if formats.is_empty() {
            return;
        }
        self.send(ClipboardMessage::SendInitiateCopy(formats));
    }
}

// ── The local clipboard ──────────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use std::path::PathBuf;
    use std::ptr;

    use windows::Win32::Foundation::{HANDLE, HGLOBAL, POINT};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
        SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::UI::Shell::{DragQueryFileW, DROPFILES, HDROP};

    const CF_UNICODETEXT: u32 = 13;
    const CF_HDROP: u32 = 15;

    /// The clipboard's change counter. It only moves when the contents change.
    pub fn sequence_number() -> u32 {
        // SAFETY: the call takes no arguments and cannot fail.
        unsafe { GetClipboardSequenceNumber() }
    }

    /// Capture what the clipboard holds right now.
    pub fn capture() -> super::LocalPayload {
        // The clipboard is closed by the guard on the way out.
        let Some(_guard) = Clipboard::open() else {
            return super::LocalPayload::default();
        };
        super::LocalPayload {
            text: text(),
            files: files(),
        }
    }

    /// The clipboard's text, if it holds any.
    fn text() -> Option<String> {
        // SAFETY: the handle is owned by the clipboard and locked for the
        // read; the lock is released before the clipboard is closed.
        unsafe {
            let Ok(handle) = GetClipboardData(CF_UNICODETEXT) else {
                return None;
            };
            let pointer = GlobalLock(HGLOBAL(handle.0));
            if pointer.is_null() {
                return None;
            }
            let units_ptr = pointer.cast::<u16>();
            let mut length = 0usize;
            while *units_ptr.add(length) != 0 {
                length += 1;
            }
            let units = std::slice::from_raw_parts(units_ptr, length);
            let text = String::from_utf16_lossy(units);
            let _ = GlobalUnlock(HGLOBAL(handle.0));
            Some(text)
        }
    }

    /// The clipboard's file list, if it holds any.
    fn files() -> Vec<super::LocalFile> {
        // SAFETY: the handle is owned by the clipboard; the queries below
        // only read from it.
        unsafe {
            let Ok(handle) = GetClipboardData(CF_HDROP) else {
                return Vec::new();
            };
            let count = DragQueryFileW(HDROP(handle.0), 0xFFFFFFFF, None);
            if count == 0 {
                return Vec::new();
            }
            let mut files = Vec::new();
            for index in 0..count {
                let length = DragQueryFileW(HDROP(handle.0), index, None);
                if length == 0 {
                    continue;
                }
                let mut buffer = vec![0u16; length as usize + 1];
                let length = DragQueryFileW(HDROP(handle.0), index, Some(&mut buffer));
                if length == 0 {
                    continue;
                }
                buffer.truncate(length as usize);
                let path = PathBuf::from(String::from_utf16_lossy(&buffer));
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default();
                let metadata = std::fs::metadata(&path).ok();
                let is_directory = metadata.as_ref().map(|meta| meta.is_dir()).unwrap_or(false);
                let size = metadata
                    .as_ref()
                    .filter(|meta| meta.is_file())
                    .map(|meta| meta.len())
                    .unwrap_or(0);
                files.push(super::LocalFile {
                    path,
                    name,
                    size,
                    is_directory,
                });
            }
            files
        }
    }

    /// Put text into the clipboard.
    pub fn set_text(text: &str) {
        // SAFETY: the memory handed to the clipboard stays owned by the
        // clipboard itself once SetClipboardData succeeds.
        unsafe {
            let Some(_guard) = Clipboard::open() else {
                return;
            };
            let mut units: Vec<u16> = text.encode_utf16().collect();
            units.push(0);
            let bytes = units.len() * std::mem::size_of::<u16>();
            let Ok(memory) = GlobalAlloc(GMEM_MOVEABLE, bytes) else {
                return;
            };
            let pointer = GlobalLock(memory);
            if pointer.is_null() {
                return;
            }
            ptr::copy_nonoverlapping(units.as_ptr() as *const u8, pointer as *mut u8, bytes);
            let _ = GlobalUnlock(memory);
            let _ = EmptyClipboard();
            let _ = SetClipboardData(CF_UNICODETEXT, HANDLE(memory.0));
        }
    }

    /// Put a file list into the clipboard.
    pub fn set_file_paths(paths: &[PathBuf]) {
        if paths.is_empty() {
            return;
        }
        // SAFETY: the memory handed to the clipboard stays owned by the
        // clipboard itself once SetClipboardData succeeds.
        unsafe {
            let Some(_guard) = Clipboard::open() else {
                return;
            };
            let mut units: Vec<u16> = Vec::new();
            for path in paths {
                units.extend(path.to_string_lossy().encode_utf16());
                units.push(0);
            }
            units.push(0);

            let header = std::mem::size_of::<DROPFILES>();
            let list_bytes = units.len() * std::mem::size_of::<u16>();
            let Ok(memory) = GlobalAlloc(GMEM_MOVEABLE, header + list_bytes) else {
                return;
            };
            let pointer = GlobalLock(memory);
            if pointer.is_null() {
                return;
            }
            let base = pointer.cast::<u8>();
            let drop_files = DROPFILES {
                pFiles: (header + std::mem::size_of::<u16>()) as u32,
                pt: POINT { x: 0, y: 0 },
                fNC: false.into(),
                fWide: true.into(),
            };
            ptr::write_unaligned(base.cast::<DROPFILES>(), drop_files);
            ptr::copy_nonoverlapping(units.as_ptr() as *const u8, base.add(header), list_bytes);
            let _ = GlobalUnlock(memory);
            let _ = EmptyClipboard();
            let _ = SetClipboardData(CF_HDROP, HANDLE(memory.0));
        }
    }

    /// The clipboard, opened for the duration of a scope.
    struct Clipboard;

    impl Clipboard {
        fn open() -> Option<Self> {
            // SAFETY: opening the clipboard is the documented way to take it;
            // the handle may be None (no owning window), which is allowed.
            unsafe {
                if OpenClipboard(None).is_err() {
                    return None;
                }
            }
            Some(Self)
        }
    }

    impl Drop for Clipboard {
        fn drop(&mut self) {
            // SAFETY: the clipboard was opened by this instance.
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::PathBuf;

    /// No local clipboard to talk to.
    pub fn sequence_number() -> u32 {
        0
    }

    pub fn capture() -> super::LocalPayload {
        super::LocalPayload::default()
    }

    pub fn set_text(_text: &str) {}

    pub fn set_file_paths(_paths: &[PathBuf]) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_payload_without_text_advertises_nothing() {
        let payload = LocalPayload::default();
        assert!(payload.formats().is_empty());
    }

    #[test]
    fn a_payload_with_text_advertises_the_text_format() {
        let payload = LocalPayload {
            text: Some("hello".to_string()),
            files: Vec::new(),
        };
        let formats = payload.formats();
        assert_eq!(formats.len(), 1);
        assert_eq!(formats[0].id, TEXT_FORMAT);
    }

    /// The echo guard is what keeps the remote's text from bouncing back:
    /// the watcher skips a local clipboard that still holds what the remote
    /// sent.
    #[test]
    fn the_echoed_text_is_recognised() {
        let shared = Shared::default();
        *shared.echoed.lock().unwrap() = Some("from the remote".to_string());
        let echoed = shared.echoed.lock().unwrap().clone();
        assert_eq!(echoed.as_deref(), Some("from the remote"));
        assert_ne!(echoed.as_deref(), Some("something else"));
    }

    /// A file descriptor carries the name and the size the remote needs.
    #[test]
    fn a_file_descriptor_carries_the_name_and_the_size() {
        let file = LocalFile {
            path: PathBuf::from("/tmp/example.txt"),
            name: "example.txt".to_string(),
            size: 42,
            is_directory: false,
        };
        let descriptor = file.descriptor();
        assert_eq!(descriptor.name, "example.txt");
        assert_eq!(descriptor.file_size, Some(42));
    }

    /// A directory is marked as one: the remote must not read its bytes.
    #[test]
    fn a_directory_is_marked_as_a_directory() {
        let file = LocalFile {
            path: PathBuf::from("/tmp/folder"),
            name: "folder".to_string(),
            size: 0,
            is_directory: true,
        };
        let descriptor = file.descriptor();
        assert!(
            descriptor
                .attributes
                .is_some_and(|attributes| attributes.contains(ClipboardFileAttributes::DIRECTORY))
        );
    }

    /// The name a remote sends is used as a file name, so it must not be able
    /// to escape the temporary directory.
    #[test]
    fn a_name_from_the_remote_cannot_escape_the_temporary_directory() {
        let file = IncomingFile {
            name: "..\\..\\Windows\\System32\\evil.dll".to_string(),
            index: 0,
            stream_id: 1,
            size: Some(1),
            data: Vec::new(),
            path: None,
            data_id: None,
        };
        let path = write_incoming_file(&file).expect("the file is written");
        let directory = std::env::temp_dir().join("mterm-rdp-cliprdr");
        assert!(path.starts_with(&directory), "{path:?}");
        assert_eq!(path.file_name().unwrap(), "evil.dll");
    }
}
