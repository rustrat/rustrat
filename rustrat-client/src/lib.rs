use error::Result;
use runtime::executor::Environment;
use runtime::CommonUtils;

use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::{fs::File, io::Read};

pub mod connector;
pub mod error;
pub mod ffi;
pub mod runtime;

#[macro_use]
extern crate bitflags;

extern crate num_derive;

// TODO TODO DLL (and EXE) in own crates

// TODO make more robust, attempt to parse arguments somewha
#[no_mangle]
pub unsafe extern "C" fn rundll_run(
    _hwnd: *const c_void,
    _hinstance: *const c_void,
    cmdline: *const c_char,
    _cmdshow: i32,
) {
    let cmdstr: &str = match CStr::from_ptr(cmdline).to_str() {
        Ok(str) => str,
        Err(_) => return,
    };

    let args: Vec<&str> = cmdstr.split_whitespace().collect();
    assert!(args.len() >= 2);

    run_webassembly_file(args[0], args[1], |out| {
        println!("Print called from wasm with following value: {}", out);
    })
    .unwrap();
}

#[no_mangle]
pub extern "C" fn run(path: *const c_char, fn_name: *const c_char) -> i32 {
    let path_str: &str = unsafe {
        match CStr::from_ptr(path).to_str() {
            Ok(str) => str,
            Err(_) => return -1,
        }
    };

    let fn_name_str: &str = unsafe {
        match CStr::from_ptr(fn_name).to_str() {
            Ok(str) => str,
            Err(_) => return -1,
        }
    };

    run_webassembly_file(path_str, fn_name_str, |out| {
        println!("Print called from wasm with following value: {}", out);
    })
    .unwrap_or(-1)
}

pub fn run_webassembly<T: AsRef<[u8]>, F: Fn(&str) + 'static>(
    wasm: T,
    fn_name: &str,
    print_closure: F,
) -> Result<i32> {
    let common_utils = CommonUtils::new();

    Environment::oneshot(wasm.as_ref(), common_utils, print_closure, fn_name)
}

pub fn run_webassembly_file<F: Fn(&str) + 'static>(
    path: &str,
    fn_name: &str,
    print_closure: F,
) -> Result<i32> {
    let mut file = File::open(path)?;
    let mut file_buffer = Vec::new();
    file.read_to_end(&mut file_buffer)?;

    run_webassembly(file_buffer, fn_name, print_closure)
}
