use std::ffi::{CStr, c_void};

use super::{FfiType, FnTable};

pub unsafe extern "C" fn has_fn_wrapper(
    _rt: ::wasm3::wasm3_sys::IM3Runtime,
    _sp: ::wasm3::wasm3_sys::m3stack_t,
    mem: *mut u8,
) -> *const core::ffi::c_void {
    use ::wasm3::WasmType as _;

    let fn_table = FnTable::new();
    let return_sp = _sp;
    // Pop string pointer from the stack and calculate pointer to raw data.
    // wasm (or rather wasm32) pointers are 32 bits.
    let fn_str_ptr = mem.offset(i32::pop_from_stack(_sp) as isize) as *const i8;
    let _sp = _sp.add(i32::SIZE_IN_SLOT_COUNT);

    let fn_str = match CStr::from_ptr(fn_str_ptr).to_str() {
        Ok(string) => {string},
        Err(_) => {
            i32::push_on_stack(0, return_sp);
            return ::wasm3::wasm3_sys::m3Err_none as _;
        },
    };

    if fn_table.has_fn(String::from(fn_str)) {
        i32::push_on_stack(1, return_sp);
    } else {
        i32::push_on_stack(0, return_sp);
    }

    ::wasm3::wasm3_sys::m3Err_none as _
}

pub unsafe extern "C" fn register_fn_wrapper(
    _rt: ::wasm3::wasm3_sys::IM3Runtime,
    _sp: ::wasm3::wasm3_sys::m3stack_t,
    mem: *mut u8,
) -> *const core::ffi::c_void {
    use ::wasm3::WasmType as _;

    let mut fn_table = FnTable::new();
    let return_sp = _sp;

    let fn_str_ptr = mem.offset(i32::pop_from_stack(_sp) as isize) as *const i8;
    let _sp = _sp.add(i32::SIZE_IN_SLOT_COUNT);
    let library_str_ptr = mem.offset(i32::pop_from_stack(_sp) as isize) as *const i8;
    let _sp = _sp.add(i32::SIZE_IN_SLOT_COUNT);
    let n_args = u32::pop_from_stack(_sp) as usize;
    let _sp = _sp.add(u32::SIZE_IN_SLOT_COUNT);
    let return_type_int = i32::pop_from_stack(_sp) as i32;
    let _sp = _sp.add(i32::SIZE_IN_SLOT_COUNT);
    let arg_type_ints_ptr = mem.offset(i32::pop_from_stack(_sp) as isize) as *const i32;
    let _sp = _sp.add(i32::SIZE_IN_SLOT_COUNT);

    let fn_str = match CStr::from_ptr(fn_str_ptr).to_str() {
        Ok(string) => {string}
        Err(_) => {
            i32::push_on_stack(0, return_sp);
            return ::wasm3::wasm3_sys::m3Err_none as _;
        }
    };

    let library_str = match CStr::from_ptr(library_str_ptr).to_str() {
        Ok(string) => {string}
        Err(_) => {
            i32::push_on_stack(0, return_sp);
            return ::wasm3::wasm3_sys::m3Err_none as _;
        }
    };

    let mut arg_type_ints: Vec<i32> = Vec::new();
    for i in 0..n_args as isize {
        arg_type_ints.push(*arg_type_ints_ptr.offset(i));
    }

    i32::push_on_stack(
        match fn_table.register_fn(String::from(fn_str), String::from(library_str), n_args, return_type_int, arg_type_ints.as_slice()) {
            Ok(_) => 1,
            Err(_) => 0,
        },
        return_sp
    );

    ::wasm3::wasm3_sys::m3Err_none as _
}

