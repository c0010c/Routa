#include "pb_internal.h"

// The relay thread owns all associations. No socket or config-array pointer is shared with
// config writers. A private SOCKS5 UDP association identifies the original local flow.
typedef enum {
    U_IDLE, U_CONNECT, U_SEND_METHOD, U_READ_METHOD, U_SEND_AUTH, U_READ_AUTH,
    U_SEND_ASSOC, U_READ_HEADER, U_READ_ADDR, U_READY
} UDP_STAGE;
typedef struct UDP_PENDING {
    struct UDP_PENDING *next;
    int len;
    unsigned char data[1];
} UDP_PENDING;
#define UDP_PENDING_LIMIT (4 * 1024 * 1024)
#define UDP_PENDING_PER_FLOW 4
#define UDP_HANDSHAKE_MS 8000

typedef struct {
    BOOL used;
    UDP_FLOW flow;
    PROXY_CONFIG proxy;
    ULONGLONG last_activity;
    UDP_STAGE stage;
    ULONGLONG deadline;
    unsigned char handshake[513]; // RFC 1929: version + two lengths + 255-byte credentials
    int offset, length;
    UDP_PENDING *head, *tail;
    int queued;
} UDP_ASSOC;

// Packet capture needs only the upstream endpoints to avoid intercepting our own UDP.
// This small index has a separate lock, held only for memory accesses, never network I/O.
#define UDP_ENDPOINT_BUCKETS 256
typedef struct { UINT32 ip; UINT16 port; int next; BOOL used; } UDP_ENDPOINT;
static SRWLOCK endpoint_lock = SRWLOCK_INIT;
static int endpoint_heads[UDP_ENDPOINT_BUCKETS]; // slot + 1, zero means end
static UDP_ENDPOINT endpoints[PB_UDP_MAX_SESSIONS];
static UDP_ASSOC *associations;
static WSAPOLLFD *pollfds;
static int *pollslots;
static unsigned char *recv_data, *send_packet;
static volatile LONG udp_stopping;
static size_t pending_bytes; // relay-thread owned, includes allocation overhead

static unsigned endpoint_hash(UINT32 ip, UINT16 port) { return (ip ^ (ip >> 16) ^ port) % UDP_ENDPOINT_BUCKETS; }

static void endpoint_remove(int slot)
{
    AcquireSRWLockExclusive(&endpoint_lock);
    UDP_ENDPOINT *e = &endpoints[slot];
    if (e->used) {
        int *p = &endpoint_heads[endpoint_hash(e->ip, e->port)];
        while (*p && *p != slot + 1) p = &endpoints[*p - 1].next;
        if (*p) *p = e->next;
        ZeroMemory(e, sizeof(*e));
    }
    ReleaseSRWLockExclusive(&endpoint_lock);
}

static void endpoint_add(int slot, const struct sockaddr_in *addr)
{
    endpoint_remove(slot);
    AcquireSRWLockExclusive(&endpoint_lock);
    UDP_ENDPOINT *e = &endpoints[slot];
    e->ip = addr->sin_addr.s_addr; e->port = ntohs(addr->sin_port); e->used = TRUE;
    unsigned bucket = endpoint_hash(e->ip, e->port);
    e->next = endpoint_heads[bucket]; endpoint_heads[bucket] = slot + 1;
    ReleaseSRWLockExclusive(&endpoint_lock);
}

BOOL udp_is_proxy_endpoint(UINT32 ip, UINT16 port)
{
    BOOL found = FALSE;
    AcquireSRWLockShared(&endpoint_lock);
    for (int n = endpoint_heads[endpoint_hash(ip, port)]; n; n = endpoints[n - 1].next) {
        if (endpoints[n - 1].ip == ip && endpoints[n - 1].port == port) { found = TRUE; break; }
    }
    ReleaseSRWLockShared(&endpoint_lock);
    return found;
}

static void pending_clear(UDP_ASSOC *a)
{
    while (a->head) {
        UDP_PENDING *p = a->head;
        a->head = p->next;
        pending_bytes -= sizeof(*p) + p->len;
        free(p);
    }
    a->tail = NULL; a->queued = 0;
}

