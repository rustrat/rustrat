use libffi::low;
use num_derive::FromPrimitive;    
use num_traits::FromPrimitive;
use std::ffi::CString;
use std::os::raw::{c_void,c_char};
use std::collections::HashMap;

pub mod error;
pub mod wrappers;

#[link(name="Kernel32")]
extern "C" {
    fn GetModuleHandleA(libraryName: *const c_char) -> *mut c_void;
    fn LoadLibraryA(libraryName: *const c_char) -> *mut c_void;
    fn GetProcAddress(handle: *mut c_void, fn_name: *mut c_char) -> *mut c_void;
}

#[derive(FromPrimitive,Copy,Clone)]
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

struct ForeignFn {
    cif: low::ffi_cif,
    fn_ptr: low::CodePtr,
    return_type: FfiType,
    arg_types: Vec<FfiType>,
    _arg_types_native: Vec<*mut low::ffi_type>,
}

impl ForeignFn {
    unsafe fn call<R>(&mut self, arguments: *mut *mut c_void) -> R {
        low::call(&mut self.cif, self.fn_ptr, arguments)
    }
}

pub struct FnTable (HashMap<String, ForeignFn>,);

impl FnTable {
    pub fn new() -> Self {
        FnTable(HashMap::new())
    }

    pub fn has_fn(&self, function: String) -> bool {
       self.0.contains_key(&function) 
    }

    pub fn register_fn(&mut self, function: String, library: String, n_args: usize, return_type_int: i32, arg_type_ints: &[i32]) -> error::Result<()> {
        if self.has_fn(function.clone()) {
            return Ok(());
        }

        let mut cif: low::ffi_cif = Default::default();
        let return_type = FfiType::from_i32(return_type_int).ok_or(error::Error::InvalidType(return_type_int))?;
        let return_type_native = unsafe {ffitype_to_native(return_type)};

        let mut arg_types: Vec<FfiType> = Vec::new();
        let mut arg_types_native: Vec<*mut low::ffi_type> = Vec::new();

        for arg in arg_type_ints {
            let arg_type = FfiType::from_i32(*arg).ok_or(error::Error::InvalidType(*arg))?;
            arg_types.push(arg_type);
            arg_types_native.push(unsafe {ffitype_to_native(arg_type)});
        }

        unsafe {
            low::prep_cif(&mut cif, low::ffi_abi_FFI_DEFAULT_ABI, n_args, return_type_native, arg_types_native.as_mut_ptr())?;
        }

        let fn_ptr = get_fn_ptr(function.as_str(), library.as_str())?;

        self.0.insert(function, ForeignFn{cif: cif, fn_ptr: low::CodePtr(fn_ptr), return_type: return_type, arg_types: arg_types, _arg_types_native: arg_types_native});

        Ok(())
    }

    pub unsafe fn call_fn<R>(&mut self, function: String, arguments: *mut *mut c_void) -> error::Result<R> {
        let foreign_fn = self.0.get_mut(&function).ok_or(error::Error::FunctionNotDefined(function))?;

        Ok(foreign_fn.call(arguments))
    }
}

unsafe fn ffitype_to_native(in_type: FfiType) -> *mut low::ffi_type {
    match in_type {
        FfiType::DOUBLE => &mut low::types::double,
        FfiType::FLOAT => &mut low::types::float,
        FfiType::LONGDOUBLE => &mut low::types::longdouble,
        FfiType::POINTER => &mut low::types::pointer,
        FfiType::SINT16 => &mut low::types::sint16,
        FfiType::SINT32 => &mut low::types::sint32,
        FfiType::SINT64 => &mut low::types::sint64,
        FfiType::SINT8 => &mut low::types::sint8,
        FfiType::UINT16 => &mut low::types::uint16,
        FfiType::UINT32 => &mut low::types::uint32,
        FfiType::UINT64 => &mut low::types::uint64,
        FfiType::UINT8 => &mut low::types::uint8,
        FfiType::VOID => &mut low::types::void,
    }
}

fn get_or_load_module(library_name: &str) -> error::Result<*mut c_void> {
    let library_name_arg = CString::new(library_name)?.into_raw();

    unsafe {
        let mut module_handle = GetModuleHandleA(library_name_arg);

        if module_handle.is_null() {
            module_handle = LoadLibraryA(library_name_arg);

            if module_handle.is_null() {
                CString::from_raw(library_name_arg);
                return Err(error::Error::LibraryNotFound(library_name.to_string()));
            }
        }

        CString::from_raw(library_name_arg);
        Ok(module_handle)
    }
}

fn get_fn_ptr(fn_name: &str, library_name: &str) -> error::Result<*mut c_void> {
    let fn_name_arg = CString::new(fn_name)?.into_raw();

    let library_handle = get_or_load_module(library_name)?;

    let fn_ptr = unsafe {GetProcAddress(library_handle, fn_name_arg)};

    unsafe {CString::from_raw(fn_name_arg)};
    
    if fn_ptr.is_null() {
        return Err(error::Error::FunctionNotFound {function: fn_name.to_string(), library: library_name.to_string()});
    } else {
        return Ok(fn_ptr);
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;
    use super::*;

    #[test]
    fn call_virtualalloc_virtualfree() {
        // When first writing this test I would occasionally lose the Vec containing the argument types.
        // This would not show up every time I ran the test, but would show up if I tried to execute it 100 times.
        // Therefore I have kept the 100 iterations for now to hopefully catch regressions.
        for _ in 0..100 {
            let mut fn_table = FnTable::new();

            let args = [FfiType::POINTER as i32, FfiType::UINT64 as i32, FfiType::UINT32 as i32, FfiType::UINT32 as i32];
            let free_args = [FfiType::POINTER as i32, FfiType::UINT64 as i32, FfiType::UINT32 as i32];

            fn_table.register_fn(String::from("VirtualAlloc"), String::from("kernel32.dll"), 4, FfiType::POINTER as i32, &args).unwrap();
            fn_table.register_fn(String::from("VirtualFree"), String::from("kernel32.dll"), 3, FfiType::POINTER as i32, &free_args).unwrap();

            unsafe {
                let mut ptr: *mut u64 = fn_table.call_fn(String::from("VirtualAlloc"), vec![&mut ptr::null::<c_void>() as *mut _ as *mut c_void, &mut 8u64 as *mut _ as *mut c_void, &mut (0x00001000u32 | 0x00002000u32) as *mut _ as *mut c_void, &mut 0x04u32 as *mut _ as *mut c_void].as_mut_ptr()).unwrap();

                assert_eq!(false, ptr.is_null());
                assert_eq!(0u64, *ptr, "Newly allocated memory is not 0, as guaranteed by VirtualAlloc.");

                println!("2");
                *ptr = 9u64;

                assert_eq!(9u64, *ptr, "Unable to write and/or read pointer from VirtualAlloc.");

                println!("4");
                let return_value: i32 = fn_table.call_fn(String::from("VirtualFree"), vec![&mut ptr as *mut _ as *mut c_void, &mut 0u64 as *mut _ as *mut c_void, &mut 0x00008000u32 as *mut _ as *mut c_void].as_mut_ptr()).unwrap();

                assert_ne!(0i32, return_value, "VirtualFree returned 0, indicating an error in freeing the memory.");
            };
        }
    }
}