use std::ffi::{c_void, CString};
use std::os::raw::c_char;

#[link(wasm_import_module = "rustrat")]
extern "C" {
    fn register_fn(
        function: *const c_char,
        library: *const c_char,
        n_args: u32,
        return_type_int: i32,
        arg_type_ints: *const i32,
    ) -> i32;
    fn call_fn_u32(function: *const c_char, arguments: *mut c_void) -> u32;
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

#[no_mangle]
pub unsafe extern "C" fn go() -> u32 {
    let fn_name = CString::new("MessageBoxA").unwrap();
    let library_name = CString::new("User32.dll").unwrap();
    let text = CString::new("MessageBoxA from WebAssembly!").unwrap();
    let caption = CString::new("Created by WebAssembly!").unwrap();
    let fn_arg_types: [i32; 4] = [
        FfiType::UINT64 as i32,
        FfiType::POINTER as i32,
        FfiType::POINTER as i32,
        FfiType::UINT32 as i32,
    ];
    let mut fn_args: [*mut c_void; 4] = [
        &mut 0u64 as *mut _ as *mut c_void,
        &mut text.as_ptr() as *mut _ as *mut c_void,
        &mut caption.as_ptr() as *mut _ as *mut c_void,
        &mut 0x0u32 as *mut _ as *mut c_void,
    ];

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

    call_fn_u32(
        fn_name.as_ptr(),
        fn_args.as_mut_ptr() as *mut _ as *mut c_void,
    )
}
