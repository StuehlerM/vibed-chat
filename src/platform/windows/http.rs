// WinHTTP HTTPS transport for the Responses API client.
// WinHTTP owns TLS, certificate validation, and proxy resolution, so this
// file contains no cryptography. Handles are closed on every exit path.
// FFI declarations live in ffi.rs per project convention.
#![allow(dead_code)]

use crate::http::ChunkDecoder;
use crate::responses::HttpTransport;
use std::ffi::c_void;

use super::ffi::*;

/// WINHTTP_QUERY_TRANSFER_ENCODING = 45 (winhttp.h).
const WINHTTP_QUERY_TRANSFER_ENCODING: DWORD = 45;

pub struct WinHttpTransport;

impl WinHttpTransport {
    pub fn new() -> Self {
        WinHttpTransport
    }
}

impl HttpTransport for WinHttpTransport {
    fn post_stream(
        &self,
        host: &str,
        path: &str,
        headers: &[(String, String)],
        body: &str,
        on_body_bytes: &mut dyn FnMut(&[u8]),
    ) -> Result<u16, String> {
        unsafe { post_stream_impl(host, path, headers, body, on_body_bytes) }
    }
}

/// Query a string response header. Returns None when absent.
unsafe fn query_header_string(request: HINTERNET, info_level: DWORD) -> Option<String> {
    // First call gets the required buffer size in bytes.
    let mut size: DWORD = 0;
    let rc = WinHttpQueryHeaders(
        request,
        info_level,
        std::ptr::null(),
        std::ptr::null_mut(),
        &mut size,
        std::ptr::null_mut(),
    );
    if rc == 0 || size == 0 {
        return None;
    }
    // size includes the null terminator; allocate u16 slots.
    let mut buf = vec![0u16; (size as usize / 2).max(1)];
    let rc = WinHttpQueryHeaders(
        request,
        info_level,
        std::ptr::null(),
        buf.as_mut_ptr() as *mut c_void,
        &mut size,
        std::ptr::null_mut(),
    );
    if rc == 0 {
        return None;
    }
    // Trim trailing NULs.
    while buf.last() == Some(&0) {
        buf.pop();
    }
    Some(String::from_utf16_lossy(&buf))
}

unsafe fn post_stream_impl(
    host: &str,
    path: &str,
    headers: &[(String, String)],
    body: &str,
    on_body_bytes: &mut dyn FnMut(&[u8]),
) -> Result<u16, String> {
    let agent = to_wide("vibe-chat/1.0");
    let session = WinHttpOpen(
        agent.as_ptr(),
        WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
        std::ptr::null(),
        std::ptr::null(),
        0,
    );
    if session.is_null() {
        return Err("WinHttpOpen failed".to_string());
    }

    let host_wide = to_wide(host);
    let connect = WinHttpConnect(session, host_wide.as_ptr(), 443, 0);
    if connect.is_null() {
        WinHttpCloseHandle(session);
        return Err("WinHttpConnect failed".to_string());
    }

    let path_wide = to_wide(path);
    // Null verb defaults to GET; pass "POST" explicitly.
    let verb_wide = to_wide("POST");
    let request = WinHttpOpenRequest(
        connect,
        verb_wide.as_ptr(),
        path_wide.as_ptr(),
        std::ptr::null(),
        std::ptr::null(),
        std::ptr::null_mut(),
        WINHTTP_FLAG_SECURE | WINHTTP_FLAG_REFRESH,
    );
    if request.is_null() {
        WinHttpCloseHandle(connect);
        WinHttpCloseHandle(session);
        return Err("WinHttpOpenRequest failed".to_string());
    }

    // One CRLF-terminated header block; Content-Length is required by HTTP/1.1.
    let mut header_block = String::new();
    for (k, v) in headers {
        header_block.push_str(&format!("{}: {}\r\n", k, v));
    }
    header_block.push_str(&format!("Content-Length: {}\r\n", body.len()));
    let headers_wide = to_wide(&header_block);
    if WinHttpAddRequestHeaders(
        request,
        headers_wide.as_ptr(),
        headers_wide.len() as DWORD - 1,
        WINHTTP_ADDREQ_FLAG_ADD | WINHTTP_ADDREQ_FLAG_REPLACE,
    ) == 0
    {
        WinHttpCloseHandle(request);
        WinHttpCloseHandle(connect);
        WinHttpCloseHandle(session);
        return Err("WinHttpAddRequestHeaders failed".to_string());
    }

    let body_bytes = body.as_bytes();
    let body_ptr: *mut c_void = body_bytes.as_ptr() as *mut c_void;
    if WinHttpSendRequest(
        request,
        std::ptr::null(),
        0,
        body_ptr,
        body_bytes.len() as DWORD,
        body_bytes.len() as DWORD,
        0,
    ) == 0
    {
        WinHttpCloseHandle(request);
        WinHttpCloseHandle(connect);
        WinHttpCloseHandle(session);
        return Err("WinHttpSendRequest failed".to_string());
    }

    if WinHttpReceiveResponse(request, std::ptr::null_mut()) == 0 {
        WinHttpCloseHandle(request);
        WinHttpCloseHandle(connect);
        WinHttpCloseHandle(session);
        return Err("WinHttpReceiveResponse failed (network or TLS error)".to_string());
    }

    let mut status: DWORD = 0;
    let mut status_len: DWORD = std::mem::size_of::<DWORD>() as DWORD;
    if WinHttpQueryHeaders(
        request,
        WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
        std::ptr::null(),
        &mut status as *mut DWORD as *mut c_void,
        &mut status_len,
        std::ptr::null_mut(),
    ) == 0
    {
        WinHttpCloseHandle(request);
        WinHttpCloseHandle(connect);
        WinHttpCloseHandle(session);
        return Err("WinHttpQueryHeaders failed".to_string());
    }

    // Chunked transfer coding is decoded here (the transport owns the
    // response headers) so the core receives plain body bytes only.
    let chunked = query_header_string(request, WINHTTP_QUERY_TRANSFER_ENCODING)
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);
    let mut decoder = ChunkDecoder::new();
    if chunked {
        decoder.enable();
    }

    // Read the body in 8 KiB chunks, streaming to the callback.
    let mut buf = vec![0u8; 8192];
    loop {
        let mut bytes_read: DWORD = 0;
        if WinHttpReadData(
            request,
            buf.as_mut_ptr(),
            buf.len() as DWORD,
            &mut bytes_read,
        ) == 0
        {
            break;
        }
        if bytes_read == 0 {
            break;
        }
        if chunked {
            decoder.extend(&buf[..bytes_read as usize]);
            let decoded = decoder.take();
            if !decoded.is_empty() {
                on_body_bytes(&decoded);
            }
        } else {
            on_body_bytes(&buf[..bytes_read as usize]);
        }
    }

    WinHttpCloseHandle(request);
    WinHttpCloseHandle(connect);
    WinHttpCloseHandle(session);
    Ok(status as u16)
}
