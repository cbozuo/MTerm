//! 本机网卡 IPv4 枚举（清单头 ＋ 的「本机地址」下拉数据源）。

/// 本机 IPv4 地址（排除 loopback；addr 为点分十进制，nic 为接口显示名）。
pub(crate) fn local_ipv4_list() -> Vec<(String, String)> {
    #[cfg(windows)]
    {
        // GetAdaptersAddresses（GAA_FLAG_SKIP_ANYCAST | SKIP_MULTICAST，只要
        // Unicast IPv4）。手写 extern 与 net.rs/listen.rs 同策略（不引 windows crate）。
        use std::mem;
        // sockaddr 只按 opaque 字节读：family(u16) + port(u16) + addr(u32 BE)。
        #[repr(C)]
        struct SockaddrOpaque {
            family: u16,
            _pad: [u8; 126],       // sockaddr_storage 足量；IPv4 时 _pad[0..4] = sin_addr
        }
        // IP_ADAPTER_UNICAST_ADDRESS（64 位布局）：Length/Flags/Next/...
        // 我们只读 Address(sockaddr*) 与其后的字段无关项——手工前缀。
        #[repr(C)]
        struct IpAdapterUnicastAddress {
            length: u32,
            flags: u32,
            next: *mut IpAdapterUnicastAddress,
            address: *mut SockaddrOpaque, // SOCKET_ADDRESS.Addr（偏移 16 处的指针字段之一）
            _rest: [u8; 40],
        }
        // 完整解析不做：SocketAddress 里 family + IPv4 时前 2+2+4 字节可从
        // SockaddrOpaque.family 与 _pad[0..4] 取（sin_port 2 字节 + sin_addr 4 字节）。
        #[link(name = "iphlpapi")]
        extern "system" {
            fn GetAdaptersAddresses(
                family: u32,
                flags: u32,
                reserved: *mut std::ffi::c_void,
                adapter_addresses: *mut u8,
                size_pointer: *mut u32,
            ) -> u32;
        }
        const AF_INET: u32 = 2;
        const GAA_FLAG_SKIP_ANYCAST: u32 = 0x2;
        const GAA_FLAG_SKIP_MULTICAST: u32 = 0x4;
        const ERROR_BUFFER_OVERFLOW: u32 = 111;

        let mut out: Vec<(String, String)> = Vec::new();
        unsafe {
            let mut size: u32 = 15 * 1024;
            let mut buf = vec![0u8; size as usize];
            let mut rc = GetAdaptersAddresses(
                AF_INET, GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST,
                std::ptr::null_mut(), buf.as_mut_ptr(), &mut size);
            if rc == ERROR_BUFFER_OVERFLOW {
                buf = vec![0u8; size as usize];
                rc = GetAdaptersAddresses(
                    AF_INET, GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST,
                    std::ptr::null_mut(), buf.as_mut_ptr(), &mut size);
            }
            if rc != 0 { return out; }
            // IP_ADAPTER_INFO 布局复杂且 Win SDK 结构体有 union，按保守偏移读：
            // 直接解析官方布局太脆 —— 退而用 std::net 不可行（无枚举 API），
            // 这里用与 resource 无关的轻量方案：按 8 字节对齐的头部字段
            // （Length(4) IfIndex(4) Next(8) AdapterName(8) FirstUnicastAddress(8)），
            // 由 FirstUnicastAddress 链表逐个取 IPv4。
            #[repr(C)]
            struct IpAdapterAddressesHead {
                length: u32,
                if_index: u32,
                next: *mut u8,
                adapter_name: *const u8,
                first_unicast: *mut IpAdapterUnicastAddress,
                // 其余字段不再读
            }
            let mut cur = buf.as_mut_ptr() as *mut IpAdapterAddressesHead;
            while !cur.is_null() && (*cur).length >= mem::size_of::<IpAdapterAddressesHead>() as u32 {
                let mut ua = (*cur).first_unicast;
                let mut seen = 0;
                while !ua.is_null() && seen < 16 {
                    seen += 1;
                    let sa = (*ua).address;
                    if !sa.is_null() && (*sa).family == AF_INET as u16 {
                        // sockaddr_in：family(2) + sin_port(2) + sin_addr(4)。
                        // _pad[0..2] 是端口（适配器地址恒 0），地址在 _pad[2..6]
                        // ——曾误读 [0..4]，网络字节序左移两字节 → 显示
                        // 0.0.169.254（2026-11-04 用户截图）。
                        let be = u32::from_be_bytes(
                            [(*sa)._pad[2], (*sa)._pad[3], (*sa)._pad[4], (*sa)._pad[5]]);
                        let ip = format!("{}.{}.{}.{}", be >> 24 & 0xFF, be >> 16 & 0xFF, be >> 8 & 0xFF, be & 0xFF);
                        if ip != "127.0.0.1" && !out.iter().any(|(a, _)| a == &ip) {
                            out.push((ip, String::new()));
                        }
                    }
                    ua = (*ua).next;
                }
                cur = (*cur).next as *mut IpAdapterAddressesHead;
            }
        }
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}
