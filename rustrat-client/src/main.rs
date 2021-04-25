use std::str;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "rustrat-client", about = "Rustrat client testing tools.")]
enum RustratClient {
    /// Execute a wasm blob
    Wasm { path: String, fn_name: String },
    /// Make HTTP Get request to www.wodinaz.com
    Http,
}

fn main() -> rustrat_client::error::Result<()> {
    let opt = RustratClient::from_args();

    match opt {
        RustratClient::Wasm { path, fn_name } => {
            let result = rustrat_client::run_webassembly(&path, &fn_name)?;
            std::process::exit(result);
        }
        RustratClient::Http => unsafe {
            let result = rustrat_client::connector::http::do_http_get(
                "https://www.wodinaz.com/".to_string(),
            )
            .unwrap();
            println!("{}", str::from_utf8(&result).unwrap());
        },
    }

    Ok(())
}
