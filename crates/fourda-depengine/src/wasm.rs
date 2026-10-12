// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The wasm32 exports: a request in linear memory, a JSON response out
//! ([`crate::api::call`]). The only `unsafe` in the crate, and only on wasm32.
//!
//! Host protocol:
//! 1. `ptr = depengine_alloc(len)`, then write the UTF-8 request at `ptr`.
//! 2. `packed = depengine_call(ptr, len)`: the request buffer is consumed;
//!    the response sits at `packed >> 32` and is `packed & 0xffff_ffff` bytes.
//! 3. Read it, then `depengine_free(response_ptr, response_len)`.
#![allow(unsafe_code)]

use std::ptr::slice_from_raw_parts_mut;

/// A zeroed buffer of `len` bytes for the host to write a request into.
#[no_mangle]
pub extern "C" fn depengine_alloc(len: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; len].into_boxed_slice()).cast::<u8>()
}

/// Free a buffer from [`depengine_alloc`] or a response from [`depengine_call`].
///
/// # Safety
/// `ptr` and `len` must be exactly a buffer this module handed out and not
/// yet freed.
#[no_mangle]
pub unsafe extern "C" fn depengine_free(ptr: *mut u8, len: usize) {
    drop(Box::from_raw(slice_from_raw_parts_mut(ptr, len)));
}

/// Answer the request in `ptr[..len]` (consuming that buffer) and return the
/// response's address and length packed into one `u64`.
///
/// # Safety
/// `ptr` and `len` must be exactly a buffer from [`depengine_alloc`], not
/// yet freed.
#[no_mangle]
pub unsafe extern "C" fn depengine_call(ptr: *mut u8, len: usize) -> u64 {
    let request = Box::from_raw(slice_from_raw_parts_mut(ptr, len));
    let response = crate::api::call(&String::from_utf8_lossy(&request)).into_bytes();
    let response = response.into_boxed_slice();
    let out_len = response.len() as u64;
    let out_ptr = Box::into_raw(response).cast::<u8>() as usize as u64;
    (out_ptr << 32) | out_len
}