static void association_disconnect(int slot)
{
    UDP_ASSOC *a = &associations[slot];
    endpoint_remove(slot);
    if (a->proxy.udp_tcp_ctrl != INVALID_SOCKET) closesocket(a->proxy.udp_tcp_ctrl);
    if (a->proxy.udp_send_sock != INVALID_SOCKET) closesocket(a->proxy.udp_send_sock);
    a->proxy.udp_tcp_ctrl = a->proxy.udp_send_sock = INVALID_SOCKET;
    a->proxy.udp_connected = FALSE;
    a->stage = U_IDLE;
    pending_clear(a);
}

static void association_free(int slot)
{
    association_disconnect(slot);
    associations[slot].used = FALSE;
}

static int association_get(const UDP_FLOW *flow)
{
    int empty = -1;
    ULONGLONG now = GetTickCount64();
    for (int i = 0; i < PB_UDP_MAX_SESSIONS; i++) {
        UDP_ASSOC *a = &associations[i];
        if (a->used && (!udp_flow_account(&a->flow, 0, 0) || now - a->last_activity > UDP_REEVAL_IDLE_MS))
            association_free(i);
        if (a->used && a->flow.id == flow->id && a->flow.policy_version == flow->policy_version) return i;
        if (!a->used && empty < 0) empty = i;
    }
    if (empty < 0) return -1; // bounded resource use; never steal another live flow's association
    UDP_ASSOC *a = &associations[empty];
    ZeroMemory(a, sizeof(*a));
    if (!copy_proxy_config(flow->config_id, &a->proxy) || a->proxy.type != PROXY_TYPE_SOCKS5) return -1;
    a->proxy.udp_tcp_ctrl = a->proxy.udp_send_sock = INVALID_SOCKET;
    a->proxy.udp_connected = FALSE; a->proxy.last_udp_attempt = 0;
    a->flow = *flow; a->used = TRUE; a->last_activity = now;
    return empty;
}

static BOOL association_connect(int slot)
{
    UDP_ASSOC *a = &associations[slot];
    if (a->stage != U_IDLE) return TRUE;
    if (InterlockedCompareExchange(&udp_stopping, 0, 0) || !udp_flow_account(&a->flow, 0, 0)) return FALSE;
    ULONGLONG now = GetTickCount64();
    if (a->proxy.last_udp_attempt && now - a->proxy.last_udp_attempt < 1000) return FALSE;
    a->proxy.last_udp_attempt = now;
    // Config writers resolve the IPv4 address before publishing the snapshot. Never
    // perform DNS or a blocking connect/handshake on this single relay thread.
    if (!a->proxy.resolved_ip) return FALSE;
    a->proxy.udp_tcp_ctrl = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    u_long nonblocking = 1;
    if (a->proxy.udp_tcp_ctrl == INVALID_SOCKET ||
        ioctlsocket(a->proxy.udp_tcp_ctrl, FIONBIO, &nonblocking) == SOCKET_ERROR) {
        association_disconnect(slot); return FALSE;
    }
    configure_tcp_socket(a->proxy.udp_tcp_ctrl, 65536, 0);
    struct sockaddr_in addr = {0}; addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = a->proxy.resolved_ip; addr.sin_port = htons(a->proxy.port);
    int result = connect(a->proxy.udp_tcp_ctrl, (struct sockaddr*)&addr, sizeof(addr));
    if (result == SOCKET_ERROR && WSAGetLastError() != WSAEWOULDBLOCK) {
        association_disconnect(slot); return FALSE;
    }
    a->stage = U_CONNECT; a->deadline = now + UDP_HANDSHAKE_MS;
    return TRUE;
}

static BOOL association_send(int slot, const unsigned char *packet, int len)
{
    UDP_ASSOC *a = &associations[slot];
    if (!udp_flow_account(&a->flow, 0, 0)) { association_free(slot); return FALSE; }
    int sent = sendto(a->proxy.udp_send_sock, (const char*)packet, len, 0,
                      (struct sockaddr*)&a->proxy.udp_relay_addr, sizeof(a->proxy.udp_relay_addr));
    if (sent == len) {
        udp_flow_account(&a->flow, (UINT32)(len - (a->flow.ipv6 ? 22 : 10)), 0);
        a->last_activity = GetTickCount64();
    } else if (sent == SOCKET_ERROR && WSAGetLastError() != WSAEWOULDBLOCK) {
        association_disconnect(slot); return FALSE;
    }
    return TRUE;
}

