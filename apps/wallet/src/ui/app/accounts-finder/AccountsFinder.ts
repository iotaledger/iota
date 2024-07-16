// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { AccountFromFinder, AddressFromFinder } from '_src/shared/accounts';
import { diffAddressesBipPaths, mergeAccounts, recoverAccounts } from './accounts-finder';
import type { IotaClient } from '@iota/iota.js/client';
import { getEmptyBalance } from './helpers';
import type { FindBalance } from './types';
import { Ed25519PublicKey } from '@iota/iota.js/keypairs/ed25519';

export enum AllowedAccountSourceTypes {
    MnemonicDerived = 'mnemonic',
    SeedDerived = 'seed',
    LedgerDerived = 'ledger',
}

export enum AllowedBip44CoinTypes {
    IOTA = 4218,
    Shimmer = 4219,
}

export enum SearchAlgorithm {
    BREADTH,
    DEPTH,
    ITERATIVE_DEEPENING_BREADTH_FIRST,
}

export interface AccountFinderConfigParams {
    getPublicKey: (params: {
        accountIndex: number;
        addressIndex: number;
        changeIndex: number;
    }) => Promise<string>;
    client: IotaClient;
    bip44CoinType: AllowedBip44CoinTypes;
    accountSourceType: AllowedAccountSourceTypes;
    algorithm?: SearchAlgorithm;
    coinType: string; // format: '0x2::iota::IOTA'
    changeIndexes?: number[];
    accountGapLimit?: number;
    addressGapLimit?: number;
}

interface GapConfiguration {
    accountGapLimit: number;
    addressGapLimit: number;
}

type GapConfigurationByCoinType = {
    [key in AllowedAccountSourceTypes]: GapConfiguration;
};

const GAP_CONFIGURATION: { [key in AllowedBip44CoinTypes]: GapConfigurationByCoinType } = {
    // in IOTA we have chrysalis users which could have rotated addresses
    [AllowedBip44CoinTypes.IOTA]: {
        [AllowedAccountSourceTypes.LedgerDerived]: {
            accountGapLimit: 1,
            addressGapLimit: 5,
        },
        [AllowedAccountSourceTypes.MnemonicDerived]: {
            accountGapLimit: 3,
            addressGapLimit: 10,
        },
        [AllowedAccountSourceTypes.SeedDerived]: {
            accountGapLimit: 3,
            addressGapLimit: 10,
        },
    },
    // In shimmer we focus on accounts indexes and never rotate addresses
    [AllowedBip44CoinTypes.Shimmer]: {
        [AllowedAccountSourceTypes.LedgerDerived]: {
            accountGapLimit: 3,
            addressGapLimit: 0,
        },
        [AllowedAccountSourceTypes.MnemonicDerived]: {
            accountGapLimit: 10,
            addressGapLimit: 0,
        },
        [AllowedAccountSourceTypes.SeedDerived]: {
            accountGapLimit: 10,
            addressGapLimit: 0,
        },
    },
};

const CHANGE_INDEXES: { [key in AllowedBip44CoinTypes]: number[] } = {
    [AllowedBip44CoinTypes.IOTA]: [0, 1],
    [AllowedBip44CoinTypes.Shimmer]: [0],
};

export class AccountsFinder {
    private accountGapLimit: number = 0;
    private addressGapLimit: number = 0;
    private changeIndexes: number[] = [0];

    private algorithm: SearchAlgorithm;
    private bip44CoinType: AllowedBip44CoinTypes;
    private coinType: string;
    private getPublicKey;
    private client: IotaClient;
    private accounts: AccountFromFinder[] = []; // Found accounts with balances.

    constructor(config: AccountFinderConfigParams) {
        this.getPublicKey = config.getPublicKey;
        this.client = config.client;
        this.bip44CoinType = config.bip44CoinType;
        this.coinType = config.coinType;
        this.changeIndexes = config.changeIndexes || CHANGE_INDEXES[config.bip44CoinType];

        this.algorithm = config.algorithm || SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST;
        this.accountGapLimit =
            config.accountGapLimit ??
            GAP_CONFIGURATION[this.bip44CoinType][config.accountSourceType]?.accountGapLimit;

        this.addressGapLimit =
            config.addressGapLimit ??
            GAP_CONFIGURATION[this.bip44CoinType][config.accountSourceType]?.addressGapLimit;
    }

    reset() {
        this.accounts = [];
    }

    // This function calls each time when user press "Search" button
    async find() {
        switch (this.algorithm) {
            case SearchAlgorithm.BREADTH:
                return await this.runBreadthSearch();
            case SearchAlgorithm.DEPTH:
                return await this.runDepthSearch();
            case SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST:
                return [...(await this.runBreadthSearch()), ...(await this.runDepthSearch())];
            default:
                throw new Error(`Unsupported search algorithm: ${this.algorithm}`);
        }
    }

    async processAccounts({ foundAccounts }: { foundAccounts: AccountFromFinder[] }) {
        const mergedAccounts = mergeAccounts(this.accounts, foundAccounts);

        // Persist new addresses
        const newAddressesBipPaths = diffAddressesBipPaths(foundAccounts, this.accounts);

        this.accounts = mergedAccounts;

        return newAddressesBipPaths;
    }

    async runDepthSearch() {
        const depthAccounts = this.accounts;

        // if we have no accounts yet, we populate with empty accounts
        if (!depthAccounts.length) {
            for (let accountIndex = 0; accountIndex <= this.accountGapLimit; accountIndex++) {
                depthAccounts.push({
                    index: accountIndex,
                    addresses: [],
                });
            }
        }

        let processedAccounts: AddressFromFinder[] = [];

        // depth search is done by searching for more addresses for each account in isolation
        for (const account of depthAccounts) {
            // during depth search we search for 1 account at a time and start from the last address index
            const foundAccounts = await recoverAccounts({
                accountStartIndex: account.index, // we search for the current account
                accountGapLimit: 0, // we only search for 1 account
                addressStartIndex: account.addresses.length, // we start from the last address index
                addressGapLimit: this.addressGapLimit, // we search for the full address gap limit
                changeIndexes: this.changeIndexes,
                findBalance: this.findBalance,
            });

            processedAccounts = [
                ...processedAccounts,
                ...(await this.processAccounts({ foundAccounts })),
            ];
        }

        return processedAccounts;
    }

    async runBreadthSearch() {
        // during breadth search we always start by searching for new account indexes
        const initialAccountIndex = this.accounts?.length ? this.accounts.length : 0; // next index or start from 0;

        const foundAccounts = await recoverAccounts({
            accountStartIndex: initialAccountIndex, // we start from the last existing account index
            accountGapLimit: this.accountGapLimit, // we search for the full account gap limit
            addressStartIndex: 0, // we start from the first address index
            addressGapLimit: this.addressGapLimit, // we search for the full address gap limit
            changeIndexes: this.changeIndexes,
            findBalance: this.findBalance,
        });

        return await this.processAccounts({ foundAccounts });
    }

    findBalance: FindBalance = async (params) => {
        const emptyBalance = getEmptyBalance(this.coinType);

        if (!this.client) {
            throw new Error('IotaClient is not initialized');
        }

        const publicKeyB64 = await this.getPublicKey(params);
        const publicKey = new Ed25519PublicKey(publicKeyB64);

        const foundBalance = await this.client.getBalance({
            owner: publicKey.toIotaAddress(),
            coinType: this.coinType,
        });

        return {
            publicKey: publicKeyB64,
            balance: foundBalance || emptyBalance,
        };
    };
}
