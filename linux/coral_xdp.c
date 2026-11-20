/* clang -O2 -g -target bpf -c coral_xdp.c -o coral_xdp.o */
#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ip.h>
#include <linux/ipv6.h>
#include <linux/tcp.h>
#include <bpf/bpf_endian.h>
#include <bpf/bpf_helpers.h>

#ifndef CORAL_PORT
#define CORAL_PORT 25565
#endif

/* One bucket per source per RX CPU. Per-CPU values avoid a shared atomic in the NIC path. */
struct source { __u32 address[4]; __u32 family; };
struct bucket { __u64 updated; __u64 credit; };
struct {
    __uint(type, BPF_MAP_TYPE_LRU_PERCPU_HASH);
    __uint(max_entries, 16384);
    __type(key, struct source);
    __type(value, struct bucket);
} syn_rate SEC(".maps");

static __always_inline int admit_syn(struct source *key)
{
    const __u64 cost = 100000000ULL;
    const __u64 capacity = 40 * cost;
    __u64 now = bpf_ktime_get_ns();
    struct bucket *b = bpf_map_lookup_elem(&syn_rate, key);
    if (!b) {
        struct bucket initial = { .updated = now, .credit = capacity - cost };
        bpf_map_update_elem(&syn_rate, key, &initial, BPF_NOEXIST);
        return XDP_PASS;
    }
    __u64 elapsed = now - b->updated;
    __u64 available = elapsed >= capacity ? capacity : elapsed + b->credit;
    if (available > capacity)
        available = capacity;
    b->updated = now;
    if (available < cost) {
        b->credit = available;
        return XDP_DROP;
    }
    b->credit = available - cost;
    return XDP_PASS;
}

SEC("xdp")
int coral_ingress(struct xdp_md *ctx)
{
    void *end = (void *)(long)ctx->data_end;
    struct ethhdr *eth = (void *)(long)ctx->data;
    if ((void *)(eth + 1) > end)
        return XDP_PASS;
    void *transport;
    __u32 length;
    struct source key = {};
    if (eth->h_proto == bpf_htons(ETH_P_IP)) {
        struct iphdr *ip = (void *)(eth + 1);
        if ((void *)(ip + 1) > end || ip->protocol != 6 || ip->ihl < 5 ||
            (ip->frag_off & bpf_htons(0x3fff)))
            return XDP_PASS;
        __u32 header = ip->ihl * 4;
        __u32 total = bpf_ntohs(ip->tot_len);
        if (total < header)
            return XDP_PASS;
        transport = (void *)ip + header;
        length = total - header;
        key.address[0] = ip->saddr;
        key.family = 4;
    } else if (eth->h_proto == bpf_htons(ETH_P_IPV6)) {
        struct ipv6hdr *ip = (void *)(eth + 1);
        if ((void *)(ip + 1) > end || ip->nexthdr != 6)
            return XDP_PASS;
        transport = ip + 1;
        length = bpf_ntohs(ip->payload_len);
        __builtin_memcpy(key.address, &ip->saddr, sizeof(key.address));
        key.family = 6;
    } else {
        return XDP_PASS;
    }
    struct tcphdr *tcp = transport;
    if ((void *)(tcp + 1) > end || tcp->dest != bpf_htons(CORAL_PORT))
        return XDP_PASS;
    __u32 header = tcp->doff * 4;
    if (header < sizeof(*tcp) || header > length || transport + header > end)
        return XDP_DROP;
    if (tcp->syn && !tcp->ack)
        return (tcp->fin || tcp->rst) ? XDP_DROP : admit_syn(&key);
    return XDP_PASS;
}

char license[] SEC("license") = "GPL";