static void handshake_stage(UDP_ASSOC *a, UDP_STAGE stage, int length)
{
    a->stage = stage; a->offset = 0; a->length = length;
}

static BOOL handshake_writes(UDP_STAGE stage)
{
    return stage == U_CONNECT || stage == U_SEND_METHOD || stage == U_SEND_AUTH || stage == U_SEND_ASSOC;
}

static void handshake_associate(UDP_ASSOC *a)
{
    ZeroMemory(a->handshake, 10);
    a->handshake[0] = SOCKS5_VERSION; a->handshake[1] = SOCKS5_CMD_UDP_ASSOCIATE;
    a->handshake[3] = SOCKS5_ATYP_IPV4;
    handshake_stage(a, U_SEND_ASSOC, 10);
}

// One bounded nonblocking operation per readiness event; partial TCP frames retain
// their offset. Every phase shares an absolute deadline, including slow trickle replies.
static void handshake_progress(int slot, short events)
{
    UDP_ASSOC *a = &associations[slot];
    if (events & (POLLERR | POLLHUP | POLLNVAL)) goto fail;
    if (a->stage == U_CONNECT) {
        int error = 0, size = sizeof(error);
        if (getsockopt(a->proxy.udp_tcp_ctrl, SOL_SOCKET, SO_ERROR, (char*)&error, &size) == SOCKET_ERROR || error) goto fail;
        a->handshake[0] = SOCKS5_VERSION;
        a->handshake[1] = a->proxy.username[0] ? 2 : 1;
        a->handshake[2] = SOCKS5_AUTH_NONE; a->handshake[3] = 2;
        handshake_stage(a, U_SEND_METHOD, a->proxy.username[0] ? 4 : 3);
        return;
    }
    BOOL writing = handshake_writes(a->stage);
    int got = writing
        ? send(a->proxy.udp_tcp_ctrl, (char*)a->handshake + a->offset, a->length - a->offset, 0)
        : recv(a->proxy.udp_tcp_ctrl, (char*)a->handshake + a->offset, a->length - a->offset, 0);
    if (got == SOCKET_ERROR && WSAGetLastError() == WSAEWOULDBLOCK) return;
    if (got <= 0) goto fail;
    a->offset += got;
    if (a->offset != a->length) return;
    switch (a->stage) {
    case U_SEND_METHOD: handshake_stage(a, U_READ_METHOD, 2); return;
    case U_READ_METHOD:
        if (a->handshake[0] != SOCKS5_VERSION) goto fail;
        if (a->handshake[1] == SOCKS5_AUTH_NONE) { handshake_associate(a); return; }
        if (a->handshake[1] != 2 || !a->proxy.username[0]) goto fail;
        {
            size_t ul = strnlen_s(a->proxy.username, sizeof(a->proxy.username));
            size_t pl = strnlen_s(a->proxy.password, sizeof(a->proxy.password));
            if (ul > 255 || pl > 255) goto fail;
            a->handshake[0] = 1; a->handshake[1] = (BYTE)ul;
            memcpy(a->handshake + 2, a->proxy.username, ul);
            a->handshake[2 + ul] = (BYTE)pl;
            memcpy(a->handshake + 3 + ul, a->proxy.password, pl);
            handshake_stage(a, U_SEND_AUTH, (int)(3 + ul + pl));
        }
        return;
    case U_SEND_AUTH: handshake_stage(a, U_READ_AUTH, 2); return;
    case U_READ_AUTH:
        if (a->handshake[0] != 1 || a->handshake[1]) goto fail;
        handshake_associate(a); return;
    case U_SEND_ASSOC: handshake_stage(a, U_READ_HEADER, 4); return;
    case U_READ_HEADER:
        if (a->handshake[0] != SOCKS5_VERSION || a->handshake[1] || a->handshake[2] ||
            a->handshake[3] != SOCKS5_ATYP_IPV4) goto fail;
        handshake_stage(a, U_READ_ADDR, 6); return;
    case U_READ_ADDR:
        ZeroMemory(&a->proxy.udp_relay_addr, sizeof(a->proxy.udp_relay_addr));
        a->proxy.udp_relay_addr.sin_family = AF_INET;
        memcpy(&a->proxy.udp_relay_addr.sin_addr, a->handshake, 4);
        memcpy(&a->proxy.udp_relay_addr.sin_port, a->handshake + 4, 2);
        if (!a->proxy.udp_relay_addr.sin_port) goto fail;
        if (!a->proxy.udp_relay_addr.sin_addr.s_addr) a->proxy.udp_relay_addr.sin_addr.s_addr = a->proxy.resolved_ip;
        if (!udp_flow_account(&a->flow, 0, 0)) { association_free(slot); return; }
        a->proxy.udp_send_sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
        {
            u_long nonblocking = 1;
            if (a->proxy.udp_send_sock == INVALID_SOCKET ||
                ioctlsocket(a->proxy.udp_send_sock, FIONBIO, &nonblocking) == SOCKET_ERROR) goto fail;
        }
        configure_udp_socket(a->proxy.udp_send_sock, 65536, 0);
        BOOL keepalive = TRUE;
        setsockopt(a->proxy.udp_tcp_ctrl, SOL_SOCKET, SO_KEEPALIVE, (char*)&keepalive, sizeof(keepalive));
        struct tcp_keepalive ka = {1, 10000, 2000}; DWORD bytes;
        WSAIoctl(a->proxy.udp_tcp_ctrl, SIO_KEEPALIVE_VALS, &ka, sizeof(ka), NULL, 0, &bytes, NULL, NULL);
        endpoint_add(slot, &a->proxy.udp_relay_addr);
        a->proxy.udp_connected = TRUE; a->stage = U_READY;
        while (a->head) {
            UDP_PENDING *p = a->head;
            a->head = p->next;
            if (!a->head) a->tail = NULL;
            a->queued--; pending_bytes -= sizeof(*p) + p->len;
            BOOL ok = association_send(slot, p->data, p->len);
            free(p);
            if (!ok) return;
        }
        return;
    default: break;
    }
fail:
    association_disconnect(slot);
}

