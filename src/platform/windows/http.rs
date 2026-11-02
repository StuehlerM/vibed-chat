// Asynchronous WinHTTP transport with worker-owned close/cancellation. Callbacks
// report completions only; no GUI state or borrowed request buffers cross threads.
#![allow(dead_code)]

use super::ffi::*;
use crate::responses::{HttpRequest, HttpResponse, HttpTransport};
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct WinHttpTransport;
impl WinHttpTransport {
    pub fn new() -> Self {
        Self
    }
}
struct InternetHandle(HINTERNET);
impl Drop for InternetHandle {
    fn drop(&mut self) {
        // SAFETY: sole owner; request children are closed before parents.
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}
fn os_error(operation: &str) -> String {
    format!("{operation}: {}", std::io::Error::last_os_error())
}
fn checked_handle(handle: HINTERNET, operation: &str) -> Result<InternetHandle, String> {
    if handle.is_null() {
        Err(os_error(operation))
    } else {
        Ok(InternetHandle(handle))
    }
}
#[derive(Debug)]
enum Completion {
    Sent,
    Headers,
    Read(usize),
    Failed(u32),
    Closed,
}
struct CallbackState {
    events: Sender<Completion>,
    read: UnsafeCell<[u8; 8192]>,
    body: Vec<u8>,
}
// SAFETY: only WinHTTP writes `read`, with exactly one outstanding read. The
// worker reads it only after READ_COMPLETE and finishes consuming the slice
// before starting another read. Callbacks never access it. The OS-owned Arc
// keeps body/read storage alive until HANDLE_CLOSING, even after cancellation.
unsafe impl Sync for CallbackState {}

unsafe extern "system" fn status_callback(
    _handle: HINTERNET,
    context: usize,
    status: DWORD,
    info: *mut c_void,
    length: DWORD,
) {
    if context == 0 {
        return;
    }
    let pointer = context as *const CallbackState;
    // SAFETY: one Arc count is transferred to WinHTTP at registration and is
    // released only by its final HANDLE_CLOSING callback. Each callback takes
    // a temporary count so a receiver waking during send cannot free its state.
    Arc::increment_strong_count(pointer);
    let state = Arc::from_raw(pointer);
    let event = match status {
        WINHTTP_CALLBACK_STATUS_SENDREQUEST_COMPLETE => Some(Completion::Sent),
        WINHTTP_CALLBACK_STATUS_HEADERS_AVAILABLE => Some(Completion::Headers),
        WINHTTP_CALLBACK_STATUS_READ_COMPLETE => Some(Completion::Read(length as usize)),
        WINHTTP_CALLBACK_STATUS_REQUEST_ERROR => {
            if !info.is_null() && length as usize >= std::mem::size_of::<WINHTTP_ASYNC_RESULT>() {
                Some(Completion::Failed(
                    (*(info as *const WINHTTP_ASYNC_RESULT)).error,
                ))
            } else {
                Some(Completion::Failed(0))
            }
        }
        WINHTTP_CALLBACK_STATUS_HANDLE_CLOSING => Some(Completion::Closed),
        _ => None,
    };
    if let Some(event) = event {
        let _ = state.events.send(event);
    }
    if status == WINHTTP_CALLBACK_STATUS_HANDLE_CLOSING {
        drop(Arc::from_raw(pointer)); // release the single OS-owned count
    }
}
struct RequestHandle {
    handle: HINTERNET,
    state: Arc<CallbackState>,
    completions: Receiver<Completion>,
}
impl RequestHandle {
    fn wait(&self, cancelled: &AtomicBool, start: Instant) -> Result<Completion, String> {
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("HTTP request cancelled".to_string());
            }
            if start.elapsed() > Duration::from_secs(120) {
                return Err("HTTP overall deadline exceeded".to_string());
            }
            match self.completions.recv_timeout(Duration::from_millis(25)) {
                Ok(Completion::Failed(code)) => {
                    return Err(format!("WinHTTP asynchronous operation: OS error {code}"))
                }
                Ok(event) => return Ok(event),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err("WinHTTP callback channel closed".to_string()),
            }
        }
    }
}
impl Drop for RequestHandle {
    fn drop(&mut self) {
        // SAFETY: only the worker closes this request, after asynchronous API
        // initiation has returned. Close aborts outstanding operations. Buffers
        // stay owned by the OS Arc until the final callback, never by this wait.
        unsafe {
            WinHttpCloseHandle(self.handle);
        }
        let deadline = Instant::now() + Duration::from_millis(250);
        while Instant::now() < deadline {
            if matches!(
                self.completions.recv_timeout(Duration::from_millis(25)),
                Ok(Completion::Closed)
            ) {
                break;
            }
        }
    }
}
impl HttpTransport for WinHttpTransport {
    fn post_stream(
        &self,
        request: &HttpRequest<'_>,
        on_response: &mut dyn FnMut(&HttpResponse) -> Result<(), String>,
        on_chunk: &mut dyn FnMut(&[u8]),
    ) -> Result<(), String> {
        let HttpRequest {
            host,
            path,
            headers,
            body,
            cancelled,
        } = *request;
        if cancelled.load(Ordering::Acquire) {
            return Err("HTTP request cancelled".to_string());
        }
        if host.contains(['\r', '\n', '\0'])
            || path.contains(['\r', '\n', '\0'])
            || headers.iter().any(|(key, value)| {
                key.contains(['\r', '\n', '\0', ':']) || value.contains(['\r', '\n', '\0'])
            })
        {
            return Err("Invalid HTTP header/target".to_string());
        }
        let body_size = DWORD::try_from(body.len()).map_err(|_| "HTTP body exceeds DWORD range")?;
        let header_block: String = headers
            .iter()
            .map(|(key, value)| format!("{key}: {value}\r\n"))
            .collect();
        let wide_headers = to_wide(&header_block);
        let header_size =
            DWORD::try_from(wide_headers.len() - 1).map_err(|_| "HTTP headers too large")?;
        let start = Instant::now();
        // SAFETY: setup/query calls use owned strings and checked lengths.
        // Async body/read storage is held by an Arc until HANDLE_CLOSING. No
        // second operation begins before the prior completion is consumed.
        unsafe {
            let session = checked_handle(
                WinHttpOpen(
                    to_wide("vibe-chat/1.0").as_ptr(),
                    4,
                    std::ptr::null(),
                    std::ptr::null(),
                    WINHTTP_FLAG_ASYNC,
                ),
                "WinHttpOpen",
            )?;
            if WinHttpSetTimeouts(session.0, 10_000, 10_000, 30_000, 30_000) == 0 {
                return Err(os_error("WinHttpSetTimeouts"));
            }
            let connection = checked_handle(
                WinHttpConnect(session.0, to_wide(host).as_ptr(), 443, 0),
                "WinHttpConnect",
            )?;
            let request = checked_handle(
                WinHttpOpenRequest(
                    connection.0,
                    to_wide("POST").as_ptr(),
                    to_wide(path).as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    WINHTTP_FLAG_SECURE | WINHTTP_FLAG_REFRESH,
                ),
                "WinHttpOpenRequest",
            )?;
            if WinHttpAddRequestHeaders(
                request.0,
                wide_headers.as_ptr(),
                header_size,
                WINHTTP_ADDREQ_FLAG_ADD | WINHTTP_ADDREQ_FLAG_REPLACE,
            ) == 0
            {
                return Err(os_error("WinHttpAddRequestHeaders"));
            }
            let (events, completions) = channel();
            let state = Arc::new(CallbackState {
                events,
                read: UnsafeCell::new([0; 8192]),
                body: body.as_bytes().to_vec(),
            });
            let context = Arc::into_raw(state.clone()) as usize;
            let mut context_value = context;
            if WinHttpSetOption(
                request.0,
                WINHTTP_OPTION_CONTEXT_VALUE,
                (&mut context_value as *mut usize).cast(),
                std::mem::size_of::<usize>() as DWORD,
            ) == 0
            {
                let error = os_error("WinHttpSetOption(context)");
                drop(Arc::from_raw(context as *const CallbackState));
                return Err(error);
            }
            let flags = WINHTTP_CALLBACK_STATUS_SENDREQUEST_COMPLETE
                | WINHTTP_CALLBACK_STATUS_HEADERS_AVAILABLE
                | WINHTTP_CALLBACK_STATUS_READ_COMPLETE
                | WINHTTP_CALLBACK_STATUS_REQUEST_ERROR
                | WINHTTP_CALLBACK_STATUS_HANDLE_CLOSING;
            if WinHttpSetStatusCallback(request.0, Some(status_callback), flags, 0) as isize == -1 {
                let error = os_error("WinHttpSetStatusCallback");
                drop(Arc::from_raw(context as *const CallbackState));
                return Err(error);
            }
            // Ownership transfers exactly once to RequestHandle after callback registration.
            let handle = request.0;
            std::mem::forget(request);
            let request = RequestHandle {
                handle,
                state,
                completions,
            };
            if WinHttpSendRequest(
                handle,
                std::ptr::null(),
                0,
                request.state.body.as_ptr() as *mut c_void,
                body_size,
                body_size,
                context,
            ) == 0
            {
                return Err(os_error("WinHttpSendRequest"));
            }
            if !matches!(request.wait(cancelled, start)?, Completion::Sent) {
                return Err("Unexpected send completion".to_string());
            }
            if WinHttpReceiveResponse(handle, std::ptr::null_mut()) == 0 {
                return Err(os_error("WinHttpReceiveResponse"));
            }
            if !matches!(request.wait(cancelled, start)?, Completion::Headers) {
                return Err("Unexpected header completion".to_string());
            }
            let mut status = 0u32;
            let mut status_len = std::mem::size_of::<DWORD>() as DWORD;
            if WinHttpQueryHeaders(
                handle,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                std::ptr::null(),
                (&mut status as *mut DWORD).cast(),
                &mut status_len,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(os_error("WinHttpQueryHeaders(status)"));
            }
            let mut content_type = [0u16; 1024];
            let mut type_len = (content_type.len() * 2) as DWORD;
            if WinHttpQueryHeaders(
                handle,
                1,
                std::ptr::null(),
                content_type.as_mut_ptr().cast(),
                &mut type_len,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(os_error("WinHttpQueryHeaders(content-type)"));
            }
            let metadata = HttpResponse {
                status: u16::try_from(status).map_err(|_| "Invalid HTTP status")?,
                content_type: from_wide(&content_type).ok_or("Invalid content-type")?,
            };
            on_response(&metadata)?;
            let mut total = 0usize;
            loop {
                if cancelled.load(Ordering::Acquire) {
                    return Err("HTTP request cancelled".to_string());
                }
                if WinHttpReadData(
                    handle,
                    request.state.read.get().cast(),
                    8192,
                    std::ptr::null_mut(),
                ) == 0
                {
                    return Err(os_error("WinHttpReadData"));
                }
                let Completion::Read(count) = request.wait(cancelled, start)? else {
                    return Err("Unexpected read completion".to_string());
                };
                if count == 0 {
                    return Ok(());
                }
                // No native read is outstanding while this shared slice is live.
                let bytes = (&*request.state.read.get())
                    .get(..count)
                    .ok_or("Invalid HTTP read size")?;
                total = total
                    .checked_add(count)
                    .ok_or("HTTP response size overflow")?;
                if total > 32 * 1024 * 1024 {
                    return Err("HTTP response exceeds 32 MiB".to_string());
                }
                on_chunk(bytes);
                if metadata.status != 200 && total >= 8192 {
                    return Ok(());
                }
            }
        }
    }
}
