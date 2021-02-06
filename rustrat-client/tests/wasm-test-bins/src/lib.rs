use std::ffi::{c_void, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn add(a: u32, b: u32) -> u32 {
    a + b
}

#[link(wasm_import_module = "env")]
extern "C" {
    fn external_fn(param: u32) -> u32;
}

#[no_mangle]
pub extern "C" fn call_external(param: u32) -> u32 {
    unsafe { external_fn(param) }
}

// Testing FFI wrappers. Every return type needs it own function for now.
#[link(wasm_import_module = "rustrat")]
extern "C" {
    fn has_fn(function: *const c_char) -> i32;
    fn register_fn(
        function: *const c_char,
        library: *const c_char,
        n_args: u32,
        return_type_int: i32,
        arg_type_ints: *const i32,
    ) -> i32;
    fn call_fn_u32(function: *const c_char, arguments: *mut c_void) -> u32;
    fn call_fn_u64(function: *const c_char, arguments: *mut c_void) -> u64;
}
pub enum FfiType {
    DOUBLE = 3,
    FLOAT = 2,
    LONGDOUBLE = 4,
    POINTER = 14,
    SINT16 = 8,
    SINT32 = 10,
    SINT64 = 12,
    SINT8 = 6,
    UINT16 = 7,
    UINT32 = 9,
    UINT64 = 11,
    UINT8 = 5,
    VOID = 0,
}

// Returns 0 if VirtualAlloc has been defined or another error occurs. Returns the result from VirtualAlloc otherwise.
#[no_mangle]
pub unsafe extern "C" fn virtualalloc_u64() -> u64 {
    let fn_name = CString::new("VirtualAlloc").unwrap();
    let library_name = CString::new("Kernel32.dll").unwrap();
    let fn_arg_types: [i32; 4] = [
        FfiType::UINT64 as i32,
        FfiType::UINT64 as i32,
        FfiType::UINT32 as i32,
        FfiType::UINT32 as i32,
    ];
    let mut fn_args: [*mut c_void; 4] = [
        &mut 0u64 as *mut _ as *mut c_void,
        &mut 8u64 as *mut _ as *mut c_void,
        &mut (0x00001000u32 | 0x00002000u32) as *mut _ as *mut c_void,
        &mut 0x04u32 as *mut _ as *mut c_void,
    ];

    if has_fn(fn_name.as_ptr() as *mut c_char) == 1 {
        return 0;
    }

    if register_fn(
        fn_name.as_ptr(),
        library_name.as_ptr(),
        4,
        FfiType::UINT64 as i32,
        fn_arg_types.as_ptr(),
    ) == 0
    {
        return 0;
    }

    call_fn_u64(
        fn_name.as_ptr(),
        fn_args.as_mut_ptr() as *mut _ as *mut c_void,
    )
}

#[no_mangle]
pub unsafe extern "C" fn virtualfree(mut ptr: u64) -> u32 {
    let fn_name = CString::new("VirtualFree").unwrap();
    let library_name = CString::new("Kernel32.dll").unwrap();
    let fn_arg_types: [i32; 3] = [
        FfiType::UINT64 as i32,
        FfiType::UINT64 as i32,
        FfiType::UINT32 as i32,
    ];
    let mut fn_args: [*mut c_void; 3] = [
        &mut ptr as *mut _ as *mut c_void,
        &mut 0u64 as *mut _ as *mut c_void,
        &mut 0x00008000u32 as *mut _ as *mut c_void,
    ];

    if has_fn(fn_name.as_ptr()) == 1 {
        return 0;
    }

    if register_fn(
        fn_name.as_ptr(),
        library_name.as_ptr(),
        3,
        FfiType::UINT32 as i32,
        fn_arg_types.as_ptr(),
    ) == 0
    {
        return 0;
    }

    call_fn_u32(
        fn_name.as_ptr(),
        fn_args.as_mut_ptr() as *mut _ as *mut c_void,
    )
}
