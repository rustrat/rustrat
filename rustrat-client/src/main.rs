use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(
    name = "rustrat-client",
    about = "Executes a compiled WebAssembly file. Links functions required to use libffi."
)]
struct Arguments {
    path: String,
    fn_name: String,
}

fn main() -> rustrat_client::error::Result<()> {
    //let opt = Arguments::from_args();
    let opt = Arguments {
        path: String::from("payloads\\target\\wasm32-unknown-unknown\\debug\\demo_messagebox.wasm"),
        fn_name: String::from("go"),
    };

    let result = rustrat_client::run_webassembly(&opt.path, &opt.fn_name)?;

    std::process::exit(result)
}