static void relay_request(BOOL ipv6, unsigned char *data, unsigned char *packet)
{
    SOCKET local = ipv6 ? udp_relay_socket6 : udp_relay_socket;
    struct sockaddr_storage peer;
    int peer_len = sizeof(peer);
    int len = recvfrom(local, (char*)data, MAXBUF, 0, (struct sockaddr*)&peer, &peer_len);
    int header = ipv6 ? 22 : 10;
    if (len <= 0 || len > MAXBUF - header) return;
    UINT16 port = ipv6 ? ntohs(((struct sockaddr_in6*)&peer)->sin6_port) : ntohs(((struct sockaddr_in*)&peer)->sin_port);
    UDP_FLOW flow;
    // Preserve the upstream application's-port lookup policy (review issue 3).
    if (!udp_flow_snapshot(port, ipv6, &flow)) return;
    int slot = association_get(&flow);
    if (slot < 0 || !association_connect(slot)) return;
    UDP_ASSOC *a = &associations[slot];
    ZeroMemory(packet, header);
    packet[3] = ipv6 ? SOCKS5_ATYP_IPV6 : SOCKS5_ATYP_IPV4;
    memcpy(packet + 4, flow.dest, ipv6 ? 16 : 4);
    packet[header - 2] = (BYTE)(flow.dest_port >> 8); packet[header - 1] = (BYTE)flow.dest_port;
    memcpy(packet + header, data, len);
    if (a->stage == U_READY) { association_send(slot, packet, header + len); return; }
    size_t size = sizeof(UDP_PENDING) + header + len;
    if (a->queued >= UDP_PENDING_PER_FLOW || pending_bytes + size > UDP_PENDING_LIMIT) return;
    UDP_PENDING *p = (UDP_PENDING*)malloc(size);
    if (!p) return;
    p->next = NULL; p->len = header + len; memcpy(p->data, packet, p->len);
    if (a->tail) a->tail->next = p; else a->head = p;
    a->tail = p; a->queued++; pending_bytes += size;
}

