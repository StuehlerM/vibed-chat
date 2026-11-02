// Windows clipboard implementation with explicit ownership and bounded reads.
#![allow(dead_code)]

use crate::platform::backend::Clipboard;
use crate::platform::windows::ffi::*;

pub struct WinClipboard(pub HWND);

struct OpenClipboardGuard;
impl Drop for OpenClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: this guard is created only after successfully opening the clipboard.
        unsafe {
            CloseClipboard();
        }
    }
}

struct GlobalMemory(HGLOBAL);
impl Drop for GlobalMemory {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns the allocation until a successful transfer.
        unsafe {
            GlobalFree(self.0);
        }
    }
}

struct LockedMemory(HGLOBAL);
impl Drop for LockedMemory {
    fn drop(&mut self) {
        // SAFETY: a guard exists only for a successful GlobalLock.
        unsafe {
            GlobalUnlock(self.0);
        }
    }
}

impl Clipboard for WinClipboard {
    fn set_text(&self, text: &str) -> Result<(), String> {
        if self.0.is_null() {
            return Err("Clipboard has no owner window".to_string());
        }
        if text.contains('\0') {
            return Err("Clipboard text contains NUL".to_string());
        }
        let wide = to_wide(text);
        let size = wide
            .len()
            .checked_mul(2)
            .ok_or("Clipboard text is too large")?;
        // SAFETY: allocations are guarded; the writable lock is at least `size`
        // bytes. Clipboard ownership transfers only when SetClipboardData succeeds.
        unsafe {
            let memory = GlobalMemory(GlobalAlloc(GMEM_MOVEABLE, size));
            if memory.0.is_null() {
                return Err("Clipboard allocation failed".to_string());
            }
            {
                let ptr = GlobalLock(memory.0);
                if ptr.is_null() {
                    return Err("Clipboard memory lock failed".to_string());
                }
                let _lock = LockedMemory(memory.0);
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr.cast::<u16>(), wide.len());
            }
            if OpenClipboard(self.0) == 0 {
                return Err("Clipboard is busy".to_string());
            }
            let _open = OpenClipboardGuard;
            if EmptyClipboard() == 0 || SetClipboardData(CF_UNICODETEXT, memory.0).is_null() {
                return Err("Clipboard transfer failed".to_string());
            }
            std::mem::forget(memory); // the system now owns the allocation
        }
        Ok(())
    }

    fn get_text(&self) -> Option<String> {
        // SAFETY: clipboard remains open while borrowed data is locked and read.
        // The slice is bounded by GlobalSize, aligned by GlobalAlloc, and capped.
        unsafe {
            if OpenClipboard(self.0) == 0 {
                return None;
            }
            let _open = OpenClipboardGuard;
            if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
                return None;
            }
            let memory = GetClipboardData(CF_UNICODETEXT);
            if memory.is_null() {
                return None;
            }
            let size = GlobalSize(memory);
            if size == 0 || size % 2 != 0 || size > 16 * 1024 * 1024 {
                return None;
            }
            let ptr = GlobalLock(memory);
            if ptr.is_null() {
                return None;
            }
            let _lock = LockedMemory(memory);
            from_wide(std::slice::from_raw_parts(ptr.cast::<u16>(), size / 2))
        }
    }
}
