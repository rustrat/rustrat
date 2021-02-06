use rustrat_client::ffi::wrappers;
use rustrat_client::wasm::*;
use wasm3;

#[test]
fn test_adder() {
    let env = WasmEnvironment::new(10 * 10 * 1024).expect("Unable to create WASM environment");
    let wasm_module = env
        .load_module(&include_bytes!("wasm-test-bins/wasm_test_bins.wasm")[..])
        .expect("Unable to load module");
    let func = wasm_module
        .find_function::<(u32, u32), u32>("add")
        .expect("Unable to find the add function");

    assert_eq!(func.call(3, 6).expect("Unable to call the add function"), 9);
}

static mut IS_EXTERNAL_FN_CALLED: bool = false;
fn external_fn(param: u32) -> u32 {
    unsafe {
        IS_EXTERNAL_FN_CALLED = true;
    }
    param
}

wasm3::make_func_wrapper!(external_fn_wrapper: external_fn(param: u32) -> u32);

#[test]
fn test_external_fn() {
    let env = WasmEnvironment::new(10 * 10 * 1024).expect("Unable to create WASM environment");
    let mut wasm_module = env
        .load_module(&include_bytes!("wasm-test-bins/wasm_test_bins.wasm")[..])
        .expect("Unable to load module");

    wasm_module
        .link_function::<(u32,), u32>("env", "external_fn", external_fn_wrapper)
        .expect("Unable to link external_fn");

    let func = wasm_module
        .find_function::<(u32,), u32>("call_external")
        .expect("Unable to find the call_external function");

    assert_eq!(
        func.call(9)
            .expect("Unable to call the call_external function"),
        9
    );
    unsafe {
        assert_eq!(IS_EXTERNAL_FN_CALLED, true, "external-fn not called");
    }
}

#[test]
fn test_wasm_virtualalloc_virtualfree() {
    let env = WasmEnvironment::new(10 * 10 * 1024).expect("Unable to create WASM environment");
    let mut wasm_module = env
        .load_module(&include_bytes!("wasm-test-bins/wasm_test_bins.wasm")[..])
        .expect("Unable to load module");

    wasm_module
        .link_function::<(u32,), i32>("rustrat", "has_fn", wrappers::has_fn_wrapper)
        .expect("Unable to link has_fn");
    wasm_module
        .link_function::<(u32, u32, u32, i32, u32), i32>(
            "rustrat",
            "register_fn",
            wrappers::register_fn_wrapper,
        )
        .expect("Unable to link register_fn");
    wasm_module
        .link_function::<(u32, u32), u32>("rustrat", "call_fn_u32", wrappers::call_fn_wrapper)
        .expect("Unable to link call_fn_u32");
    wasm_module
        .link_function::<(u32, u32), u64>("rustrat", "call_fn_u64", wrappers::call_fn_wrapper)
        .expect("Unable to link call_fn_u64");

    let virtualalloc = wasm_module
        .find_function::<(), u64>("virtualalloc_u64")
        .expect("Unable to find the virtualalloc_u64 function");
    let virtualfree = wasm_module
        .find_function::<(u64,), u32>("virtualfree")
        .expect("Unable to find the virtualfree function");

    for _ in 0..100 {
        unsafe {
            wrappers::setup_fn_table();
            let ptr = virtualalloc
                .call()
                .expect("Unable to call the virtualalloc_u64 function.");
            let actual_ptr: *mut u64 = ptr as *mut u64;

            assert_ne!(0, ptr, "VirtualAlloc return 0/NULL, indicating an error.");
            assert_eq!(
                0, *actual_ptr,
                "Allocated pointer does not contain 0, as guaranteed by VirtualAlloc."
            );

            *actual_ptr = 9;

            assert_eq!(
                9, *actual_ptr,
                "Unable to read and/or write to pointer from VirtualAlloc."
            );

            assert_ne!(
                0,
                virtualfree
                    .call(ptr)
                    .expect("Unable to call the virtualfree function."),
                "Virtualfree returned 0, indicating an error."
            );
        }
    }
}