pub unsafe extern "C" fn call_fn_wrapper(
    _rt: ::wasm3::wasm3_sys::IM3Runtime,
    _sp: ::wasm3::wasm3_sys::m3stack_t,
    mem: *mut u8,
) -> *const core::ffi::c_void {
    use ::wasm3::WasmType as _;

    let mut fn_table = FnTable::new();
    let return_sp = _sp;

    let fn_str_ptr = mem.offset(i32::pop_from_stack(_sp) as isize) as *const i8;
    let _sp = _sp.add(i32::SIZE_IN_SLOT_COUNT);
    let args_ptr = mem.offset(i32::pop_from_stack(_sp) as isize) as *mut c_void;
    let _sp = _sp.add(i32::SIZE_IN_SLOT_COUNT);

    let fn_str = match CStr::from_ptr(fn_str_ptr).to_str() {
        Ok(string) => {string}
        Err(_) => {
            i32::push_on_stack(0, return_sp);
            return ::wasm3::wasm3_sys::m3Err_none as _;
        }
    };

    let foreign_fn = match fn_table.0.get_mut(&String::from(fn_str)) {
        Some(function) => function,
        None => {
            i32::push_on_stack(0, return_sp);
            return ::wasm3::wasm3_sys::m3Err_none as _;
        },
    };
    let mut args: Vec<*mut c_void> = Vec::new();

    for i in 0..foreign_fn.cif.nargs as isize {
        let arg: *mut c_void = args_ptr.offset(i) ;
        args.push(match foreign_fn.arg_types[i as usize] {
            FfiType::POINTER => mem.offset(*(arg as *const i32) as isize) as *mut c_void,
            _ => arg,
        });
    }

    match foreign_fn.return_type {
        FfiType::DOUBLE => {
            let return_value: f64 = foreign_fn.call(args.as_mut_ptr());
            f64::push_on_stack(return_value, return_sp);
        },
        FfiType::FLOAT => {
            let return_value: f32 = foreign_fn.call(args.as_mut_ptr());
            f32::push_on_stack(return_value, return_sp);
        },
        FfiType::LONGDOUBLE => {
            // Could  possibly be something more than 64 bits? (80 bits)
            let return_value: f64 = foreign_fn.call(args.as_mut_ptr());
            f64::push_on_stack(return_value, return_sp);
        },
        FfiType::POINTER => {
            let return_value: u64 = foreign_fn.call(args.as_mut_ptr());
            u64::push_on_stack(return_value, return_sp);
        },
        FfiType::SINT16 => {
            // wasm3-rs support to push things smaller than 32 bits? Is it even possible?
            let return_value: i16 = foreign_fn.call(args.as_mut_ptr());
            i32::push_on_stack(return_value as i32, return_sp);
        },
        FfiType::SINT32 => {
            let return_value: i32 = foreign_fn.call(args.as_mut_ptr());
            i32::push_on_stack(return_value, return_sp);
        },
        FfiType::SINT64 => {
            let return_value: i64 = foreign_fn.call(args.as_mut_ptr());
            i64::push_on_stack(return_value, return_sp);
        },
        FfiType::SINT8 => {
            let return_value: i8 = foreign_fn.call(args.as_mut_ptr());
            i32::push_on_stack(return_value as i32, return_sp);
        },
        FfiType::UINT16 => {
            let return_value: u16 = foreign_fn.call(args.as_mut_ptr());
            u32::push_on_stack(return_value as u32, return_sp);
        },
        FfiType::UINT32 => {
            let return_value: u32 = foreign_fn.call(args.as_mut_ptr());
            u32::push_on_stack(return_value, return_sp);
        },
        FfiType::UINT64 => {
            let return_value: u64 = foreign_fn.call(args.as_mut_ptr());
            u64::push_on_stack(return_value, return_sp);
        },
        FfiType::UINT8 => {
            let return_value: u8 = foreign_fn.call(args.as_mut_ptr());
            u32::push_on_stack(return_value as u32, return_sp);
        },
        FfiType::VOID => {
            // Void, lets try to read a 32 bit int and hope nothing bad happens...
            // Pushing 0 on the WebAssembly stack to keep it happy
            let _: i32 = foreign_fn.call(args.as_mut_ptr());
            i32::push_on_stack(0, return_sp);
        },
    };

    ::wasm3::wasm3_sys::m3Err_none as _
}