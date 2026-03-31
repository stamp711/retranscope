use std::{env, ffi::CString, fs, io::Write, os::unix::io::AsRawFd, path::PathBuf, ptr};

use libbpf_cargo::SkeletonBuilder;

/// Generate vmlinux.h from /sys/kernel/btf/vmlinux using libbpf's BTF dump API.
fn generate_vmlinux(output: &std::path::Path) {
    let btf_path = CString::new("/sys/kernel/btf/vmlinux").unwrap();
    let mut file = fs::File::create(output).expect("failed to create vmlinux.h");
    let fd = file.as_raw_fd();

    unsafe extern "C" fn printf_cb(
        ctx: *mut std::os::raw::c_void,
        fmt: *const std::os::raw::c_char,
        args: *mut libbpf_sys::__va_list_tag,
    ) {
        unsafe { libbpf_sys::vdprintf(ctx as i32, fmt, args) };
    }

    unsafe {
        let btf = libbpf_sys::btf__parse_raw(btf_path.as_ptr());
        assert!(!btf.is_null(), "btf__parse_raw failed");

        let dump = libbpf_sys::btf_dump__new(btf, Some(printf_cb), fd as *mut _, ptr::null());
        assert!(!dump.is_null(), "btf_dump__new failed");

        file.write_all(
            b"#ifndef __VMLINUX_H__\n\
              #define __VMLINUX_H__\n\n\
              #ifndef BPF_NO_PRESERVE_ACCESS_INDEX\n\
              #pragma clang attribute push (__attribute__((preserve_access_index)), apply_to = record)\n\
              #endif\n\n",
        )
        .unwrap();

        let nr_types = libbpf_sys::btf__type_cnt(btf);
        for i in 1..nr_types {
            libbpf_sys::btf_dump__dump_type(dump, i);
        }

        file.write_all(
            b"\n#ifndef BPF_NO_PRESERVE_ACCESS_INDEX\n\
              #pragma clang attribute pop\n\
              #endif\n\n\
              #endif /* __VMLINUX_H__ */\n",
        )
        .unwrap();

        libbpf_sys::btf_dump__free(dump);
        libbpf_sys::btf__free(btf);
    }
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let bpf_src = "src/bpf/retranscope.bpf.c";

    let vmlinux_h = out_dir.join("vmlinux.h");
    generate_vmlinux(&vmlinux_h);

    SkeletonBuilder::new()
        .source(bpf_src)
        .clang_args(["-I", out_dir.to_str().unwrap()])
        .build_and_generate(out_dir.join("retranscope.skel.rs"))
        .expect("failed to build and generate BPF skeleton");

    println!("cargo:rerun-if-changed={bpf_src}");
}
