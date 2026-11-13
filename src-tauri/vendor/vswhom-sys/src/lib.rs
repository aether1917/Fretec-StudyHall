//! Vendored replacement for `vswhom-sys`.
//!
//! The upstream crate compiles a C++ file to discover Visual Studio / Windows SDK
//! installations. This project builds with a downloaded MSVC CRT/SDK and `lld-link`
//! instead of an installed Visual Studio, so there is no C++ compiler available for
//! build scripts. `embed-resource` only needs this as a best-effort fallback: when it
//! returns "not found", embed-resource falls back to the `RC` environment variable or
//! `rc.exe` on `PATH`, which this project provides.
//!
//! The struct layout and symbol names match the upstream crate exactly.

extern crate libc;

use libc::{c_int, wchar_t};

#[repr(C)]
pub struct Find_Result {
    pub windows_sdk_version: c_int,
    pub windows_sdk_root: *mut wchar_t,
    pub windows_sdk_um_library_path: *mut wchar_t,
    pub windows_sdk_ucrt_library_path: *mut wchar_t,
    pub vs_exe_path: *mut wchar_t,
    pub vs_library_path: *mut wchar_t,
}

#[no_mangle]
pub extern "C" fn vswhom_find_visual_studio_and_windows_sdk() -> Find_Result {
    Find_Result {
        windows_sdk_version: 0,
        windows_sdk_root: std::ptr::null_mut(),
        windows_sdk_um_library_path: std::ptr::null_mut(),
        windows_sdk_ucrt_library_path: std::ptr::null_mut(),
        vs_exe_path: std::ptr::null_mut(),
        vs_library_path: std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn vswhom_free_resources(_result: *mut Find_Result) {}
