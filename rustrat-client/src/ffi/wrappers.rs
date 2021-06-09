use crate::error::*;

use std::cell::RefCell;
use std::ffi::CStr;
use std::rc::Rc;

use super::FnTable;

// TODO combine the two link functions?
pub fn link_print_closure<F: Fn(&str) + 'static>(
    module: &mut wasm3::Module,
    print_closure: F,
) -> Result<()> {
    module
        .link_closure(
            "rustrat",
            "print",
            move |cc, (str_ptr,): (u32,)| match wasm_cstr_to_str(cc, str_ptr) {
                Ok(output) => {
                    print_closure(output);
                    0
                }
                Err(_) => {
                    log::error!("Invalid string given to print from WASM.");
                    1
                }
            },
        )
        .or_else(not_found_is_okay)?;

    Ok(())
}

pub fn link_ffi_bindings(
    module: &mut wasm3::Module,
    fn_table_ref: &Rc<RefCell<FnTable>>,
) -> Result<()> {
    let fn_table_rc = fn_table_ref.clone();
    module
        .link_closure(
            "rustrat",
            "has_fn",
            move |cc, (str_ptr,): (u32,)| match wasm_cstr_to_str(cc, str_ptr) {
                Ok(fn_name) => {
                    let fn_table = fn_table_rc.borrow();
                    if fn_table.has_fn(fn_name.to_string()) {
                        1
                    } else {
                        0
                    }
                }
                Err(_) => {
                    log::error!("Invalid string given to has_fn from WASM.");
                    0
                }
            },
        )
        .or_else(not_found_is_okay)?;

    let fn_table_rc = fn_table_ref.clone();
    module
        .link_closure(
            "rustrat",
            "register_fn",
            move |cc,
                  (fn_str_ptr, library_str_ptr, n_args, return_type_int, arg_type_ints_ptr): (
                u32,
                u32,
                u32,
                i32,
                u32,
            )| {
                let fn_name = match wasm_cstr_to_str(cc, fn_str_ptr) {
                    Ok(fn_name) => fn_name,
                    Err(_) => {
                        log::error!("Error when attempting to convert function name to string.");
                        return 0;
                    }
                };
                let library_name = match wasm_cstr_to_str(cc, library_str_ptr) {
                    Ok(library_name) => library_name,
                    Err(_) => {
                        log::error!("Error when attempting to convert library name to string.");
                        return 0;
                    }
                };

                let mut arguments = Vec::new();
                unsafe {
                    let arguments_ptr = (cc.memory() as *const _ as *const i8)
                        .offset(arg_type_ints_ptr as isize)
                        as *const i32;
                    for i in 0..n_args as isize {
                        arguments.push(*arguments_ptr.offset(i));
                    }
                }

                let mut fn_table = fn_table_rc.borrow_mut();

                match fn_table.register_fn(
                    fn_name.to_string(),
                    library_name.to_string(),
                    return_type_int,
                    &arguments,
                ) {
                    Ok(_) => 1,
                    Err(err) => {
                        log::error!("Error when registering function: {:?}", err);
                        0
                    }
                }
            },
        )
        .or_else(not_found_is_okay)?;

    let fn_table_rc = fn_table_ref.clone();
    module
        .link_closure(
            "rustrat",
            "call_fn_u32",
            move |cc, (fn_str_ptr, args_ptr): (u32, u32)| -> u32 {
                let fn_table = fn_table_rc.borrow();

                unsafe {
                    match do_ffi_call(&fn_table, cc, fn_str_ptr, args_ptr) {
                        Ok(result) => result,
                        Err(err) => {
                            log::debug!("Unable to call ffi call: {:?}", err);
                            0
                        }
                    }
                }
            },
        )
        .or_else(not_found_is_okay)?;

    let fn_table_rc = fn_table_ref.clone();
    module
        .link_closure(
            "rustrat",
            "call_fn_u64",
            move |cc, (fn_str_ptr, args_ptr): (u32, u32)| -> u64 {
                let fn_table = fn_table_rc.borrow();

                unsafe {
                    match do_ffi_call(&fn_table, cc, fn_str_ptr, args_ptr) {
                        Ok(result) => result,
                        Err(err) => {
                            log::debug!("Unable to call ffi call: {:?}", err);
                            0
                        }
                    }
                }
            },
        )
        .or_else(not_found_is_okay)?;

    Ok(())
}

