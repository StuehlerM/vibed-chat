// Windows dialogs. Callers must release application borrows before entering.
#![allow(dead_code)]

use crate::platform::backend::Dialogs;
use crate::platform::windows::ffi::*;

pub struct WinDialogs(pub HWND);

impl Dialogs for WinDialogs {
    fn pick_folder(&self, title: &str) -> Option<String> {
        // SAFETY: OLE is initialized once on the UI thread. All buffers and
        // strings outlive the modal call, and the returned PIDL is freed once.
        unsafe {
            let title_wide = to_wide(title);
            let mut display_name = vec![0u16; 260];
            let mut bi = BROWSEINFOW {
                hwndOwner: self.0,
                pidlRoot: std::ptr::null(),
                pszDisplayName: display_name.as_mut_ptr(),
                lpszTitle: title_wide.as_ptr(),
                ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_NONEWFOLDERBUTTON,
                lpfn: None,
                lParam: 0,
                iImage: 0,
            };
            let pidl = SHBrowseForFolderW(&mut bi);
            if pidl.is_null() {
                return None;
            }
            let mut path = vec![0u16; 260];
            let ok = SHGetPathFromIDListW(pidl, path.as_mut_ptr());
            CoTaskMemFree(pidl);
            if ok == 0 {
                return None;
            }
            from_wide(&path)
        }
    }

    fn confirm(&self, title: &str, message: &str) -> bool {
        // SAFETY: owner belongs to this UI thread; strings live across the call.
        unsafe {
            MessageBoxW(
                self.0,
                to_wide(message).as_ptr(),
                to_wide(title).as_ptr(),
                MB_YESNO | MB_ICONWARNING,
            ) == IDYES as i32
        }
    }

    fn show_error(&self, title: &str, message: &str) {
        // SAFETY: owner belongs to this UI thread; strings live across the call.
        unsafe {
            MessageBoxW(
                self.0,
                to_wide(message).as_ptr(),
                to_wide(title).as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
