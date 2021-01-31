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