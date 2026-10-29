// Real local SOCKS5 + UDP sockets. No WinDivert driver or external network is used.
#include "pb_internal.h"
#include "pbtest.h"

#define MOCK_MAX 192
typedef struct {
    SOCKET control, udp;
    struct sockaddr_in client;
    char packet[256];
    int length;
} MOCK_CHANNEL;
typedef struct {
    SOCKET listener, upstream;
    HANDLE thread;
    volatile LONG stop, hold, received, count, failed, disconnect, authenticate, fragment, reject_auth;
    MOCK_CHANNEL channels[MOCK_MAX];
} MOCK;

static BOOL read_exact(SOCKET s, char *buf, int n)
{
    while (n) { int got = recv(s, buf, n, 0); if (got <= 0) return FALSE; buf += got; n -= got; }
    return TRUE;
}

static BOOL mock_write(MOCK *m, SOCKET s, const char *data, int len)
{
    if (!InterlockedCompareExchange(&m->fragment, 0, 0)) return send(s, data, len, 0) == len;
    for (int i = 0; i < len; i++) {
        if (send(s, data + i, 1, 0) != 1) return FALSE;
        Sleep(2);
    }
    return TRUE;
}

static BOOL mock_accept(MOCK *m)
{
    SOCKET s = accept(m->listener, NULL, NULL);
    if (s == INVALID_SOCKET) return FALSE;
    DWORD timeout = 2000;
    setsockopt(s, SOL_SOCKET, SO_RCVTIMEO, (char*)&timeout, sizeof(timeout));
    char buf[256];
    if (!read_exact(s, buf, 2) || (BYTE)buf[1] == 0 || !read_exact(s, buf, (BYTE)buf[1])) goto fail;
    if (InterlockedCompareExchange(&m->authenticate, 0, 0)) {
        if (!mock_write(m, s, "\x05\x02", 2)) goto fail;
        if (!read_exact(s, buf, 2) || buf[0] != 1 || (BYTE)buf[1] != 4 ||
            !read_exact(s, buf, 4) || memcmp(buf, "user", 4) ||
            !read_exact(s, buf, 1) || (BYTE)buf[0] != 4 ||
            !read_exact(s, buf, 4) || memcmp(buf, "pass", 4)) goto fail;
        if (InterlockedCompareExchange(&m->reject_auth, 0, 0)) {
            mock_write(m, s, "\x01\x01", 2); closesocket(s); return TRUE;
        }
        if (!mock_write(m, s, "\x01\x00", 2)) goto fail;
    } else if (!mock_write(m, s, "\x05\x00", 2)) goto fail;
    if (!read_exact(s, buf, 10) || buf[0] != 5 || buf[1] != 3 || buf[3] != 1) goto fail;
    int slot = (int)InterlockedCompareExchange(&m->count, 0, 0);
    if (slot >= MOCK_MAX) goto fail;
    // A real SOCKS server may return ONE UDP endpoint for every TCP association.
    // Replies are then separated only by each association's client-side UDP socket.
    SOCKET u = m->upstream;
    struct sockaddr_in addr = {0};
    int len = sizeof(addr);
    getsockname(u, (struct sockaddr*)&addr, &len);
    char reply[10] = {5,0,0,1,127,0,0,1,0,0};
    memcpy(reply + 8, &addr.sin_port, 2);
    if (!mock_write(m, s, reply, 10)) goto fail;
    m->channels[slot].control = s; m->channels[slot].udp = u;
    InterlockedIncrement(&m->count);
    return TRUE;
fail:
    closesocket(s); return FALSE;
}

