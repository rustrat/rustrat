use libffi::middle;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::{c_char, c_void};

pub mod error;
pub mod wrappers;

#[link(name = "Kernel32")]
extern "C" {
    fn GetModuleHandleA(libraryName: *const c_char) -> *mut c_void;
    fn LoadLibraryA(libraryName: *const c_char) -> *mut c_void;
    fn GetProcAddress(handle: *mut c_void, fn_name: *mut c_char) -> *mut c_void;
    pub fn GetLastError() -> u32;
}

// TODO macro to convert C header information to register_fn or something
pub enum Win32FfiTypes {
    LPCSTR = FfiType::POINTER as isize,
    DWORD = FfiType::SINT32 as isize,
}

#[derive(FromPrimitive, Copy, Clone)]
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

impl FfiType {
    fn to_libffi_type(&self) -> middle::Type {
        match *self {
            FfiType::DOUBLE => middle::Type::f64(),
            FfiType::FLOAT => middle::Type::f32(),
            FfiType::LONGDOUBLE => middle::Type::f64(),
            FfiType::POINTER => middle::Type::pointer(),
            FfiType::SINT16 => middle::Type::i16(),
            FfiType::SINT32 => middle::Type::i32(),
            FfiType::SINT64 => middle::Type::i64(),
            FfiType::SINT8 => middle::Type::i8(),
            FfiType::UINT16 => middle::Type::u16(),
            FfiType::UINT32 => middle::Type::u32(),
            FfiType::UINT64 => middle::Type::u64(),
            FfiType::UINT8 => middle::Type::u8(),
            FfiType::VOID => middle::Type::void(),
        }
    }
}

struct ForeignFn {
    cif: middle::Cif,
    n_args: usize,
    fn_ptr: middle::CodePtr,
    return_type: FfiType,
    arg_types: Vec<FfiType>,
}

// TODO make WebAssembly responsible for more of the work creating structs etc. by making it pass pointers to libffi-rs types instead?

impl ForeignFn {
    unsafe fn call<R>(&self, arguments: &[middle::Arg]) -> R {
        self.cif.call(self.fn_ptr, arguments)
    }
}

pub struct FnTable(HashMap<String, ForeignFn>);

impl FnTable {
    pub fn new() -> Self {
        FnTable(HashMap::new())
    }

    pub fn has_fn(&self, function: String) -> bool {
        self.0.contains_key(&function)
    }

    pub fn register_fn(
        &mut self,
        function: String,
        library: String,
        return_type_int: i32,
        arg_type_ints: &[i32],
    ) -> error::Result<()> {
        if self.has_fn(function.clone()) {
            return Ok(());
        }

        let return_type =
            FfiType::from_i32(return_type_int).ok_or(error::Error::InvalidType(return_type_int))?;
        let libffi_return_type = return_type.to_libffi_type();

        let mut arg_types: Vec<FfiType> = Vec::new();
        let mut libffi_arg_types: Vec<middle::Type> = Vec::new();

        for arg in arg_type_ints {
            let arg_type = FfiType::from_i32(*arg).ok_or(error::Error::InvalidType(*arg))?;
            arg_types.push(arg_type);
            libffi_arg_types.push(arg_type.to_libffi_type());
        }

        let cif = middle::Cif::new(libffi_arg_types.into_iter(), libffi_return_type);

        let fn_ptr = get_fn_ptr(function.as_str(), library.as_str())?;

        self.0.insert(
            function,
            ForeignFn {
                cif: cif,
                fn_ptr: middle::CodePtr(fn_ptr),
                n_args: arg_types.len(),
                return_type: return_type,
                arg_types: arg_types,
            },
        );

        Ok(())
    }

    pub unsafe fn call_fn<R>(
        &self,
        function: String,
        arguments: &[middle::Arg],
    ) -> error::Result<R> {
        let foreign_fn = self
            .0
            .get(&function)
            .ok_or(error::Error::FunctionNotDefined(function))?;

        Ok(foreign_fn.call(arguments))
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

    let fn_ptr = unsafe { GetProcAddress(library_handle, fn_name_arg) };

    unsafe { CString::from_raw(fn_name_arg) };

    if fn_ptr.is_null() {
        return Err(error::Error::FunctionNotFound {
            function: fn_name.to_string(),
            library: library_name.to_string(),
        });
    } else {
        return Ok(fn_ptr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn call_virtualalloc_virtualfree() {
        // When first writing this test I would occasionally lose the Vec containing the argument types.
        // This would not show up every time I ran the test, but would show up if I tried to execute it 100 times.
        // Therefore I have kept the 100 iterations for now to hopefully catch regressions.
        for _ in 0..100 {
            let mut fn_table = FnTable::new();

            let args = [
                FfiType::POINTER as i32,
                FfiType::UINT64 as i32,
                FfiType::UINT32 as i32,
                FfiType::UINT32 as i32,
            ];
            let free_args = [
                FfiType::POINTER as i32,
                FfiType::UINT64 as i32,
                FfiType::UINT32 as i32,
            ];

            fn_table
                .register_fn(
                    String::from("VirtualAlloc"),
                    String::from("kernel32.dll"),
                    FfiType::POINTER as i32,
                    &args,
                )
                .unwrap();
            fn_table
                .register_fn(
                    String::from("VirtualFree"),
                    String::from("kernel32.dll"),
                    FfiType::POINTER as i32,
                    &free_args,
                )
                .unwrap();

            unsafe {
                let ptr: *mut u64 = fn_table
                    .call_fn(
                        String::from("VirtualAlloc"),
                        vec![
                            middle::arg(&ptr::null::<c_void>()),
                            middle::arg(&8u64),
                            middle::arg(&(0x00001000u32 | 0x00002000u32)),
                            middle::arg(&0x04u32),
                        ]
                        .as_slice(),
                    )
                    .unwrap();

                assert_eq!(false, ptr.is_null());
                assert_eq!(
                    0u64, *ptr,
                    "Newly allocated memory is not 0, as guaranteed by VirtualAlloc."
                );

                *ptr = 9u64;

                assert_eq!(
                    9u64, *ptr,
                    "Unable to write and/or read pointer from VirtualAlloc."
                );

                let return_value: i32 = fn_table
                    .call_fn(
                        String::from("VirtualFree"),
                        vec![
                            middle::arg(&ptr),
                            middle::arg(&0u64),
                            middle::arg(&0x00008000u32),
                        ]
                        .as_slice(),
                    )
                    .unwrap();

                assert_ne!(
                    0i32, return_value,
                    "VirtualFree returned 0, indicating an error in freeing the memory."
                );
            };
        }
    }
}
