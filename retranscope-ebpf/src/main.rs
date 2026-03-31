#![no_std]
#![no_main]

use aya_ebpf::{macros::btf_tracepoint, programs::BtfTracePointContext};
use aya_log_ebpf::info;

#[btf_tracepoint(function = "tcp_retransmit_skb")]
pub fn tcp_retransmit_skb(ctx: BtfTracePointContext) -> i32 {
    match try_tcp_retransmit_skb(ctx) {
        Ok(ret) => ret,
        Err(ret) => ret,
    }
}

fn try_tcp_retransmit_skb(ctx: BtfTracePointContext) -> Result<i32, i32> {
    info!(&ctx, "tracepoint tcp_retransmit_skb called");
    Ok(0)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(link_section = "license")]
#[unsafe(no_mangle)]
static LICENSE: [u8; 13] = *b"Dual MIT/GPL\0";
