//! 端口模式：本机监听端口快照（GetExtendedTcpTable + 进程名）。

use super::model::ListenEntry;

/// 本机 IPv4 监听端口快照（按端口升序、同端口去重）。
#[cfg(windows)]
pub(crate) fn listen_table() -> Vec<ListenEntry> {
    // 手写 extern + 结构（同 net.rs 的 icmp：不依赖 windows crate 的 feature 组合）。
    // ⚠️ TableClass 必须与行结构配对：OWNER_MODULE_LISTENER=6 配 160 字节行；
    // 这里只需 port/addr/pid → 用 OWNER_PID_LISTENER=3 + 24 字节 MIB_TCPROW_OWNER_PID，
    // stride 一致（此前 class=3 却按 MODULE 布局读，行错位 → 状态字节全是垃圾 →
    // LISTEN 过滤后 0 行，端口表永远为空）。
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MibTcpRowOwnerPid {
        state: u32,
        local_addr: u32,
        local_port: u32,   // 网络序（低 16 位有效）
        remote_addr: u32,
        remote_port: u32,
        pid: u32,
    }
    #[repr(C)]
    struct MibTcpTableOwnerPid {
        num_entries: u32,
        table: [MibTcpRowOwnerPid; 1],
    }
    #[link(name = "iphlpapi")]
    extern "system" {
        fn GetExtendedTcpTable(
            table: *mut MibTcpTableOwnerPid,
            size: *mut u32,
            order: i32,
            af: u32,
            class: u32,
            reserved: u32,
        ) -> u32;
    }
    const TCP_TABLE_OWNER_PID_LISTENER: u32 = 3;
    const AF_INET_V: u32 = 2;
    const MIB_TCP_STATE_LISTEN: u32 = 2;
    let mut out = Vec::new();
    let mut entries: Vec<(u16, String, u32)> = Vec::new(); // (port, addr, pid)
    unsafe {
        let mut size: u32 = 0;
        let _ = GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET_V, TCP_TABLE_OWNER_PID_LISTENER, 0);
        if size == 0 { return out; }
        let mut buf = vec![0u8; size as usize];
        let rc = GetExtendedTcpTable(buf.as_mut_ptr() as *mut MibTcpTableOwnerPid, &mut size, 0, AF_INET_V, TCP_TABLE_OWNER_PID_LISTENER, 0);
        if rc != 0 { return out; }
        let tbl = &*(buf.as_ptr() as *const MibTcpTableOwnerPid);
        let n = tbl.num_entries as usize;
        let rows = std::slice::from_raw_parts(tbl.table.as_ptr(), n);
        for row in rows {
            if row.state != MIB_TCP_STATE_LISTEN { continue; }
            // dwLocalPort 是网络序（htons），低 16 位
            let port = u16::from_be((row.local_port & 0xFFFF) as u16);
            let a = (row.local_addr >> 24) as u8;
            let b = (row.local_addr >> 16) as u8;
            let c = (row.local_addr >> 8) as u8;
            let d = row.local_addr as u8;
            entries.push((port, format!("{}.{}.{}.{}:{}", a, b, c, d, port), row.pid));
        }
    }
    // 进程名：System::new_all() 一次快照（与 resource_ui 同源）。按需
    // refresh_processes(Some) 在 Windows 普通权限下查不到名字（全变 System）。
    let sys = sysinfo::System::new_all();
    for (port, addr, pid) in entries {
        let name = sys.process(sysinfo::Pid::from_u32(pid))
            .map(|pr| pr.name().to_string_lossy().to_string())
            .unwrap_or_else(|| "System".into());
        out.push(ListenEntry { port, name, pid, addr });
    }
    out.sort_by_key(|r| r.port);
    out.dedup_by(|a, b| a.port == b.port && a.name == b.name);
    out
}

#[cfg(not(windows))]
pub(crate) fn listen_table() -> Vec<ListenEntry> { Vec::new() }
