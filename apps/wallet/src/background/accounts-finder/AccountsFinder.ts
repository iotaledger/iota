// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AccountFromFinder, type AddressFromFinder } from '_src/shared/accounts';
import { hasBalance } from './accounts-finder';
import NetworkEnv from '../NetworkEnv';
import {
    IotaClient,
    getFullnodeUrl,
    type GetBalanceParams,
    type CoinBalance,
} from '@iota/iota.js/client';
import { getAccountSourceByID } from '../account-sources';
import { type MakeDerivationOptions } from '_src/background/account-sources/bip44Path';
import { AccountType } from '../accounts/Account';
import { IOTA_GAS_COIN_TYPE } from '_redux/slices/iota-objects/Coin';

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

export interface SearchAccountsFinderParams {
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

export interface RecoverAccountsParams {
    accountStartIndex: number;
    accountGapLimit: number;
    addressStartIndex: number;
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
            addressGapLimit: 30,
        },
        [AccountType.SeedDerived]: {
            accountGapLimit: 3,
            addressGapLimit: 2,
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

const DEFAULT_PARAMS = {
    bip44CoinType: AllowedBip44CoinTypes.IOTA,
    accountType: AccountType.LedgerDerived as AllowedAccountTypes,
    coinType: IOTA_GAS_COIN_TYPE,
    sourceID: '',
    accountGapLimit: 0,
    addressGapLimit: 0,
};

class AccountsFinder {
    private algorithm: SearchAlgorithm = SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST;
    private accountGapLimit: number = DEFAULT_PARAMS.accountGapLimit;
    private addressGapLimit: number = DEFAULT_PARAMS.addressGapLimit;
    private bip44CoinType: AllowedBip44CoinTypes = DEFAULT_PARAMS.bip44CoinType; // 4218 for IOTA or 4219 for Shimmer
    private coinType: string = IOTA_GAS_COIN_TYPE;
    private sourceID: string = DEFAULT_PARAMS.sourceID;
    private changeIndexes: number[] = CHANGE_INDEXES[DEFAULT_PARAMS.bip44CoinType];
    public client: IotaClient | null = null;

    accounts: AccountFromFinder[] = []; // Found accounts with balances.

    reset() {
        this.accounts = [];
    }

    async setConfigs(params: SearchAccountsFinderParams) {
        const network = await NetworkEnv.getActiveNetwork();
        this.client = new IotaClient({
            url: network.customRpcUrl ? network.customRpcUrl : getFullnodeUrl(network.network),
        });

        this.bip44CoinType = params.bip44CoinType;
        this.coinType = params.coinType;
        this.sourceID = params.sourceID;
        this.changeIndexes = params.changeIndexes || CHANGE_INDEXES[params.bip44CoinType];

        this.algorithm = params.algorithm || SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST;

        this.accountGapLimit =
            params.accountGapLimit ??
            GAP_CONFIGURATION[this.bip44CoinType][params.accountType].accountGapLimit;

        this.addressGapLimit =
            params.addressGapLimit ??
            GAP_CONFIGURATION[this.bip44CoinType][params.accountType].addressGapLimit;
    }

    async searchBalances(accountIndex: number, addressIndex: number) {
        const addresses: AddressFromFinder[] = [];

        // if any of the addresses has a balance, we increase the target address index to keep searching
        let isBalanceExists = false;
        for (const changeIndex of this.changeIndexes) {
            const bipPath = {
                coinType: this.bip44CoinType,
                accountIndex: accountIndex,
                addressIndex: addressIndex,
                changeIndex,
            };
            const pubKeyHash = await this.getPublicKey(bipPath);

            const balance = await this.getBalance({
                owner: pubKeyHash,
                coinType: this.coinType,
            });

            if (hasBalance(balance)) {
                isBalanceExists = true;
            }

            addresses.push({
                pubKeyHash,
                balance,
                bipPath,
            });
        }
        return {
            addresses,
            isBalanceExists,
        };
    }

    // Merge function
    mergeAccounts(accountsLists: AccountFromFinder[][]): AccountFromFinder[] {
        const mergedAccountsMap: Map<number, Array<Array<AddressFromFinder>>> = new Map();

        // Flatten the list of account lists and process each account
        accountsLists.flat().forEach((account) => {
            if (!mergedAccountsMap.has(account.index)) {
                // If the account index is not yet in the map, add it directly
                mergedAccountsMap.set(account.index, account.addresses);
            } else {
                // If the account index exists, merge the addresses
                const existingAddresses = mergedAccountsMap.get(account.index);

                if (existingAddresses) {
                    const newAddresses = this.mergeAddresses(existingAddresses, account.addresses);
                    mergedAccountsMap.set(account.index, newAddresses);
                }
            }
        });

        // Convert the map back to an array of AccountFromFinder
        return Array.from(mergedAccountsMap, ([index, addresses]) => ({
            index,
            addresses,
        }));
    }

    // Helper function to merge two arrays of arrays of addresses
    mergeAddresses(
        existingAddresses: Array<Array<AddressFromFinder>>,
        newAddresses: Array<Array<AddressFromFinder>>,
    ): Array<Array<AddressFromFinder>> {
        const maxLength = Math.max(existingAddresses.length, newAddresses.length);
        const merged: Array<Array<AddressFromFinder>> = [];

        for (let i = 0; i < maxLength; i++) {
            merged[i] = [...(existingAddresses[i] || []), ...(newAddresses[i] || [])];
        }

        return merged;
    }

    async recoverAccount(accountIndex: number, addressStartIndex: number, addressGapLimit: number) {
        const account = this.accounts.find((acc) => acc.index === accountIndex) ?? {
            index: accountIndex,
            addresses: [],
        };

        // Flag to check if any of the addresses of the account has a balance
        let isBalanceExists = false;

        // Isolated search for no address rotation
        if (!addressGapLimit) {
            const { addresses, isBalanceExists: isBalanceExists } = await this.searchBalances(
                accountIndex,
                addressStartIndex,
            );

            account.addresses.push(addresses); // we add the addresses to the account

            return {
                account,
                isBalanceExists,
            };
        }

        // on each fixed account index, we search for addresses in the given range
        let targetAddressIndex = addressStartIndex + addressGapLimit;
        for (
            let addressIndex = addressStartIndex;
            addressIndex < targetAddressIndex;
            addressIndex++
        ) {
            const { addresses, isBalanceExists: isHasBalance } = await this.searchBalances(
                accountIndex,
                addressIndex,
            );

            if (isHasBalance) {
                targetAddressIndex = addressIndex + addressGapLimit;
                isBalanceExists = true;
            }

            account.addresses.push(addresses);
        }

        return { account, isBalanceExists };
    }

    async recoverAccounts(
        params: RecoverAccountsParams = {
            accountStartIndex: 0,
            accountGapLimit: 0,
            addressStartIndex: 0,
            addressGapLimit: 0,
        },
    ) {
        const { accountStartIndex, accountGapLimit, addressStartIndex, addressGapLimit } = params;

        const accounts: AccountFromFinder[] = [];
        let targetAccountIndex = accountStartIndex + accountGapLimit;

        // isolated search for one account;
        if (!accountGapLimit) {
            const { account } = await this.recoverAccount(
                accountStartIndex,
                addressStartIndex,
                addressGapLimit,
            );
            accounts.push(account);
            return accounts;
        }

        // we search for accounts in the given range
        for (
            let accountIndex = accountStartIndex;
            accountIndex < targetAccountIndex;
            accountIndex++
        ) {
            const { isBalanceExists, account } = await this.recoverAccount(
                accountIndex,
                addressStartIndex,
                addressGapLimit,
            );

            // if any of the addresses of the given account has a balance,
            // we increase the target account index to keep searching
            if (isBalanceExists) {
                targetAccountIndex = accountIndex + accountGapLimit;
            }
            // we add the account to the list of accounts
            accounts.push(account);
        }
        return accounts;
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
            const foundAccounts = await this.recoverAccounts({
                accountStartIndex: account.index, // we search for the current account
                accountGapLimit: 0, // we only search for 1 account
                addressStartIndex: account.addresses.length, // we start from the last address index
                addressGapLimit: this.addressGapLimit, // we search for the full address gap limit
            });

            this.accounts = [...this.accounts, ...foundAccounts];
        }
        return this.accounts;
    }

    async runBreadthSearch() {
        // during breadth search we always start by searching for new account indexes
        const initialAccountIndex = this.accounts?.length ? this.accounts.length : 0; // next index or start from 0;

        const foundAccounts = await this.recoverAccounts({
            accountStartIndex: initialAccountIndex, // we start from the last existing account index
            accountGapLimit: this.accountGapLimit, // we search for the full account gap limit
            addressStartIndex: 0, // we start from the first address index
            addressGapLimit: this.addressGapLimit, // we search for the full address gap limit
        });

        this.accounts = [...this.accounts, ...foundAccounts];
        return this.accounts;
    }

    // This function calls each time when user press "Search" button
    async find(
        params: SearchAccountsFinderParams = {
            bip44CoinType: DEFAULT_PARAMS.bip44CoinType,
            accountType: DEFAULT_PARAMS.accountType,
            coinType: DEFAULT_PARAMS.coinType,
            sourceID: DEFAULT_PARAMS.sourceID,
        },
    ) {
        console.log('--- find run1');
        await this.setConfigs(params);

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

    async getPublicKey(options: MakeDerivationOptions) {
        const accountSource = await getAccountSourceByID(this.sourceID);

        if (!accountSource) {
            throw new Error('Could not find account source');
        }

        const pubKey = await accountSource.derivePubKey(options);
        return pubKey?.toIotaAddress();
    }

    async getBalance(params: GetBalanceParams): Promise<CoinBalance> {
        const emptyBalance: CoinBalance = {
            coinType: this.coinType,
            coinObjectCount: 0,
            totalBalance: '0',
            lockedBalance: {},
        };

        if (!this.client) {
            throw new Error('IotaClient is not initialized');
        }

        const foundBalance = await this.client.getBalance(params);

        return foundBalance || emptyBalance;
    }

    getResults(): AddressFromFinder[] {
        return this.accounts
            .flatMap((acc) => acc.addresses.flat())
            .filter((addr) => hasBalance(addr.balance));
    }
}

const accountsFinder = new AccountsFinder();
export default accountsFinder;
