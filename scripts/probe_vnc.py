"""Minimal, read-only RFB prober for diagnosing a VNC handshake failure.

Sends only the client version banner and then *reads* what the server offers.
It never picks a security type and never sends credentials, so it cannot lock
an account or disturb a live session — it only answers one question: does the
server offer any security type at all, or does it refuse the connection
outright (RFB 3.8: zero security types + a reason string)?

Usage: python probe_vnc.py <host> [port]
"""

import socket
import sys


def recv_exact(sock: socket.socket, count: int, timeout: float = 5.0):
    """Read exactly `count` bytes; return (data, status) with status one of
    ok / timeout / closed. A short read is reported, never silently padded."""
    sock.settimeout(timeout)
    buf = b""
    while len(buf) < count:
        try:
            chunk = sock.recv(count - len(buf))
        except socket.timeout:
            return buf, "timeout"
        except OSError as err:
            return buf, f"error: {err}"
        if not chunk:
            return buf, "closed"
        buf += chunk
    return buf, "ok"


def main() -> int:
    host = sys.argv[1] if len(sys.argv) > 1 else "192.168.0.69"
    port = int(sys.argv[2]) if len(sys.argv) > 2 else 5900

    try:
        sock = socket.create_connection((host, port), timeout=5.0)
    except OSError as err:
        print(f"TCP connect to {host}:{port} failed: {err}")
        return 2
    print(f"TCP connected to {host}:{port}")

    version, why = recv_exact(sock, 12)
    print(f"server banner: {version!r} ({why})")
    if not version.startswith(b"RFB "):
        print("not an RFB server (or it closed before the banner)")
        sock.close()
        return 3

    # Reply with a version the server can meet us at.
    if version >= b"RFB 003.008":
        mine = b"RFB 003.008\n"
    elif version >= b"RFB 003.007":
        mine = b"RFB 003.007\n"
    else:
        mine = b"RFB 003.003\n"
    sock.sendall(mine)
    print(f"client banner sent: {mine!r}")

    if mine in (b"RFB 003.008\n", b"RFB 003.007\n"):
        head, why = recv_exact(sock, 1)
        print(f"security-type count byte: {head!r} ({why})")
        if not head:
            print("server closed before offering security types")
            sock.close()
            return 4
        count = head[0]
        if count == 0:
            length_raw, why = recv_exact(sock, 4)
            if len(length_raw) == 4:
                length = int.from_bytes(length_raw, "big")
                reason, why2 = recv_exact(sock, min(length, 4096))
                print(f"server REFUSED the connection: {reason!r} ({why2})")
            else:
                print(f"refusal reason unreadable ({why})")
            sock.close()
            return 5
        types, why = recv_exact(sock, count)
        print(f"security types offered: {list(types)} ({why})")
        names = {0: "invalid", 1: "None", 2: "VNC Auth", 16: "Tight",
                 18: "TLS", 19: "VeNCrypt", 20: "SASL", 30: "TightUnix",
                 35: "Apple ARD"}
        print("  ->", ", ".join(names.get(t, str(t)) for t in types))
        print("no security type chosen; closing (read-only probe)")
    else:
        sec, why = recv_exact(sock, 4)
        print(f"RFB 3.3 security type word: {sec!r} ({why})")

    sock.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