static DWORD WINAPI mock_worker(LPVOID arg)
{
    MOCK *m = (MOCK*)arg;
    while (!InterlockedCompareExchange(&m->stop, 0, 0)) {
        if (InterlockedExchange(&m->disconnect, 0)) {
            for (int i = 0; i < m->count; i++) {
                if (m->channels[i].control != INVALID_SOCKET) {
                    closesocket(m->channels[i].control); m->channels[i].control = INVALID_SOCKET;
                }
            }
        }
        WSAPOLLFD fds[2];
        ZeroMemory(fds, sizeof(fds));
        fds[0].fd = m->listener; fds[0].events = POLLRDNORM;
        fds[1].fd = m->upstream; fds[1].events = POLLRDNORM;
        if (WSAPoll(fds, 2, 10) > 0) {
            if (fds[0].revents & POLLRDNORM) if (!mock_accept(m)) InterlockedIncrement(&m->failed);
            if (fds[1].revents & POLLRDNORM) {
                for (int i = 0; i < MOCK_MAX; i++) if (!m->channels[i].length) {
                    MOCK_CHANNEL *c = &m->channels[i]; int len = sizeof(c->client);
                    int got = recvfrom(m->upstream, c->packet, sizeof(c->packet), 0, (struct sockaddr*)&c->client, &len);
                    if (got > 0) { c->length = got; InterlockedIncrement(&m->received); }
                    break;
                }
            }
        }
        if (!InterlockedCompareExchange(&m->hold, 0, 0)) {
            // Reverse association order deliberately: arrival time must not choose the client.
            for (int i = MOCK_MAX - 1; i >= 0; i--) {
                MOCK_CHANNEL *c = &m->channels[i];
                if (c->length) {
                    sendto(m->upstream, c->packet, c->length, 0, (struct sockaddr*)&c->client, sizeof(c->client));
                    c->length = 0;
                }
            }
        }
    }
    for (int i = 0; i < m->count; i++) {
        if (m->channels[i].control != INVALID_SOCKET) closesocket(m->channels[i].control);
    }
    closesocket(m->listener); closesocket(m->upstream);
    return 0;
}

static UINT16 mock_start(MOCK *m)
{
    ZeroMemory(m, sizeof(*m));
    m->upstream = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    struct sockaddr_in udp = {0}; udp.sin_family = AF_INET; udp.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    CHECK(bind(m->upstream, (struct sockaddr*)&udp, sizeof(udp)) == 0);
    configure_udp_socket(m->upstream, 262144, 0);
    m->listener = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    struct sockaddr_in addr = {0}; addr.sin_family = AF_INET; addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    CHECK(bind(m->listener, (struct sockaddr*)&addr, sizeof(addr)) == 0);
    CHECK(listen(m->listener, 16) == 0);
    int len = sizeof(addr); getsockname(m->listener, (struct sockaddr*)&addr, &len);
    m->thread = CreateThread(NULL, 0, mock_worker, m, 0, NULL);
    CHECK(m->thread != NULL);
    return ntohs(addr.sin_port);
}

static BOOL wait_received(MOCK *m, LONG count)
{
    ULONGLONG deadline = GetTickCount64() + 15000;
    while (InterlockedCompareExchange(&m->received, 0, 0) < count && GetTickCount64() < deadline) Sleep(10);
    return InterlockedCompareExchange(&m->received, 0, 0) >= count;
}

