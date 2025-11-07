// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_open_rpc::MethodRouting;

#[derive(Debug, Clone)]
pub struct RpcRouter {
    routes: HashMap<String, MethodRouting>,
    disable_routing: bool,
}

impl RpcRouter {
    pub fn new(routes: HashMap<String, MethodRouting>, disable_routing: bool) -> Self {
        Self {
            routes,
            disable_routing,
        }
    }

    pub fn route<'c, 'a: 'c, 'b: 'c>(&'a self, method: &'b str, version: Option<&str>) -> &'c str {
        if self.disable_routing {
            method
        } else {
            // Modify the method name if routing is enabled
            match (version, self.routes.get(method)) {
                (Some(v), Some(route)) if route.matches(v) => route.route_to.as_str(),
                _ => method,
            }
        }
    }
}
