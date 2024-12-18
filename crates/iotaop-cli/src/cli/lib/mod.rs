// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod autocomplete;
pub mod cache;
mod oauth;

pub use autocomplete::FilePathCompleter;
pub use oauth::get_oauth_token;
pub mod utils;

pub fn get_api_server() -> String {
    // if env var is set, use that
    if let Ok(api_server) = std::env::var("API_SERVER") {
        return api_server.to_string();
    }

    "https://meta-svc.api.iota.org".to_string()
}
