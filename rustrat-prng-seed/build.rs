use std::env;

fn main() {
    let target = env::var("TARGET").unwrap();

    let src: &str;
    let output: &str;

    // TODO add additional targets to support cross-compiling for Windows on Linux
    match target.as_str() {
        "x86_64-pc-windows-msvc" => {
            src = "src/asm/x86_64.asm";
            output = "x86_64";
        }
        "i686-pc-windows-msvc" => {
            src = "src/asm/x86.asm";
            output = "x86";
        }
        _ => panic!("Unsupported target: {}", target),
    }

    println!("cargo:rerun-if-changed={}", src);

    cc::Build::new().file(src).compile(output);
}