static void relay_response(int slot, unsigned char *packet)
{
    UDP_ASSOC *a = &associations[slot];
    struct sockaddr_in peer;
    int peer_len = sizeof(peer);
    int len = recvfrom(a->proxy.udp_send_sock, (char*)packet, MAXBUF, 0, (struct sockaddr*)&peer, &peer_len);
    if (len == SOCKET_ERROR) {
        if (WSAGetLastError() != WSAEWOULDBLOCK) association_disconnect(slot);
        return;
    }
    if (len < 4 || packet[0] || packet[1] || packet[2]) return;
    int header = packet[3] == SOCKS5_ATYP_IPV4 ? 10 : packet[3] == SOCKS5_ATYP_IPV6 ? 22 : 0;
    if (!header || len < header || (packet[3] == SOCKS5_ATYP_IPV6) != a->flow.ipv6) return;
    // A reply may use the same endpoint as every other app; only this association's
    // immutable flow identity decides the recipient. Replaced tracking entries reject old replies.
    if (!udp_flow_account(&a->flow, 0, 0)) { association_free(slot); return; }
    int sent;
    if (a->flow.ipv6) {
        struct sockaddr_in6 target = {0}; target.sin6_family = AF_INET6;
        memcpy(&target.sin6_addr, a->flow.client, 16); target.sin6_port = htons(a->flow.client_port);
        sent = sendto(udp_relay_socket6, (char*)packet + header, len - header, 0, (struct sockaddr*)&target, sizeof(target));
    } else {
        struct sockaddr_in target = {0}; target.sin_family = AF_INET;
        memcpy(&target.sin_addr, a->flow.client, 4); target.sin_port = htons(a->flow.client_port);
        sent = sendto(udp_relay_socket, (char*)packet + header, len - header, 0, (struct sockaddr*)&target, sizeof(target));
    }
    if (sent == len - header) {
        udp_flow_account(&a->flow, 0, (UINT32)sent);
        a->last_activity = GetTickCount64();
    }
}

static void poll_add(int *count, SOCKET s, int slot)
{
    pollfds[*count].fd = s; pollfds[*count].events = POLLRDNORM; pollfds[*count].revents = 0;
    if (slot >= 0 && s == associations[slot].proxy.udp_tcp_ctrl && handshake_writes(associations[slot].stage))
        pollfds[*count].events = POLLWRNORM;
    pollslots[*count] = slot; (*count)++;
}

DWORD WINAPI udp_relay_server(LPVOID arg)
{
    unsigned char *data = recv_data, *packet = send_packet;
    while (running && !InterlockedCompareExchange(&udp_stopping, 0, 0)) {
        int count = 0;
        poll_add(&count, udp_relay_socket, -1);
        poll_add(&count, udp_relay_socket6, -2);
        ULONGLONG now = GetTickCount64();
        for (int i = 0; i < PB_UDP_MAX_SESSIONS; i++) {
            UDP_ASSOC *a = &associations[i];
            if (!a->used) continue;
            if (!udp_flow_account(&a->flow, 0, 0) || now - a->last_activity > UDP_REEVAL_IDLE_MS) {
                association_free(i); continue;
            }
            if (a->stage != U_IDLE && a->stage != U_READY && now >= a->deadline)
                association_disconnect(i);
            if (a->proxy.udp_tcp_ctrl != INVALID_SOCKET)
                poll_add(&count, a->proxy.udp_tcp_ctrl, i);
            if (a->proxy.udp_connected) {
                poll_add(&count, a->proxy.udp_send_sock, i);
            }
        }
        // WSAPoll has no fd_set's 64/128-socket ceiling. Each slot remembers its owner.
        int ready = WSAPoll(pollfds, (ULONG)count, 200);
        if (ready <= 0) continue;
        // Process existing association sockets BEFORE accepting new requests that may
        // retire/reuse their slots. No stale poll result can be applied to a new socket.
        for (int n = 2; n < count; n++) {
            if (!pollfds[n].revents) continue;
            int slot = pollslots[n]; UDP_ASSOC *a = &associations[slot];
            if (!a->used) continue;
            if (pollfds[n].fd == a->proxy.udp_tcp_ctrl) {
                if (a->stage == U_READY) association_disconnect(slot);
                else handshake_progress(slot, pollfds[n].revents);
            } else if (pollfds[n].fd == a->proxy.udp_send_sock) {
                relay_response(slot, packet);
            }
        }
        if (pollfds[0].revents & POLLRDNORM) relay_request(FALSE, data, packet);
        if (pollfds[1].revents & POLLRDNORM) relay_request(TRUE, data, packet);
    }
    for (int i = 0; i < PB_UDP_MAX_SESSIONS; i++) if (associations[i].used) association_free(i);
    return 0;
}

