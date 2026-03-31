use std::{
    mem::MaybeUninit,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::Context;
use libbpf_rs::{
    MapCore, MapFlags,
    skel::{OpenSkel, Skel, SkelBuilder},
};
use retranscope_ebpf::RetranscopeSkelBuilder;

use crate::state::State;

fn read_percpu_counter(map: &libbpf_rs::Map) -> u64 {
    let key = 0u32.to_ne_bytes();
    match map.lookup_percpu(&key, MapFlags::ANY) {
        Ok(Some(values)) => {
            let mut total: u64 = 0;
            for v in &values {
                let bytes: [u8; 8] = v[..8].try_into().unwrap_or([0; 8]);
                total += u64::from_ne_bytes(bytes);
            }
            total
        }
        _ => 0,
    }
}

pub(crate) fn collector_thread(
    state: Arc<Mutex<State>>,
    interval_ms: Arc<AtomicU64>,
    quit: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let builder = RetranscopeSkelBuilder::default();
    let mut open_object = MaybeUninit::uninit();
    let open_skel = builder
        .open(&mut open_object)
        .context("failed to open BPF skeleton")?;
    let mut skel = open_skel.load().context("failed to load BPF programs")?;
    skel.attach().context("failed to attach BPF programs")?;

    let mut last = Instant::now();
    while !quit.load(Ordering::Relaxed) {
        let ms = interval_ms.load(Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(ms));

        let now = Instant::now();
        let elapsed = now.duration_since(last);
        last = now;

        let trans_total = read_percpu_counter(&skel.maps.trans_bytes);
        let retrans_total = read_percpu_counter(&skel.maps.retrans_bytes);

        let mut s = state.lock().unwrap();
        s.trans.record_sample(trans_total, elapsed);
        s.retrans.record_sample(retrans_total, elapsed);
    }

    Ok(())
}