static SOCKET client_open(BOOL ipv6, UINT16 *port)
{
    SOCKET s = socket(ipv6 ? AF_INET6 : AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    struct sockaddr_storage ss = {0}; int len;
    if (ipv6) {
        struct sockaddr_in6 *a = (struct sockaddr_in6*)&ss;
        a->sin6_family = AF_INET6; a->sin6_addr = in6addr_loopback; len = sizeof(*a);
    } else {
        struct sockaddr_in *a = (struct sockaddr_in*)&ss;
        a->sin_family = AF_INET; a->sin_addr.s_addr = htonl(INADDR_LOOPBACK); len = sizeof(*a);
    }
    CHECK(s != INVALID_SOCKET && bind(s, (struct sockaddr*)&ss, len) == 0);
    CHECK(getsockname(s, (struct sockaddr*)&ss, &len) == 0);
    *port = ipv6 ? ntohs(((struct sockaddr_in6*)&ss)->sin6_port) : ntohs(((struct sockaddr_in*)&ss)->sin_port);
    DWORD timeout = 3000; setsockopt(s, SOL_SOCKET, SO_RCVTIMEO, (char*)&timeout, sizeof(timeout));
    return s;
}

static void track(UINT16 port, BOOL ipv6, UINT32 config)
{
    udp_policy_snapshot();
    if (ipv6) {
        UINT8 dest[16]; inet_pton(AF_INET6, "2001:db8::99", dest);
        add_connection_v6(port, TRUE, (const UINT8*)&in6addr_loopback, dest, 9999, config, 123, 1);
    } else {
        struct in_addr dest; inet_pton(AF_INET, "198.51.100.99", &dest);
        add_connection(port, TRUE, htonl(INADDR_LOOPBACK), dest.s_addr, 9999, config, 123, 1);
    }
}

static void request(SOCKET s, BOOL ipv6, int value)
{
    struct sockaddr_storage ss = {0}; int len;
    if (ipv6) {
        struct sockaddr_in6 *a = (struct sockaddr_in6*)&ss;
        a->sin6_family = AF_INET6; a->sin6_addr = in6addr_loopback; a->sin6_port = htons(g_local_udp_relay_port); len = sizeof(*a);
    } else {
        struct sockaddr_in *a = (struct sockaddr_in*)&ss;
        a->sin_family = AF_INET; a->sin_addr.s_addr = htonl(INADDR_LOOPBACK); a->sin_port = htons(g_local_udp_relay_port); len = sizeof(*a);
    }
    CHECK(sendto(s, (char*)&value, sizeof(value), 0, (struct sockaddr*)&ss, len) == sizeof(value));
}

static void expect(SOCKET s, int wanted)
{
    int value = -1;
    CHECK(recv(s, (char*)&value, sizeof(value), 0) == sizeof(value));
    CHECK(value == wanted);
}

static SOCKET accept_bounded(SOCKET listener)
{
    WSAPOLLFD fd = {0}; fd.fd = listener; fd.events = POLLRDNORM;
    CHECK(WSAPoll(&fd, 1, 3000) == 1 && (fd.revents & POLLRDNORM));
    SOCKET s = accept(listener, NULL, NULL);
    CHECK(s != INVALID_SOCKET);
    DWORD timeout = 1000; setsockopt(s, SOL_SOCKET, SO_RCVTIMEO, (char*)&timeout, sizeof(timeout));
    return s;
}

static void expect_control_closed(SOCKET s)
{
    char byte; int got = recv(s, &byte, 1, 0);
    int error = got == SOCKET_ERROR ? WSAGetLastError() : 0;
    CHECK(got == 0 || (got == SOCKET_ERROR && (error == WSAECONNRESET || error == WSAECONNABORTED)));
}

static void test_nonblocking_handshakes(MOCK *m, UINT32 healthy, UINT16 proxy_port)
{
    SOCKET listener = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in addr = {0}; addr.sin_family = AF_INET; addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    CHECK(bind(listener, (struct sockaddr*)&addr, sizeof(addr)) == 0 && listen(listener, 16) == 0);
    int alen = sizeof(addr); CHECK(getsockname(listener, (struct sockaddr*)&addr, &alen) == 0);
    UINT32 slow = ProxyBridge_AddProxyConfig(PROXY_TYPE_SOCKS5, "127.0.0.1", ntohs(addr.sin_port), NULL, NULL, FALSE);
    UINT32 auth = ProxyBridge_AddProxyConfig(PROXY_TYPE_SOCKS5, "127.0.0.1", proxy_port, "user", "pass", FALSE);
    CHECK(slow && auth);
    UINT16 live_port, auth_port, ports[6];
    SOCKET live = client_open(FALSE, &live_port), credentialed = client_open(TRUE, &auth_port);
    track(live_port, FALSE, healthy);
    request(live, FALSE, 20000); expect(live, 20000);
    SOCKET clients[6], controls[6];
    for (int i = 0; i < 6; i++) {
        clients[i] = client_open(FALSE, &ports[i]); track(ports[i], FALSE, slow);
        request(clients[i], FALSE, 21000 + i);
    }
    // Six simultaneous SOCKS servers accept TCP but never answer the greeting.
    // This must neither serialize handshakes nor stall the established live flow.
    for (int i = 0; i < 6; i++) {
        controls[i] = accept_bounded(listener);
        char greeting[3]; CHECK(read_exact(controls[i], greeting, 3));
        CHECK(memcmp(greeting, "\x05\x01\x00", 3) == 0);
    }
    DWORD short_timeout = 750; setsockopt(live, SOL_SOCKET, SO_RCVTIMEO, (char*)&short_timeout, sizeof(short_timeout));
    for (int i = 0; i < 18; i++) {
        request(live, FALSE, 22000 + i); expect(live, 22000 + i);
        Sleep(500);
    }
    // Absolute handshake deadlines close silent controls even with no further packets.
    for (int i = 0; i < 6; i++) {
        expect_control_closed(controls[i]);
        closesocket(controls[i]);
        closesocket(clients[i]); remove_connection(ports[i], TRUE, FALSE);
    }
    // Keep the first four datagrams while connecting, drop excess without growing
    // the queue, and accept the common 0.0.0.0 bound-address reply.
    UINT16 queued_port; SOCKET queued = client_open(FALSE, &queued_port);
    track(queued_port, FALSE, slow);
    for (int i = 0; i < 6; i++) request(queued, FALSE, 22500 + i);
    SOCKET buffered = accept_bounded(listener); char method[3], associate[10];
    CHECK(read_exact(buffered, method, 3));
    request(live, FALSE, 22499); expect(live, 22499); // all preceding requests have been consumed
    CHECK(send(buffered, "\x05\x00", 2, 0) == 2);
    CHECK(read_exact(buffered, associate, 10) && associate[1] == 3);
    struct sockaddr_in upstream = {0}; int ulen = sizeof(upstream);
    CHECK(getsockname(m->upstream, (struct sockaddr*)&upstream, &ulen) == 0);
    char reply[10] = {5,0,0,1,0,0,0,0,0,0}; memcpy(reply + 8, &upstream.sin_port, 2);
    for (int i = 0; i < 10; i++) { CHECK(send(buffered, reply + i, 1, 0) == 1); Sleep(2); }
    for (int i = 0; i < 4; i++) expect(queued, 22500 + i);
    WSAPOLLFD empty = {0}; empty.fd = queued; empty.events = POLLRDNORM;
    CHECK(WSAPoll(&empty, 1, 500) == 0);
    closesocket(buffered); closesocket(queued); remove_connection(queued_port, TRUE, FALSE);
    // Fragment every response, including authentication and the bound endpoint.
    InterlockedExchange(&m->authenticate, 1); InterlockedExchange(&m->fragment, 1);
    track(auth_port, TRUE, auth);
    request(credentialed, TRUE, 23000); expect(credentialed, 23000);
    remove_connection(auth_port, TRUE, TRUE);
    InterlockedExchange(&m->reject_auth, 1);
    track(auth_port, TRUE, auth); request(credentialed, TRUE, 23001);
    WSAPOLLFD fd = {0}; fd.fd = credentialed; fd.events = POLLRDNORM;
    CHECK(WSAPoll(&fd, 1, 500) == 0); // failed authentication never forwards payload
    remove_connection(auth_port, TRUE, TRUE);
    InterlockedExchange(&m->reject_auth, 0); InterlockedExchange(&m->authenticate, 0); InterlockedExchange(&m->fragment, 0);
    // Policy invalidation while a handshake waits must discard its buffered first packet.
    UINT16 retired_port; SOCKET retired = client_open(FALSE, &retired_port);
    track(retired_port, FALSE, slow); request(retired, FALSE, 24000);
    SOCKET control = accept_bounded(listener); char greeting[3]; CHECK(read_exact(control, greeting, 3));
    remove_connection(retired_port, TRUE, FALSE);
    expect_control_closed(control);
    closesocket(control);
    track(retired_port, FALSE, slow); request(retired, FALSE, 24002);
    control = accept_bounded(listener); CHECK(read_exact(control, greeting, 3));
    CHECK(ProxyBridge_EditProxyConfig(slow, PROXY_TYPE_SOCKS5, "127.0.0.1", ntohs(addr.sin_port), NULL, NULL, FALSE));
    expect_control_closed(control); closesocket(control);
    // Stop cancels pending handshakes immediately, without waiting for their deadline.
    track(retired_port, FALSE, slow); request(retired, FALSE, 24001);
    control = accept_bounded(listener); CHECK(read_exact(control, greeting, 3));
    ULONGLONG start = GetTickCount64(); udp_relay_stop();
    CHECK(GetTickCount64() - start < 1500);
    expect_control_closed(control);
    closesocket(control); closesocket(listener); closesocket(retired);
    closesocket(live); closesocket(credentialed);
    remove_connection(retired_port, TRUE, FALSE); remove_connection(live_port, TRUE, FALSE); remove_connection(auth_port, TRUE, TRUE);
    CHECK(ProxyBridge_DeleteProxyConfig(slow)); CHECK(ProxyBridge_DeleteProxyConfig(auth));
    CHECK(udp_relay_start()); // no pending payload or handshake survives restart
}

int main(void)
{
    WSADATA wsa; CHECK(WSAStartup(MAKEWORD(2, 2), &wsa) == 0);
    // A busy IPv4 OR IPv6 port must fail synchronously and release the other socket.
    UINT16 busy;
    SOCKET occupied = client_open(FALSE, &busy);
    CHECK(ProxyBridge_SetRelayPorts(busy == 34010 ? 34012 : 34010, busy));
    // Exercise the production startup path: it must try UDP even without SOCKS configs,
    // and fail here before reaching WinDivertOpen (the driver is never loaded).
    CHECK(!ProxyBridge_Start());
    CHECK(!running);
    running = TRUE;
    CHECK(!udp_relay_start());
    CHECK(udp_relay_thread == NULL && udp_relay_socket == INVALID_SOCKET && udp_relay_socket6 == INVALID_SOCKET);
    closesocket(occupied);
    running = FALSE;
    occupied = client_open(TRUE, &busy);
    CHECK(ProxyBridge_SetRelayPorts(busy == 34010 ? 34012 : 34010, busy));
    running = TRUE;
    CHECK(!udp_relay_start());
    CHECK(udp_relay_thread == NULL && udp_relay_socket == INVALID_SOCKET && udp_relay_socket6 == INVALID_SOCKET);
    closesocket(occupied);
    // Startup with ZERO proxy configs must already own both UDP listeners.
    CHECK(g_proxy_config_count == 0);
    CHECK(udp_relay_start());
    CHECK(udp_relay_socket != INVALID_SOCKET && udp_relay_socket6 != INVALID_SOCKET);
    CHECK(!udp_relay_start());

    MOCK *m = (MOCK*)calloc(1, sizeof(MOCK));
    UINT16 proxy_port = mock_start(m);
    UINT32 a = ProxyBridge_AddProxyConfig(PROXY_TYPE_SOCKS5, "127.0.0.1", proxy_port, NULL, NULL, FALSE);
    UINT32 b = ProxyBridge_AddProxyConfig(PROXY_TYPE_SOCKS5, "127.0.0.1", proxy_port, NULL, NULL, FALSE);
    CHECK(a != 0 && b != 0);
    UINT16 p1, p2, p3, p6;
    SOCKET c1 = client_open(FALSE, &p1), c2 = client_open(FALSE, &p2);
    SOCKET c3 = client_open(FALSE, &p3), c6 = client_open(TRUE, &p6);
    track(p1, FALSE, a); track(p2, FALSE, a); track(p3, FALSE, b); track(p6, TRUE, a);
    // Same config, different config, and IPv6 all visit the SAME remote service.
    InterlockedExchange(&m->hold, 1);
    request(c1, FALSE, 101); request(c2, FALSE, 202); request(c3, FALSE, 303); request(c6, TRUE, 606);
    CHECK(wait_received(m, 4));
    CHECK(InterlockedCompareExchange(&m->count, 0, 0) == 4);
    InterlockedExchange(&m->hold, 0);
    expect(c1, 101); expect(c2, 202); expect(c3, 303); expect(c6, 606);
    struct sockaddr_in upstream = {0}; int upstream_len = sizeof(upstream);
    getsockname(m->upstream, (struct sockaddr*)&upstream, &upstream_len);
    CHECK(udp_is_proxy_endpoint(upstream.sin_addr.s_addr, ntohs(upstream.sin_port)));
    remove_connection(p3, TRUE, FALSE);
    Sleep(250);
    CHECK(udp_is_proxy_endpoint(upstream.sin_addr.s_addr, ntohs(upstream.sin_port))); // other flows share the endpoint

    // A delayed reply from a retired association must not reach a reused local port.
    InterlockedExchange(&m->hold, 1);
    request(c1, FALSE, 111); CHECK(wait_received(m, 5));
    UDP_FLOW old, newer;
    CHECK(udp_flow_snapshot(p1, FALSE, &old));
    remove_connection(p1, TRUE, FALSE);
    track(p1, FALSE, a);
    CHECK(udp_flow_snapshot(p1, FALSE, &newer));
    CHECK(old.id != newer.id && !udp_flow_account(&old, 0, 99));
    request(c1, FALSE, 112); CHECK(wait_received(m, 6));
    InterlockedExchange(&m->hold, 0);
    expect(c1, 112);

    // A config/policy edit also retires outstanding replies, even with the same local port.
    InterlockedExchange(&m->hold, 1);
    request(c2, FALSE, 211); CHECK(wait_received(m, 7));
    CHECK(ProxyBridge_EditProxyConfig(a, PROXY_TYPE_SOCKS5, "127.0.0.1", proxy_port, NULL, NULL, FALSE));
    track(p2, FALSE, a);
    request(c2, FALSE, 212); CHECK(wait_received(m, 8));
    InterlockedExchange(&m->hold, 0);
    expect(c2, 212);

    // More than 128 sockets: the old fd_set approach would silently omit live sessions.
    enum { MANY = 70 };
    SOCKET many[MANY]; UINT16 ports[MANY];
    InterlockedExchange(&m->hold, 1);
    for (int i = 0; i < MANY; i++) {
        many[i] = client_open(FALSE, &ports[i]); track(ports[i], FALSE, a); request(many[i], FALSE, 1000 + i);
    }
    CHECK(wait_received(m, 8 + MANY));
    InterlockedExchange(&m->hold, 0);
    for (int i = 0; i < MANY; i++) expect(many[i], 1000 + i);

    // Closed SOCKS control channels are recreated on the next request.
    InterlockedExchange(&m->disconnect, 1);
    Sleep(1200); // passes the upstream helper's one-second retry guard
    request(many[0], FALSE, 9000);
    CHECK(wait_received(m, 9 + MANY));
    expect(many[0], 9000);
    test_nonblocking_handshakes(m, a, proxy_port);
    for (int i = 0; i < MANY; i++) { closesocket(many[i]); remove_connection(ports[i], TRUE, FALSE); }
    closesocket(c1); closesocket(c2); closesocket(c3); closesocket(c6);
    remove_connection(p1, TRUE, FALSE); remove_connection(p2, TRUE, FALSE);
    remove_connection(p3, TRUE, FALSE); remove_connection(p6, TRUE, TRUE);
    udp_relay_stop();
    CHECK(udp_relay_thread == NULL && udp_relay_socket == INVALID_SOCKET && udp_relay_socket6 == INVALID_SOCKET);
    CHECK(!udp_is_proxy_endpoint(upstream.sin_addr.s_addr, ntohs(upstream.sin_port)));
    InterlockedExchange(&m->stop, 1);
    CHECK(WaitForSingleObject(m->thread, 5000) == WAIT_OBJECT_0);
    CloseHandle(m->thread);
    CHECK(m->failed == 0);
    free(m);
    CHECK(ProxyBridge_DeleteProxyConfig(a)); CHECK(ProxyBridge_DeleteProxyConfig(b));
    // Repeat with no configs: no stale worker, listener, or association survives a restart.
    CHECK(udp_relay_start()); udp_relay_stop();
    CHECK(udp_relay_start()); udp_relay_stop();
    running = FALSE; WSACleanup();
    return pbtest_done("test_udp_relay");
}
