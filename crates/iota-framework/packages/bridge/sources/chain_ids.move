// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module bridge::chain_ids;

// Chain IDs
const IotaMainnet: u8 = 0;
const IotaTestnet: u8 = 1;
const IotaCustom: u8 = 2;

const EthMainnet: u8 = 10;
const EthSepolia: u8 = 11;
const EthCustom: u8 = 12;

const EInvalidBridgeRoute: u64 = 0;

//////////////////////////////////////////////////////
// Types
//

public struct BridgeRoute has copy, drop, store {
    source: u8,
    destination: u8,
}

//////////////////////////////////////////////////////
// Public functions
//

public fun iota_mainnet(): u8 { IotaMainnet }

public fun iota_testnet(): u8 { IotaTestnet }

public fun iota_custom(): u8 { IotaCustom }

public fun eth_mainnet(): u8 { EthMainnet }

public fun eth_sepolia(): u8 { EthSepolia }

public fun eth_custom(): u8 { EthCustom }

public use fun route_source as BridgeRoute.source;

public fun route_source(route: &BridgeRoute): &u8 {
    &route.source
}

public use fun route_destination as BridgeRoute.destination;

public fun route_destination(route: &BridgeRoute): &u8 {
    &route.destination
}

public fun assert_valid_chain_id(id: u8) {
    assert!(
        id == IotaMainnet ||
            id == IotaTestnet ||
            id == IotaCustom ||
            id == EthMainnet ||
            id == EthSepolia ||
            id == EthCustom,
        EInvalidBridgeRoute,
    )
}

public fun valid_routes(): vector<BridgeRoute> {
    vector[
        BridgeRoute { source: IotaMainnet, destination: EthMainnet },
        BridgeRoute { source: EthMainnet, destination: IotaMainnet },
        BridgeRoute { source: IotaTestnet, destination: EthSepolia },
        BridgeRoute { source: IotaTestnet, destination: EthCustom },
        BridgeRoute { source: IotaCustom, destination: EthCustom },
        BridgeRoute { source: IotaCustom, destination: EthSepolia },
        BridgeRoute { source: EthSepolia, destination: IotaTestnet },
        BridgeRoute { source: EthSepolia, destination: IotaCustom },
        BridgeRoute { source: EthCustom, destination: IotaTestnet },
        BridgeRoute { source: EthCustom, destination: IotaCustom },
    ]
}

public fun is_valid_route(source: u8, destination: u8): bool {
    let route = BridgeRoute { source, destination };
    valid_routes().contains(&route)
}

// Checks and return BridgeRoute if the route is supported by the bridge.
public fun get_route(source: u8, destination: u8): BridgeRoute {
    let route = BridgeRoute { source, destination };
    assert!(valid_routes().contains(&route), EInvalidBridgeRoute);
    route
}

//////////////////////////////////////////////////////
// Test functions
//

#[test]
fun test_chains_ok() {
    assert_valid_chain_id(IotaMainnet);
    assert_valid_chain_id(IotaTestnet);
    assert_valid_chain_id(IotaCustom);
    assert_valid_chain_id(EthMainnet);
    assert_valid_chain_id(EthSepolia);
    assert_valid_chain_id(EthCustom);
}

#[test]
#[expected_failure(abort_code = EInvalidBridgeRoute)]
fun test_chains_error() {
    assert_valid_chain_id(100);
}

#[test]
#[expected_failure(abort_code = EInvalidBridgeRoute)]
fun test_iota_chains_error() {
    // this will break if we add one more iota chain id and should be corrected
    assert_valid_chain_id(4);
}

#[test]
#[expected_failure(abort_code = EInvalidBridgeRoute)]
fun test_eth_chains_error() {
    // this will break if we add one more eth chain id and should be corrected
    assert_valid_chain_id(13);
}

#[test]
fun test_routes() {
    let valid_routes = vector[
        BridgeRoute { source: IotaMainnet, destination: EthMainnet },
        BridgeRoute { source: EthMainnet, destination: IotaMainnet },
        BridgeRoute { source: IotaTestnet, destination: EthSepolia },
        BridgeRoute { source: IotaTestnet, destination: EthCustom },
        BridgeRoute { source: IotaCustom, destination: EthCustom },
        BridgeRoute { source: IotaCustom, destination: EthSepolia },
        BridgeRoute { source: EthSepolia, destination: IotaTestnet },
        BridgeRoute { source: EthSepolia, destination: IotaCustom },
        BridgeRoute { source: EthCustom, destination: IotaTestnet },
        BridgeRoute { source: EthCustom, destination: IotaCustom },
    ];
    let mut size = valid_routes.length();
    while (size > 0) {
        size = size - 1;
        let route = valid_routes[size];
        assert!(is_valid_route(route.source, route.destination)); // should not assert
    }
}

#[test]
#[expected_failure(abort_code = EInvalidBridgeRoute)]
fun test_routes_err_iota_1() {
    get_route(IotaMainnet, IotaMainnet);
}

#[test]
#[expected_failure(abort_code = EInvalidBridgeRoute)]
fun test_routes_err_iota_2() {
    get_route(IotaMainnet, IotaTestnet);
}

#[test]
#[expected_failure(abort_code = EInvalidBridgeRoute)]
fun test_routes_err_iota_3() {
    get_route(IotaMainnet, EthSepolia);
}

#[test]
#[expected_failure(abort_code = EInvalidBridgeRoute)]
fun test_routes_err_iota_4() {
    get_route(IotaMainnet, EthCustom);
}

#[test]
#[expected_failure(abort_code = EInvalidBridgeRoute)]
fun test_routes_err_eth_1() {
    get_route(EthMainnet, EthMainnet);
}

#[test]
#[expected_failure(abort_code = EInvalidBridgeRoute)]
fun test_routes_err_eth_2() {
    get_route(EthMainnet, EthCustom);
}

#[test]
#[expected_failure(abort_code = EInvalidBridgeRoute)]
fun test_routes_err_eth_3() {
    get_route(EthMainnet, IotaCustom);
}

#[test]
#[expected_failure(abort_code = EInvalidBridgeRoute)]
fun test_routes_err_eth_4() {
    get_route(EthMainnet, IotaTestnet);
}
