# RustRAT

RustRAT is a project to write a **R**emote **A**ccess **T**rojan in Rust.
For more details about the project, see https://www.wodinaz.com/.

This repository contains several folders with different projects.
* rustrat-client The RAT itself
* rustrat-server The server listening for incoming connections (not written yet)
* payloads Projects that compiles down to WebAssembly for execution and helper libraries for payload creation

## Usage
* `rustrat-client-exe.exe path\to\webassembly.wasm function_name`
* `runddl32 rustrat_client.dll,rundll_run path\to\webassembly.wasm function_name`

Note that the functionality to execute compiled WebAssembly will be moved to a separate crate for development/debugging purposes.