use crate::error::*;
use crate::ffi::wrappers;
use crate::runtime::CommonUtils;

// TODO create (named) runtimes that can be reused

pub struct Environment {
    runtime: wasm3::Runtime,
}

impl Environment {
    pub fn oneshot<F: Fn(&str) + 'static>(
        wasm: &[u8],
        common_utils: CommonUtils,
        print_closure: F,
        fn_name: &str,
    ) -> Result<i32> {
        let mut env = Environment::new(wasm, common_utils, print_closure)?;
        env.execute(fn_name)
    }

    pub fn new<F: Fn(&str) + 'static>(
        wasm: &[u8],
        common_utils: CommonUtils,
        print_closure: F,
    ) -> Result<Self> {
        let environment = wasm3::Environment::new()?;
        // TODO configurable stack size?
        let runtime = environment.create_runtime(10 * 1000 * 1000)?;

        let mut module = runtime.parse_and_load_module(wasm)?;

        wrappers::link_print_closure(&mut module, print_closure)?;
        wrappers::link_ffi_bindings(&mut module, &common_utils.fn_table)?;

        Ok(Self { runtime })
    }

    pub fn execute(&mut self, fn_name: &str) -> Result<i32> {
        let function = self.runtime.find_function::<(), i32>(fn_name)?;
        Ok(function.call()?)
    }

    // TODO something? Currently only here for tests. Should probably add support for calling functions with arguments(?) and remove this
    pub fn get_wasm_environment(&mut self) -> &mut wasm3::Runtime {
        &mut self.runtime
    }
}
