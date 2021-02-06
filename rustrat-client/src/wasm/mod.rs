use wasm3::error::Result;
use wasm3::{Environment, Module, Runtime};

pub struct WasmEnvironment {
    env: Environment,
    runtime: Runtime,
}

impl WasmEnvironment {
    pub fn new(stack_size: u32) -> Result<Self> {
        let env = Environment::new()?;
        let runtime = env.create_runtime(stack_size)?;

        Ok(WasmEnvironment { env, runtime })
    }

    pub fn load_module(&self, bytes: &[u8]) -> Result<Module> {
        let parsed_module = Module::parse(&self.env, bytes)?;
        self.runtime.load_module(parsed_module)
    }
}
