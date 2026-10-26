// Windows dialog implementations using Win32 API.
#![allow(dead_code)]

use crate::platform::backend::Dialogs;
use crate::platform::windows::ffi::*;

pub struct WinDialogs;

impl Dialogs for WinDialogs {
    fn pick_folder(&self, title: &str) -> Option<String> {
        unsafe {
            let _ = OleInitialize(std::ptr::null_mut());

            let title_wide = to_wide(title);
            let mut display_name: Vec<u16> = vec![0; 260];

            let mut bi = BROWSEINFOW {
                hwndOwner: std::ptr::null_mut(),
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

            let mut path_buf: Vec<u16> = vec![0; 260];
            let ok = SHGetPathFromIDListW(pidl, path_buf.as_mut_ptr());
            CoTaskMemFree(pidl as *mut std::ffi::c_void);

            if ok == 0 {
                return None;
            }

            Some(from_wide(path_buf.as_ptr()))
        }
    }

    fn confirm(&self, title: &str, message: &str) -> bool {
        unsafe {
            let msg = to_wide(message);
            let t = to_wide(title);
            let result = MessageBoxW(
                std::ptr::null_mut(),
                msg.as_ptr(),
                t.as_ptr(),
                MB_YESNO | MB_ICONWARNING,
            );
            result == IDYES as i32
        }
    }

    fn show_error(&self, title: &str, message: &str) {
        unsafe {
            let msg = to_wide(message);
            let t = to_wide(title);
            MessageBoxW(
                std::ptr::null_mut(),
                msg.as_ptr(),
                t.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
