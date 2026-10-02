"""列出 Windows PE 可执行文件的 DLL 导入依赖（纯标准库，无需 objdump/Dependencies）。

用途：打包前确认 exe 是否自足。mingw-w64 工具链（x86_64-pc-windows-gnu）在未开
`+crt-static` 时会动态依赖 libgcc_s_seh-1.dll / libwinpthread-1.dll 等 —— 这类
DLL 不在系统里，分发时必须随包附带，否则目标机直接起不来。

用法:  python scripts/pe-deps.py target/release/meatshell.exe
"""
import struct
import sys

# 系统自带、无需随包分发的 DLL 前缀（大小写不敏感）
SYSTEM_PREFIXES = ("api-ms-win-", "ext-ms-win-")
SYSTEM_DLLS = {
    "kernel32.dll", "user32.dll", "gdi32.dll", "advapi32.dll", "shell32.dll",
    "ole32.dll", "oleaut32.dll", "shlwapi.dll", "comdlg32.dll", "comctl32.dll",
    "ws2_32.dll", "winmm.dll", "imm32.dll", "uxtheme.dll", "dwmapi.dll",
    "bcrypt.dll", "crypt32.dll", "userenv.dll", "netapi32.dll", "version.dll",
    "dnsapi.dll", "iphlpapi.dll", "secur32.dll", "setupapi.dll", "mswsock.dll",
    "ntdll.dll", "powrprof.dll", "propsys.dll", "rpcrt4.dll", "authz.dll",
    "msvcrt.dll", "ucrtbase.dll", "vcruntime140.dll", "dbghelp.dll",
    "opengl32.dll", "wtsapi32.dll", "psapi.dll", "winspool.drv",
    # Win8+ 起随系统提供，常被误判成第三方（实测踩过）：
    "combase.dll", "bcryptprimitives.dll", "dwrite.dll", "pdh.dll",
    "uiautomationcore.dll", "dcomp.dll", "wlanapi.dll", "ncrypt.dll",
    "cfgmgr32.dll", "win32u.dll", "gdi32full.dll", "msvcp_win.dll",
}


def read_imports(path):
    with open(path, "rb") as f:
        data = f.read()

    if data[:2] != b"MZ":
        raise ValueError("不是 PE 文件（缺少 MZ 头）")
    e_lfanew = struct.unpack_from("<I", data, 0x3C)[0]
    if data[e_lfanew:e_lfanew + 4] != b"PE\0\0":
        raise ValueError("不是 PE 文件（缺少 PE 签名）")

    coff = e_lfanew + 4
    num_sections = struct.unpack_from("<H", data, coff + 2)[0]
    opt_size = struct.unpack_from("<H", data, coff + 16)[0]
    opt = coff + 20
    magic = struct.unpack_from("<H", data, opt)[0]
    is_pe32_plus = magic == 0x20B

    # DataDirectory[1] = Import Table；PE32 偏移 96，PE32+ 偏移 112
    dd = opt + (112 if is_pe32_plus else 96)
    import_rva, import_size = struct.unpack_from("<II", data, dd + 8)

    sections = []
    sec = opt + opt_size
    for i in range(num_sections):
        off = sec + i * 40
        va, vsize = struct.unpack_from("<II", data, off + 12)
        raw_size, raw_ptr = struct.unpack_from("<II", data, off + 16)
        sections.append((va, max(vsize, raw_size), raw_ptr))

    def rva_to_off(rva):
        for va, size, ptr in sections:
            if va <= rva < va + size:
                return ptr + (rva - va)
        return None

    def cstr(off):
        end = data.index(b"\0", off)
        return data[off:end].decode("ascii", "replace")

    if not import_rva:
        return []

    names = []
    off = rva_to_off(import_rva)
    while True:
        desc = struct.unpack_from("<IIIII", data, off)
        if not any(desc):
            break
        name_rva = desc[3]
        if name_rva:
            no = rva_to_off(name_rva)
            if no is not None:
                names.append(cstr(no))
        off += 20
    return names


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 1

    for path in sys.argv[1:]:
        print("== %s ==" % path)
        try:
            dlls = read_imports(path)
        except Exception as e:
            print("  !! 解析失败:", e)
            continue
        external = []
        for d in sorted(set(dlls), key=str.lower):
            low = d.lower()
            if low in SYSTEM_DLLS or low.startswith(SYSTEM_PREFIXES):
                print("  [系统]   %s" % d)
            else:
                print("  [需随包] %s" % d)
                external.append(d)
        print("  共 %d 个导入 DLL；其中需要随包分发 %d 个"
              % (len(set(dlls)), len(external)))
        if not external:
            print("  → 自足，可单文件分发")
        else:
            print("  → 必须附带: %s" % ", ".join(external))
    return 0


if __name__ == "__main__":
    sys.exit(main())
