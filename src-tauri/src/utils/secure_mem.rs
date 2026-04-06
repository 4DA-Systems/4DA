// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Secure memory utilities — prevents sensitive data from being paged to swap.
//!
//! Uses platform-native APIs to lock memory pages containing secrets:
//! - Windows: `VirtualLock`
//! - Unix: `mlock`
//!
//! Falls back gracefully when locking fails (insufficient privileges,
//! resource limits) — logs a warning but doesn't crash.

// Intentional unsafe: FFI calls to OS memory-locking APIs (mlock / VirtualLock).
// These are safe to call on any valid memory region — they only affect paging behavior.

/// Lock a byte slice in physical memory, preventing it from being paged to swap.
///
/// This is best-effort: if the OS denies the request (e.g., insufficient
/// privileges or `RLIMIT_MEMLOCK` on Linux), a warning is logged and the
/// function returns `false`. The data remains usable but may be swapped.
#[allow(unsafe_code, unused_variables, dead_code)]
pub(crate) fn lock_memory(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }

    #[cfg(unix)]
    {
        // SAFETY: mlock is safe to call on any valid memory region.
        // It pins the pages in physical RAM until munlock or process exit.
        let result = unsafe { libc::mlock(data.as_ptr() as *const libc::c_void, data.len()) };
        if result != 0 {
            tracing::warn!(
                target: "4da::security",
                errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1),
                len = data.len(),
                "mlock failed — sensitive data may be swapped to disk"
            );
            return false;
        }
        true
    }

    #[cfg(windows)]
    {
        // SAFETY: VirtualLock is safe to call on any valid memory region.
        // It prevents the pages from being paged out to the swap file.
        let result = unsafe {
            windows_sys::Win32::System::Memory::VirtualLock(
                data.as_ptr() as *mut core::ffi::c_void,
                data.len(),
            )
        };
        if result == 0 {
            tracing::warn!(
                target: "4da::security",
                error = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1),
                len = data.len(),
                "VirtualLock failed — sensitive data may be swapped to disk"
            );
            return false;
        }
        true
    }

    #[cfg(not(any(unix, windows)))]
    {
        tracing::warn!(target: "4da::security", "Memory locking not available on this platform");
        false
    }
}

/// Unlock a previously locked byte slice, allowing it to be paged normally.
#[allow(unsafe_code, unused_variables, dead_code)]
pub(crate) fn unlock_memory(data: &[u8]) {
    if data.is_empty() {
        return;
    }

    #[cfg(unix)]
    {
        // SAFETY: munlock is safe on any valid region — it simply marks
        // pages as eligible for paging again.
        unsafe {
            libc::munlock(data.as_ptr() as *const libc::c_void, data.len());
        }
    }

    #[cfg(windows)]
    {
        // SAFETY: VirtualUnlock is safe on any valid region — it reverses
        // a previous VirtualLock.
        unsafe {
            windows_sys::Win32::System::Memory::VirtualUnlock(
                data.as_ptr() as *mut core::ffi::c_void,
                data.len(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_empty_is_ok() {
        assert!(lock_memory(&[]));
    }

    #[test]
    fn test_lock_and_unlock_small_buffer() {
        let data = b"test secret key material";
        // May or may not succeed depending on platform/privileges
        let locked = lock_memory(data);
        // Either way, unlock should not panic
        unlock_memory(data);
        // On CI/test environments, locking may fail — that's OK
        let _ = locked;
    }
}