unsafe fn do_ffi_call<R>(
    fn_table: &FnTable,
    cc: &wasm3::CallContext,
    fn_str_ptr: u32,
    args_ptr: u32,
) -> Result<R> {
    let fn_name = wasm_cstr_to_str(cc, fn_str_ptr)?;
    log::debug!("Running {} through FFI binding from WASM", fn_name);
    let function = fn_table
        .0
        .get(fn_name)
        .ok_or_else(|| Error::FunctionDoesNotExist(fn_name.to_string()))?;

    let mut arguments = Vec::with_capacity(function.n_args);
    let arguments_ptr_array =
        (cc.memory() as *const _ as *const i8).offset(args_ptr as isize) as *const u32;

    for i in 0..function.n_args as isize {
        // WASM pointer (32-bit) to the location in WASM's memory the value is stored
        let argument_wasm_ptr = *arguments_ptr_array.offset(i) as isize;
        // The native pointer to where the value is stored
        let argument_ptr =
            (cc.memory() as *const _ as *const i8).offset(argument_wasm_ptr) as *const _;

        // TODO make it easier to work with native pointers (memory not in WASM)
        arguments.push(match function.arg_types[i as usize] {
            crate::ffi::FfiType::POINTER => {
                // This is a pointer to WASM memory(?)
                libffi::middle::arg(
                    &((cc.memory() as *const _ as *const i8)
                        .offset(*(argument_ptr as *const i32) as isize)
                        as *mut std::ffi::c_void),
                )
            }
            crate::ffi::FfiType::DOUBLE => libffi::middle::arg(&(*(argument_ptr as *mut f64))),
            crate::ffi::FfiType::FLOAT => libffi::middle::arg(&(*(argument_ptr as *mut f32))),
            crate::ffi::FfiType::LONGDOUBLE => libffi::middle::arg(&(*(argument_ptr as *mut f64))),
            crate::ffi::FfiType::SINT16 => libffi::middle::arg(&(*(argument_ptr as *mut i16))),
            crate::ffi::FfiType::SINT32 => libffi::middle::arg(&(*(argument_ptr as *mut i32))),
            crate::ffi::FfiType::SINT64 => libffi::middle::arg(&(*(argument_ptr as *mut i64))),
            crate::ffi::FfiType::SINT8 => libffi::middle::arg(&(*(argument_ptr as *mut i8))),
            crate::ffi::FfiType::UINT16 => libffi::middle::arg(&(*(argument_ptr as *mut u16))),
            crate::ffi::FfiType::UINT32 => libffi::middle::arg(&(*(argument_ptr as *mut u32))),
            crate::ffi::FfiType::UINT64 => libffi::middle::arg(&(*(argument_ptr as *mut u64))),
            crate::ffi::FfiType::UINT8 => libffi::middle::arg(&(*(argument_ptr as *mut u8))),
            crate::ffi::FfiType::VOID => libffi::middle::arg(&0), // TODO what to do here? Panic?
        });
    }

    // TODO make sure return type is correct? Is this possible with generics?
    Ok(function.call(&arguments))
}

fn wasm_cstr_to_str(cc: &wasm3::CallContext, str_ptr: u32) -> Result<&str> {
    let out;
    unsafe {
        let mem = cc.memory() as *const _ as *const i8;
        let str_ptr = mem.offset(str_ptr as isize);
        out = CStr::from_ptr(str_ptr).to_str()?;
    }

    Ok(out)
}

fn not_found_is_okay(err: wasm3::error::Error) -> Result<()> {
    match err {
        wasm3::error::Error::FunctionNotFound => Ok(()),
        _ => Err(Error::from(err)),
    }
}
