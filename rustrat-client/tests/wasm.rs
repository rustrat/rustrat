use rustrat_client::wasm::*;
use wasm3;

#[test]
fn test_adder() {
    let env = WasmEnvironment::new(10 * 10 * 1024)
        .expect("Unable to create WASM environment");
    let wasm_module = env.load_module(&include_bytes!("wasm-test-bins/wasm_test_bins.wasm")[..])
        .expect("Unable to load module");
    let func = wasm_module.find_function::<(u32, u32), u32>("add")
        .expect("Unable to find add function");
    
    assert_eq!(func.call(3, 6).expect("Unable to call add function"), 9);
}

static mut IS_EXTERNAL_FN_CALLED: bool = false;
fn external_fn(param: u32) -> u32 {
    unsafe { IS_EXTERNAL_FN_CALLED = true; }
    param
}

wasm3::make_func_wrapper!(external_fn_wrapper: external_fn(param: u32) -> u32);

#[test]
fn test_external_fn() {
    let env = WasmEnvironment::new(10 * 10 * 1024)
        .expect("Unable to create WASM environment");
    let mut wasm_module = env.load_module(&include_bytes!("wasm-test-bins/wasm_test_bins.wasm")[..])
        .expect("Unable to load module");

    wasm_module.link_function::<(u32,), u32>("env", "external_fn", external_fn_wrapper)
        .expect("Unable to link external_fn");

    let func = wasm_module.find_function::<(u32,), u32>("call_external")
        .expect("Unable to find call_external function");

   assert_eq!(func.call(9).expect("Unable to call call_external function"), 9); 
   unsafe { assert_eq!(IS_EXTERNAL_FN_CALLED, true, "external-fn not called"); }
}