use error::Result;
use ffi::wrappers;
use wasm::WasmEnvironment;

use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::{fs::File, io::Read};

pub mod error;
pub mod ffi;
pub mod wasm;

extern crate num_derive;

// TODO make more robust, attempt to parse arguments somewhat
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

    let args: Vec<&str> = cmdstr.split(" ").collect();
    assert!(args.len() >= 2);

    run_webassembly(args[0], args[1]).unwrap();
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

    return match run_webassembly(path_str, fn_name_str) {
        Ok(result) => result,
        Err(_) => -1,
    };
}

pub fn run_webassembly(path: &str, fn_name: &str) -> Result<i32> {
    let mut file = File::open(path)?;
    let mut file_buffer = Vec::new();
    file.read_to_end(&mut file_buffer)?;

    wrappers::setup_fn_table();

    let env = WasmEnvironment::new(10 * 10 * 1024)?;
    let mut wasm_module = env.load_module(&file_buffer)?;

    match wasm_module.link_function::<(u32,), i32>("rustrat", "has_fn", wrappers::has_fn_wrapper) {
        Ok(_) | Err(wasm3::error::Error::FunctionNotFound) => {}
        Err(e) => return Err(error::Error::from(e)),
    };

    match wasm_module.link_function::<(u32, u32, u32, i32, u32), i32>(
        "rustrat",
        "register_fn",
        wrappers::register_fn_wrapper,
    ) {
        Ok(_) | Err(wasm3::error::Error::FunctionNotFound) => {}
        Err(e) => return Err(error::Error::from(e)),
    };

    match wasm_module.link_function::<(u32, u32), u32>(
        "rustrat",
        "call_fn_u32",
        wrappers::call_fn_wrapper,
    ) {
        Ok(_) | Err(wasm3::error::Error::FunctionNotFound) => {}
        Err(e) => return Err(error::Error::from(e)),
    };

    match wasm_module.link_function::<(u32, u32), u64>(
        "rustrat",
        "call_fn_u64",
        wrappers::call_fn_wrapper,
    ) {
        Ok(_) | Err(wasm3::error::Error::FunctionNotFound) => {}
        Err(e) => return Err(error::Error::from(e)),
    };

    let func = wasm_module.find_function::<(), i32>(fn_name)?;

    Ok(func.call()?)
}
