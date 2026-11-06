// Routing-policy regression tests. No WinDivert driver or network proxy is started.
#include "pb_internal.h"
#include "pbtest.h"

static UINT32 ip4(const char *s) { struct in_addr a; inet_pton(AF_INET, s, &a); return a.s_addr; }
static UINT8 src6[16], dst6[16];
static UINT32 dst4;

typedef struct { UINT32 id; HANDLE started; } DELETE_REQUEST;
static DWORD WINAPI delete_config_worker(LPVOID arg)
{
    DELETE_REQUEST *request = (DELETE_REQUEST*)arg;
    SetEvent(request->started);
    return ProxyBridge_DeleteProxyConfig(request->id) ? 0 : 1;
}

static void seed_caches(UINT32 config, UINT32 rule)
{
    UINT32 selected = 0;
    match_rule("app.exe", dst4, 443, TRUE, &selected); // captures the verdict version
    add_connection(41000, TRUE, ip4("192.0.2.1"), dst4, 443, config, 123, rule);
    add_connection_v6(41000, TRUE, src6, dst6, 443, config, 123, rule);
    add_connection(41001, FALSE, ip4("192.0.2.1"), dst4, 443, config, 123, rule);
    udp_direct_remember(FALSE, &dst4, 42000, 443);
    udp_direct_remember(TRUE, dst6, 42001, 443);
    CHECK(udp_flow_validate(41000, FALSE, dst4, NULL, 443));
    CHECK(udp_flow_validate(41000, TRUE, 0, dst6, 443));
    CHECK(udp_direct_cached(FALSE, &dst4, 42000, 443));
    CHECK(udp_direct_cached(TRUE, dst6, 42001, 443));
}

static void check_invalidated(void)
{
    UINT32 ip = 0, config = 0;
    UINT16 port = 0;
    UINT8 ip6[16];
    PB_FLOW flows[4];
    CHECK(!get_connection_full(41000, TRUE, &ip, &port, &config));
    CHECK(!get_connection_full_v6(41000, TRUE, ip6, &port, &config));
    CHECK(!get_connection(41000, TRUE, &ip, &port));
    CHECK(!find_v6_udp_sender(dst6, 443, ip6, &port, 100));
    CHECK(!is_connection_tracked(41000, TRUE, FALSE));
    CHECK(!is_connection_tracked(41000, TRUE, TRUE));
    CHECK(udp_flows_snapshot(flows, 4) == 0);
    CHECK(!udp_direct_cached(FALSE, &dst4, 42000, 443));
    CHECK(!udp_direct_cached(TRUE, dst6, 42001, 443));
    CHECK(!udp_flow_validate(41000, FALSE, dst4, NULL, 443));
    CHECK(!udp_flow_validate(41000, TRUE, 0, dst6, 443));
    CHECK(get_connection_full(41001, FALSE, &ip, &port, &config)); // TCP is unaffected
}

static void check_process_missing_config(UINT32 missing)
{
    // Exercise the actual IPv4 and IPv6 entry points using this process's bound UDP sockets.
    UINT32 rule = ProxyBridge_AddRule("*", "*", "*", "", RULE_PROTOCOL_BOTH, RULE_ACTION_PROXY, missing, FALSE);
    CHECK(rule != 0);
    SOCKET s4 = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    SOCKET s6 = socket(AF_INET6, SOCK_DGRAM, IPPROTO_UDP);
    CHECK(s4 != INVALID_SOCKET && s6 != INVALID_SOCKET);
    struct sockaddr_in a4 = {0};
    a4.sin_family = AF_INET; a4.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    CHECK(bind(s4, (struct sockaddr*)&a4, sizeof(a4)) == 0);
    int len = sizeof(a4);
    CHECK(getsockname(s4, (struct sockaddr*)&a4, &len) == 0);
    DWORD pid = 0;
    UINT32 config = 0;
    CHECK(check_process_rule(a4.sin_addr.s_addr, ntohs(a4.sin_port), dst4, 443, TRUE, &pid, &config) == RULE_ACTION_BLOCK);
    CHECK(pid == GetCurrentProcessId());
    struct sockaddr_in6 a6 = {0};
    a6.sin6_family = AF_INET6; a6.sin6_addr = in6addr_loopback;
    CHECK(bind(s6, (struct sockaddr*)&a6, sizeof(a6)) == 0);
    len = sizeof(a6);
    CHECK(getsockname(s6, (struct sockaddr*)&a6, &len) == 0);
    CHECK(check_process_rule_v6((const UINT8*)&a6.sin6_addr, ntohs(a6.sin6_port), dst6, 443, TRUE, &pid, &config) == RULE_ACTION_BLOCK);
    CHECK(pid == GetCurrentProcessId());
    closesocket(s4); closesocket(s6);
    CHECK(ProxyBridge_DeleteRule(rule));
}

