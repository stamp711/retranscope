#include "vmlinux.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>

char LICENSE[] SEC("license") = "Dual MIT/GPL";

struct {
	__uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
	__uint(max_entries, 1);
	__type(key, u32);
	__type(value, u64);
} trans_bytes SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
	__uint(max_entries, 1);
	__type(key, u32);
	__type(value, u64);
} retrans_bytes SEC(".maps");

static __always_inline int account_skb_bytes(struct sk_buff *skb, void *map)
{
	u32 key = 0;
	u32 len = BPF_CORE_READ(skb, len);

	u64 *counter = bpf_map_lookup_elem(map, &key);
	if (counter)
		*counter += len;

	return 0;
}

SEC("fentry/__tcp_transmit_skb")
int BPF_PROG(tcp_transmit_skb, struct sock *sk, struct sk_buff *skb)
{
	return account_skb_bytes(skb, &trans_bytes);
}

SEC("tp_btf/tcp_retransmit_skb")
int BPF_PROG(tcp_retransmit_skb, struct sock *sk, struct sk_buff *skb)
{
	return account_skb_bytes(skb, &retrans_bytes);
}