static void udp_release(void)
{
    if (udp_relay_socket != INVALID_SOCKET) closesocket(udp_relay_socket);
    if (udp_relay_socket6 != INVALID_SOCKET) closesocket(udp_relay_socket6);
    udp_relay_socket = udp_relay_socket6 = INVALID_SOCKET;
    free(associations); free(pollfds); free(pollslots);
    free(recv_data); free(send_packet);
    associations = NULL; pollfds = NULL; pollslots = NULL;
    recv_data = send_packet = NULL;
}

BOOL udp_relay_start(void)
{
    if (udp_relay_thread != NULL) return FALSE;
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) return FALSE;
    associations = (UDP_ASSOC*)calloc(PB_UDP_MAX_SESSIONS, sizeof(*associations));
    pollfds = (WSAPOLLFD*)calloc(2 + 2 * PB_UDP_MAX_SESSIONS, sizeof(*pollfds));
    pollslots = (int*)calloc(2 + 2 * PB_UDP_MAX_SESSIONS, sizeof(*pollslots));
    recv_data = (unsigned char*)malloc(MAXBUF); send_packet = (unsigned char*)malloc(MAXBUF);
    if (!associations || !pollfds || !pollslots || !recv_data || !send_packet) goto fail;
    udp_relay_socket = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    udp_relay_socket6 = socket(AF_INET6, SOCK_DGRAM, IPPROTO_UDP);
    if (udp_relay_socket == INVALID_SOCKET || udp_relay_socket6 == INVALID_SOCKET) goto fail;
    // Preserve ANY listening for WinDivert's src/dst-address rewrite, but do not share a
    // port with another process: readiness must mean we actually own both listeners.
    BOOL exclusive = TRUE; int v6only = 1;
    if (setsockopt(udp_relay_socket, SOL_SOCKET, SO_EXCLUSIVEADDRUSE, (char*)&exclusive, sizeof(exclusive)) == SOCKET_ERROR ||
        setsockopt(udp_relay_socket6, SOL_SOCKET, SO_EXCLUSIVEADDRUSE, (char*)&exclusive, sizeof(exclusive)) == SOCKET_ERROR ||
        setsockopt(udp_relay_socket6, IPPROTO_IPV6, IPV6_V6ONLY, (char*)&v6only, sizeof(v6only)) == SOCKET_ERROR) goto fail;
    struct sockaddr_in a4 = {0}; a4.sin_family = AF_INET; a4.sin_port = htons(g_local_udp_relay_port);
    struct sockaddr_in6 a6 = {0}; a6.sin6_family = AF_INET6; a6.sin6_port = htons(g_local_udp_relay_port);
    if (bind(udp_relay_socket, (struct sockaddr*)&a4, sizeof(a4)) == SOCKET_ERROR ||
        bind(udp_relay_socket6, (struct sockaddr*)&a6, sizeof(a6)) == SOCKET_ERROR) goto fail;
    configure_udp_socket(udp_relay_socket, 262144, 0);
    configure_udp_socket(udp_relay_socket6, 262144, 0);
    u_long nonblocking = 1;
    if (ioctlsocket(udp_relay_socket, FIONBIO, &nonblocking) == SOCKET_ERROR ||
        ioctlsocket(udp_relay_socket6, FIONBIO, &nonblocking) == SOCKET_ERROR) goto fail;
    InterlockedExchange(&udp_stopping, 0);
    udp_relay_thread = CreateThread(NULL, 0, udp_relay_server, NULL, 0, NULL);
    if (!udp_relay_thread) goto fail;
    log_message("UDP relay ready on port %u (IPv4 + IPv6)", g_local_udp_relay_port);
    return TRUE;
fail:
    log_message("UDP relay start failed: %d", WSAGetLastError());
    udp_release(); WSACleanup();
    return FALSE;
}

void udp_relay_stop(void)
{
    if (!udp_relay_thread) return;
    InterlockedExchange(&udp_stopping, 1);
    // All network I/O is nonblocking; the poll exits within 200 ms. Join before
    // freeing state so no old worker can use a restarted engine's sockets.
    WaitForSingleObject(udp_relay_thread, INFINITE);
    CloseHandle(udp_relay_thread); udp_relay_thread = NULL;
    udp_release(); WSACleanup();
}
