// test_routa_patches.c - Routa's patches to ProxyBridge (see ../UPSTREAM.md):
//   configurable relay ports (ProxyBridge_SetRelayPorts), Unicode case-insensitive process path compare,
//   and the loopback helpers behind "BLOCK never drops loopback".
// Built with every file in ../src by scripts/test-proxybridge.ps1; the WinDivert driver is never loaded.
#include "pb_internal.h"
#include "pbtest.h"

static UINT32 ip4(const char* s) { struct in_addr a; inet_pton(AF_INET, s, &a); return a.S_un.S_addr; }
static BOOL lb6(const char* s) { UINT8 a[16]; inet_pton(AF_INET6, s, a); return pb_is_loopback_v6(a); }

int main(void)
{
    /* ================= relay ports ================= */
    CHECK(g_local_relay_port == LOCAL_PROXY_PORT && g_local_udp_relay_port == LOCAL_UDP_RELAY_PORT);   /* defaults unchanged */
    CHECK(!ProxyBridge_SetRelayPorts(0, 34011));
    CHECK(!ProxyBridge_SetRelayPorts(34010, 0));
    CHECK(!ProxyBridge_SetRelayPorts(40000, 40000));
    CHECK(g_local_relay_port == LOCAL_PROXY_PORT && g_local_udp_relay_port == LOCAL_UDP_RELAY_PORT);   /* rejected: nothing changes */
    CHECK(ProxyBridge_SetRelayPorts(40000, 40001));
    CHECK(g_local_relay_port == 40000 && g_local_udp_relay_port == 40001);
    CHECK(ProxyBridge_SetRelayPorts(LOCAL_PROXY_PORT, LOCAL_UDP_RELAY_PORT));

    /* ================= process path: Unicode case-insensitive ================= */
    /* UTF-8: C:/<two CJK chars>/<A-umlaut>.exe vs c:/<same CJK>/<a-umlaut>.EXE (backslashes in the literals) */
    CHECK(match_process_pattern("C:\\\xE6\xB5\x8B\xE8\xAF\x95\\\xC3\x84.exe",
                                "c:\\\xE6\xB5\x8B\xE8\xAF\x95\\\xC3\xA4.EXE"));
    CHECK(!match_process_pattern("C:\\a\\b.exe", "C:\\a\\c.exe"));
    CHECK(match_process_pattern("\xC3\x84.exe", "D:\\x\\\xC3\xA4.EXE"));                     /* file-name-only pattern */
    CHECK(!match_process_pattern("\xC3\x84.exe", "D:\\x\\\xC3\xB6.exe"));                    /* A-umlaut != o-umlaut */
    CHECK(match_process_pattern("CHROME.EXE", "C:\\Program Files\\chrome.exe"));             /* ASCII behaviour unchanged */
    CHECK(!match_process_pattern("chrome.exe", "C:\\Program Files\\chrome.exe.bak"));
    CHECK(match_process_pattern("*", "C:\\any.exe") && match_process_pattern("chr*.exe", "C:\\x\\chrome.exe"));

    /* ================= loopback detection (BLOCK -> DIRECT) ================= */
    CHECK(pb_is_loopback_v4(ip4("127.0.0.1")) && pb_is_loopback_v4(ip4("127.255.255.254")) && pb_is_loopback_v4(ip4("127.1.2.3")));
    CHECK(!pb_is_loopback_v4(ip4("128.0.0.1")) && !pb_is_loopback_v4(ip4("10.0.0.127")) && !pb_is_loopback_v4(ip4("0.0.0.0")) &&
          !pb_is_loopback_v4(ip4("1.0.0.127")));                                                /* only the first octet counts */
    CHECK(lb6("::1") && lb6("::ffff:127.0.0.1") && lb6("::ffff:127.9.9.9"));
    CHECK(!lb6("::") && !lb6("::2") && !lb6("::ffff:10.0.0.1") && !lb6("2001:db8::1") && !lb6("::127.0.0.1"));

    return pbtest_done("test_routa_patches");
}
