// Windows clipboard implementation using Win32 API.
#![allow(dead_code)]

use crate::platform::backend::Clipboard;
use crate::platform::windows::ffi::*;
use std::ffi::c_void;

pub struct WinClipboard;

impl Clipboard for WinClipboard {
    fn set_text(&self, text: &str) {
        unsafe {
            if OpenClipboard(NULL) == 0 {
                return;
            }
            EmptyClipboard();

            let wide = to_wide(text);
            let size = wide.len() * 2;

            let hglobal = GlobalAlloc(GMEM_MOVEABLE, size);
            if !hglobal.is_null() {
                let ptr = GlobalLock(hglobal);
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(
                        wide.as_ptr() as *const c_void,
                        ptr,
                        wide.len() * 2,
                    );
                    GlobalUnlock(hglobal);
                    SetClipboardData(CF_UNICODETEXT, hglobal);
                } else {
                    GlobalFree(hglobal);
                }
            }

            CloseClipboard();
        }
    }

    fn get_text(&self) -> Option<String> {
        unsafe {
            if OpenClipboard(NULL) == 0 {
                return None;
            }

            if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
                CloseClipboard();
                return None;
            }

            let hglobal = GetClipboardData(CF_UNICODETEXT);
            if hglobal.is_null() {
                CloseClipboard();
                return None;
            }

            let ptr = GlobalLock(hglobal);
            if ptr.is_null() {
                CloseClipboard();
                return None;
            }

            let result = from_wide(ptr as *const u16);
            GlobalUnlock(hglobal);
            CloseClipboard();
            Some(result)
        }
    }
}
