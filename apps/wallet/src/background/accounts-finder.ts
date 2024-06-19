// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type FoundAccount } from '_src/shared/accounts';
import { type AccountSource } from './account-sources/AccountSource';
import { type Bip44Path } from './account-sources/bip44Path';

export enum SearchAlgorithmType {
    BFS = 'bfs', // Breadth First Search
    DFS = 'dfs', // Depth-first search
    IDS = 'ids', // Iterative deepening depth-first search
}

export interface AccountFinderOptions {
    startAccountIndex: number;
    startAddressIndex: number;
    searchAlgorithm: SearchAlgorithmType;
    accountSource: AccountSource;
}

export class AccountFinder {
    private options: AccountFinderOptions;

    constructor(options: AccountFinderOptions) {
        this.options = options;
    }

    async deriveAddresses(): Promise<Array<FoundAccount>> {
        // Get all the bip44 derivation paths given the specified confguration
        const paths = this.getDerivationPaths();

        // Actually derive all the addresses given the paths from above
        const addresses = await Promise.all(
            paths.map(async (bipPath) => {
                const { address } = await this.options.accountSource.deriveAccount(bipPath);
                return {
                    address,
                    bipPath,
                };
            }),
        );

        return addresses;
    }

    getDerivationPaths(): Array<Bip44Path> {
        const size = 50;

        function bfs() {
            return new Array(size).fill('').map((_, i) => ({
                accountIndex: i,
                addressIndex: 0,
            }));
        }

        function dfs() {
            return new Array(size).fill('').map((_, i) => ({
                accountIndex: 0,
                addressIndex: i,
            }));
        }

        switch (this.options.searchAlgorithm) {
            // Only increment the account index
            case SearchAlgorithmType.BFS:
                return bfs();

            // Only increment the address index
            case SearchAlgorithmType.DFS:
                return dfs();

            // Increment account and address indexes
            case SearchAlgorithmType.IDS:
                return [...bfs(), ...dfs()];
        }
    }
}
