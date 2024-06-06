// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module bridge::chain_ids {

    use std::vector;

    // Chain IDs
    const IotaMainnet: u8 = 0;
    const IotaTestnet: u8 = 1;
    const IotaDevnet: u8 = 2;
    const IotaLocalTest: u8 = 3;

    const EthMainnet: u8 = 10;
    const EthSepolia: u8 = 11;
    const EthLocalTest: u8 = 12;

    struct BridgeRoute has drop {
        source: u8,
        destination: u8,
    }

    public fun iota_mainnet(): u8 {
        IotaMainnet
    }

    public fun iota_testnet(): u8 {
        IotaTestnet
    }

    public fun iota_devnet(): u8 {
        IotaDevnet
    }

    public fun iota_local_test(): u8 {
        IotaLocalTest
    }

    public fun eth_mainnet(): u8 {
        EthMainnet
    }

    public fun eth_sepolia(): u8 {
        EthSepolia
    }

    public fun eth_local_test(): u8 {
        EthLocalTest
    }

    public fun valid_routes(): vector<BridgeRoute> {
        vector[
            BridgeRoute { source: IotaMainnet, destination: EthMainnet },
            BridgeRoute { source: IotaDevnet, destination: EthSepolia },
            BridgeRoute { source: IotaTestnet, destination: EthSepolia },
            BridgeRoute { source: IotaLocalTest, destination: EthLocalTest },
            BridgeRoute { source: EthMainnet, destination: IotaMainnet },
            BridgeRoute { source: EthSepolia, destination: IotaDevnet },
            BridgeRoute { source: EthSepolia, destination: IotaTestnet },
            BridgeRoute { source: EthLocalTest, destination: IotaLocalTest }]
    }

    public fun is_valid_route(source: u8, destination: u8): bool {
        let route = BridgeRoute { source, destination };
        return vector::contains(&valid_routes(), &route)
    }
}
