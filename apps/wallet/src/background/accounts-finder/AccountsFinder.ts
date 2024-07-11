// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AccountFromFinder } from '_src/shared/accounts';
import { diffAddressesBipPaths, mergeAccounts, recoverAccounts } from './accounts-finder';
import NetworkEnv from '../NetworkEnv';
import { IotaClient, getFullnodeUrl } from '@iota/iota.js/client';
import { AccountType } from '../accounts/Account';
import { GAS_TYPE_ARG } from '_redux/slices/iota-objects/Coin';
import {
    persistAddressesToSource,
    getEmptyBalance,
    getPublicKey,
} from '_src/background/accounts-finder/helpers';
import { type FindBalance } from '_src/background/accounts-finder/types';

// Note: we exclude private keys for the account finder because more addresses cant be derived from them
export type AllowedAccountTypes = Exclude<AccountType, AccountType.PrivateKeyDerived>;

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
    bip44CoinType: AllowedBip44CoinTypes;
    accountType: AllowedAccountTypes;
    algorithm?: SearchAlgorithm;
    coinType: string; // format: '0x2::iota::IOTA'
    sourceID: string;
    changeIndexes?: number[];
    accountGapLimit?: number;
    addressGapLimit?: number;
}

interface GapConfiguration {
    accountGapLimit: number;
    addressGapLimit: number;
}

type GapConfigurationByCoinType = {
    [key in AllowedAccountTypes]: GapConfiguration;
};

const GAP_CONFIGURATION: { [key in AllowedBip44CoinTypes]: GapConfigurationByCoinType } = {
    // in IOTA we have chrysalis users which could have rotated addresses
    [AllowedBip44CoinTypes.IOTA]: {
        [AccountType.LedgerDerived]: {
            accountGapLimit: 1,
            addressGapLimit: 5,
        },
        [AccountType.MnemonicDerived]: {
            accountGapLimit: 3,
            addressGapLimit: 10,
        },
        [AccountType.SeedDerived]: {
            accountGapLimit: 3,
            addressGapLimit: 10,
        },
    },
    // In shimmer we focus on accounts indexes and never rotate addresses
    [AllowedBip44CoinTypes.Shimmer]: {
        [AccountType.LedgerDerived]: {
            accountGapLimit: 3,
            addressGapLimit: 0,
        },
        [AccountType.MnemonicDerived]: {
            accountGapLimit: 10,
            addressGapLimit: 0,
        },
        [AccountType.SeedDerived]: {
            accountGapLimit: 10,
            addressGapLimit: 0,
        },
    },
};

const CHANGE_INDEXES: { [key in AllowedBip44CoinTypes]: number[] } = {
    [AllowedBip44CoinTypes.IOTA]: [0, 1],
    [AllowedBip44CoinTypes.Shimmer]: [0],
};

class AccountsFinder {
    private accountGapLimit: number = 0;
    private addressGapLimit: number = 0;
    private changeIndexes: number[] = [0];

    private algorithm: SearchAlgorithm = SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST;
    private bip44CoinType: AllowedBip44CoinTypes = AllowedBip44CoinTypes.IOTA; // 4218 for IOTA or 4219 for Shimmer
    private coinType: string = GAS_TYPE_ARG;
    private sourceID: string = '';
    public client: IotaClient | null = null;

    accounts: AccountFromFinder[] = []; // Found accounts with balances.

    reset() {
        this.accounts = [];
    }

    async setConfig(config: AccountFinderConfigParams) {
        const network = await NetworkEnv.getActiveNetwork();
        this.client = new IotaClient({
            url: network.customRpcUrl ? network.customRpcUrl : getFullnodeUrl(network.network),
        });

        this.bip44CoinType = config.bip44CoinType;
        this.coinType = config.coinType;
        this.sourceID = config.sourceID;
        this.changeIndexes = config.changeIndexes || CHANGE_INDEXES[config.bip44CoinType];

        this.algorithm = config.algorithm || SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST;

        this.accountGapLimit =
            config.accountGapLimit ??
            GAP_CONFIGURATION[this.bip44CoinType][config.accountType].accountGapLimit;

        this.addressGapLimit =
            config.addressGapLimit ??
            GAP_CONFIGURATION[this.bip44CoinType][config.accountType].addressGapLimit;
    }

    async processAccounts({ foundAccounts }: { foundAccounts: AccountFromFinder[] }) {
        const mergedAccounts = mergeAccounts(this.accounts, foundAccounts);

        // Persist new addresses
        const newAddressesBipPaths = diffAddressesBipPaths(foundAccounts, this.accounts);
        await persistAddressesToSource(this.sourceID, newAddressesBipPaths);

        this.accounts = mergedAccounts;
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

            await this.processAccounts({ foundAccounts });
        }
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

        await this.processAccounts({ foundAccounts });
    }

    // This function calls each time when user press "Search" button
    async find(config: AccountFinderConfigParams) {
        await this.setConfig(config);

        switch (this.algorithm) {
            case SearchAlgorithm.BREADTH:
                await this.runBreadthSearch();
                break;
            case SearchAlgorithm.DEPTH:
                await this.runDepthSearch();
                break;
            case SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST:
                await this.runBreadthSearch();
                await this.runDepthSearch();
                break;
            default:
                throw new Error(`Unsupported search algorithm: ${this.algorithm}`);
        }
    }

    findBalance: FindBalance = async (params) => {
        const emptyBalance = getEmptyBalance(this.coinType);

        if (!this.client) {
            throw new Error('IotaClient is not initialized');
        }

        const publicKeyHash = await getPublicKey({
            sourceID: this.sourceID,
            coinType: this.bip44CoinType,
            accountIndex: params.accountIndex,
            addressIndex: params.addressIndex,
            changeIndex: params.changeIndex,
        });

        const foundBalance = await this.client.getBalance({
            owner: publicKeyHash,
            coinType: this.coinType,
        });

        return {
            publicKeyHash,
            balance: foundBalance || emptyBalance,
        };
    };
}

const accountsFinder = new AccountsFinder();
export default accountsFinder;