int main(void)
{
    WSADATA wsa;
    CHECK(WSAStartup(MAKEWORD(2, 2), &wsa) == 0);
    dst4 = ip4("198.51.100.1");
    inet_pton(AF_INET6, "2001:db8::1", src6);
    inet_pton(AF_INET6, "2001:db8::2", dst6);

    CHECK(find_proxy_config(0) == NULL);
    CHECK(checked_proxy_action(RULE_ACTION_PROXY, 0, TRUE) == RULE_ACTION_BLOCK);
    UINT32 a = ProxyBridge_AddProxyConfig(PROXY_TYPE_SOCKS5, "127.0.0.1", 1080, NULL, NULL, FALSE);
    UINT32 b = ProxyBridge_AddProxyConfig(PROXY_TYPE_SOCKS5, "127.0.0.1", 1081, NULL, NULL, FALSE);
    CHECK(a != 0 && b != 0 && a != b);
    CHECK(find_proxy_config(0)->config_id == a);
    CHECK(find_proxy_config(b)->port == 1081);
    CHECK(find_proxy_config(b + 1000) == NULL);
    CHECK(checked_proxy_action(RULE_ACTION_PROXY, b + 1000, TRUE) == RULE_ACTION_BLOCK);
    CHECK(checked_proxy_action(RULE_ACTION_PROXY, b + 1000, FALSE) == RULE_ACTION_BLOCK);
    CHECK(checked_proxy_action(RULE_ACTION_DIRECT, b + 1000, TRUE) == RULE_ACTION_DIRECT);
    CHECK(checked_proxy_action(RULE_ACTION_BLOCK, a, TRUE) == RULE_ACTION_BLOCK);
    CHECK(checked_proxy_action(RULE_ACTION_PROXY, a, TRUE) == RULE_ACTION_PROXY);
    check_process_missing_config(b + 1000);

    UINT32 rule = ProxyBridge_AddRule("app.exe", "*", "*", "", RULE_PROTOCOL_BOTH, RULE_ACTION_PROXY, a, FALSE);
    CHECK(rule != 0);
    seed_caches(a, rule);
    CHECK(ProxyBridge_EditRule(rule, "app.exe", "*", "*", "", RULE_PROTOCOL_BOTH, RULE_ACTION_BLOCK, 0, FALSE));
    check_invalidated();
    UINT32 selected = 0;
    CHECK(match_rule("app.exe", dst4, 443, TRUE, &selected) == RULE_ACTION_BLOCK);
    CHECK(match_rule_v6("app.exe", dst6, 443, TRUE, &selected) == RULE_ACTION_BLOCK);

    // A decision evaluated before an edit must remain old even when cached AFTER the edit.
    CHECK(ProxyBridge_EditRule(rule, "app.exe", "*", "*", "", RULE_PROTOCOL_BOTH, RULE_ACTION_PROXY, a, FALSE));
    CHECK(match_rule("app.exe", dst4, 443, TRUE, &selected) == RULE_ACTION_PROXY);
    CHECK(ProxyBridge_EditRule(rule, "app.exe", "*", "*", "", RULE_PROTOCOL_BOTH, RULE_ACTION_BLOCK, 0, FALSE));
    add_connection(41000, TRUE, ip4("192.0.2.1"), dst4, 443, a, 123, rule);
    add_connection_v6(41000, TRUE, src6, dst6, 443, a, 123, rule);
    udp_direct_remember(FALSE, &dst4, 42000, 443);
    udp_direct_remember(TRUE, dst6, 42001, 443);
    check_invalidated();

    // Proxy A -> B and Proxy -> Direct force a new verdict without waiting for idle timeout.
    seed_caches(a, rule);
    CHECK(ProxyBridge_EditRule(rule, "app.exe", "*", "*", "", RULE_PROTOCOL_BOTH, RULE_ACTION_PROXY, b, FALSE));
    check_invalidated();
    CHECK(match_rule("app.exe", dst4, 443, TRUE, &selected) == RULE_ACTION_PROXY && selected == b);
    seed_caches(b, rule);
    CHECK(ProxyBridge_EditRule(rule, "app.exe", "*", "*", "", RULE_PROTOCOL_BOTH, RULE_ACTION_DIRECT, 0, FALSE));
    check_invalidated();
    CHECK(match_rule("app.exe", dst4, 443, TRUE, &selected) == RULE_ACTION_DIRECT);

    seed_caches(b, rule);
    CHECK(ProxyBridge_DisableRule(rule));
    check_invalidated();
    seed_caches(b, rule);
    CHECK(ProxyBridge_EnableRule(rule));
    check_invalidated();
    seed_caches(b, rule);
    UINT32 other = ProxyBridge_AddRule("other.exe", "*", "*", "", RULE_PROTOCOL_BOTH, RULE_ACTION_BLOCK, 0, FALSE);
    CHECK(other != 0);
    check_invalidated();
    seed_caches(b, rule);
    CHECK(ProxyBridge_MoveRuleToPosition(other, 1));
    check_invalidated();
    seed_caches(b, rule);
    CHECK(ProxyBridge_DeleteRule(other));
    check_invalidated();

    seed_caches(a, rule);
    CHECK(ProxyBridge_EditProxyConfig(a, PROXY_TYPE_SOCKS5, "127.0.0.1", 1082, NULL, NULL, FALSE));
    check_invalidated();
    seed_caches(a, rule);
    // Deletion cannot compact the config array while a relay reader still uses it.
    PROXY_CONFIG snapshot;
    CHECK(copy_proxy_config(a, &snapshot));
    HANDLE started = CreateEvent(NULL, TRUE, FALSE, NULL);
    CHECK(started != NULL);
    DELETE_REQUEST request = {a, started};
    AcquireSRWLockShared(&g_proxy_config_lock);
    HANDLE worker = CreateThread(NULL, 0, delete_config_worker, &request, 0, NULL);
    CHECK(worker != NULL);
    CHECK(WaitForSingleObject(started, 5000) == WAIT_OBJECT_0);
    CHECK(WaitForSingleObject(worker, 50) == WAIT_TIMEOUT);
    ReleaseSRWLockShared(&g_proxy_config_lock);
    CHECK(WaitForSingleObject(worker, 5000) == WAIT_OBJECT_0);
    DWORD exit_code = 1;
    CHECK(GetExitCodeThread(worker, &exit_code) && exit_code == 0);
    CloseHandle(worker); CloseHandle(started);
    check_invalidated();
    CHECK(snapshot.config_id == a && snapshot.port == 1082); // TCP's value copy cannot become B
    CHECK(!copy_proxy_config(a, &snapshot));
    CHECK(find_proxy_config(a) == NULL); // B survives; A must never fall back to it
    CHECK(find_proxy_config(0)->config_id == b);
    CHECK(checked_proxy_action(RULE_ACTION_PROXY, a, TRUE) == RULE_ACTION_BLOCK);
    // Packet-path config reads must fail closed, not wait behind a writer.
    AcquireSRWLockExclusive(&g_proxy_config_lock);
    CHECK(!copy_proxy_config(b, &snapshot));
    CHECK(checked_proxy_action(RULE_ACTION_PROXY, b, TRUE) == RULE_ACTION_BLOCK);
    ReleaseSRWLockExclusive(&g_proxy_config_lock);
    seed_caches(b, rule);
    UINT32 c = ProxyBridge_AddProxyConfig(PROXY_TYPE_SOCKS5, "127.0.0.1", 1083, NULL, NULL, FALSE);
    CHECK(c != 0);
    check_invalidated();

    CHECK(ProxyBridge_DeleteRule(rule));
    CHECK(ProxyBridge_DeleteProxyConfig(b));
    CHECK(ProxyBridge_DeleteProxyConfig(c));
    remove_connection(41000, TRUE, FALSE);
    remove_connection(41000, TRUE, TRUE);
    remove_connection(41001, FALSE, FALSE);
    WSACleanup();
    return pbtest_done("test_udp_policy");
}
