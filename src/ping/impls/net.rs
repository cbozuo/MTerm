//! Ping 的网络探测实现：Win32 ICMP（无需管理员）+ TCP connect 兜底。

use std::net::IpAddr;

// ── ICMP（Windows IcmpSendEcho，无需管理员）─────────────────────────────
#[cfg(windows)]
pub(crate) mod icmp {
    // 手写 extern（#titlebar-ping）：不依赖 windows crate 的 feature 组合，
    // 直接链接系统库 iphlpapi.dll。ICMP_ECHO_REPLY 只读前 12 字节（Address/
    // Status/RoundTripTime），布局由 Win32 ABI 保证。
    #[link(name = "iphlpapi")]
    extern "system" {
        fn IcmpCreateFile() -> *mut std::ffi::c_void;
        fn IcmpCloseHandle(handle: *mut std::ffi::c_void) -> u32;
        fn IcmpSendEcho(
            handle: *mut std::ffi::c_void,
            dest: u32,
            request_data: *const u8,
            request_size: u16,
            request_options: *const std::ffi::c_void,
            reply_buffer: *mut u8,
            reply_size: u32,
            timeout: u32,
        ) -> u32;
    }

    pub const PAYLOAD: usize = 32;
    pub const REPLY_EXTRA: usize = 128;
    /// INVALID_HANDLE_VALUE
    const INVALID: isize = -1;

    pub struct Session { handle: *mut std::ffi::c_void }

    impl Session {
        pub fn open() -> Option<Self> {
            unsafe {
                let h = IcmpCreateFile();
                if (h as isize) == INVALID { None } else { Some(Self { handle: h }) }
            }
        }
        /// 发一包，返回 RTT(ms)；None＝超时/失败。
        pub fn echo(&self, ip: std::net::Ipv4Addr, timeout_ms: u32) -> Option<f64> {
            let dest = u32::from(ip).to_be();
            let payload = [0u8; PAYLOAD];
            // ICMP_ECHO_REPLY 前缀：Address(4) Status(4) RoundTripTime(4)
            let reply_len = 12 + PAYLOAD + REPLY_EXTRA;
            let mut buf = vec![0u8; reply_len];
            let n = unsafe {
                IcmpSendEcho(
                    self.handle,
                    dest,
                    payload.as_ptr(),
                    PAYLOAD as u16,
                    std::ptr::null(),
                    buf.as_mut_ptr(),
                    reply_len as u32,
                    timeout_ms,
                )
            };
            if n == 0 { return None; }
            let status = u32::from_le_bytes(buf[4..8].try_into().unwrap());
            // 0 = IP_STATUS_BASE success
            if status != 0 { return None; }
            let rtt = u32::from_le_bytes(buf[8..12].try_into().unwrap());
            Some(rtt as f64)
        }
    }

    impl Drop for Session {
        fn drop(&mut self) { unsafe { let _ = IcmpCloseHandle(self.handle); } }
    }
}

/// 3 包 ICMP burst；返回 (采样点[3], 是否全超时)。
#[cfg(windows)]
pub(crate) fn icmp_probe(ip: std::net::Ipv4Addr) -> (Vec<Option<f64>>, bool) {
    let Some(sess) = icmp::Session::open() else { return (vec![None; 3], true) };
    let mut out = Vec::with_capacity(3);
    for _ in 0..3 { out.push(sess.echo(ip, 2000)); }
    let all_lost = out.iter().all(|s| s.is_none());
    (out, all_lost)
}
#[cfg(not(windows))]
pub(crate) fn icmp_probe(_ip: std::net::Ipv4Addr) -> (Vec<Option<f64>>, bool) { (vec![None; 3], true) }

/// TCP connect 探测（毫秒）；IPv6 兜底路径。
pub(crate) fn tcp_probe(ip: IpAddr, timeout_ms: u64) -> Option<f64> {
    let addr = std::net::SocketAddr::new(ip, 80);
    let start = std::time::Instant::now();
    match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(timeout_ms)) {
        Ok(_) => Some(start.elapsed().as_secs_f64() * 1000.0),
        Err(_) => None,
    }
}
