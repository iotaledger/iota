// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    IotaClient,
    getFullnodeUrl,
    type CoinBalance,
    type GetBalanceParams,
} from '@iota/iota.js/client';
import { type MakeDerivationOptions } from '_src/background/account-sources/bip44Path';
import { type AccountFromFinder, type AddressFromFinder } from '_src/shared/accounts';
import NetworkEnv from '../NetworkEnv';
import { getAccountSourceByID } from '../account-sources';
import { AccountType } from '../accounts/Account';
import { hasBalance } from './accounts-finder';

interface GapConfiguration {
    accountGapLimit: number;
    addressGapLimit: number;
}

export enum SearchAlgorithm {
    BREADTH,
    DEPTH,
    ITERATIVE_DEEPENING_BREADTH_FIRST,
}

const GAP_CONFIGURATION: { [key in AccountType]: GapConfiguration } = {
    [AccountType.Ledger]: {
        accountGapLimit: 1,
        addressGapLimit: 5,
    },
    [AccountType.MnemonicDerived]: {
        accountGapLimit: 3,
        addressGapLimit: 30,
    },
    [AccountType.SeedDerived]: {
        accountGapLimit: 3,
        addressGapLimit: 30,
    },
    // Private key accounts are fixed bip paths, so we don't need to search for them
    [AccountType.Imported]: {
        accountGapLimit: 0,
        addressGapLimit: 0,
    },
};

class AccountsFinder {
    // configured with parameters at init()
    private algorithm: SearchAlgorithm = SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST;
    private bip44CoinType: number = 0;
    private coinType: string = '';
    private sourceID: string = '';
    // configured automatically after init()
    private accountGapLimit: number = 0;
    private addressGapLimit: number = 0;

    public client: IotaClient | null = null;

    accounts: AccountFromFinder[] = [];

    init(
        accountType: AccountType,
        algorithm: SearchAlgorithm,
        bip44CoinType: number,
        coinType: string,
        sourceID: string,
    ) {
        this.accounts = [];

        this.algorithm = algorithm;
        this.bip44CoinType = bip44CoinType;
        this.coinType = coinType;
        this.sourceID = sourceID;

        this.accountGapLimit = GAP_CONFIGURATION[accountType].accountGapLimit;
        this.addressGapLimit = GAP_CONFIGURATION[accountType].addressGapLimit;
    }

    async find() {
        if (
            this.algorithm === SearchAlgorithm.BREADTH ||
            this.algorithm === SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST
        ) {
            this.accounts = await this.runBreadthSearch();
        }
        if (
            this.algorithm === SearchAlgorithm.DEPTH ||
            this.algorithm === SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST
        ) {
            this.accounts = await this.runDepthSearch();
        }
    }

    async runBreadthSearch() {
        // during breadth search we always search the last known account index + 1
        const initialAccountIndex = this.accounts.length
            ? this.accounts[this.accounts.length - 1].index + 1
            : 0;
        // during breadth search we always search starting from the address index 0
        const initialAddressIndex = 0;

        const searchedAccounts = await this.recoverAccounts(
            initialAccountIndex,
            this.accountGapLimit,
            initialAddressIndex,
            this.addressGapLimit,
        );
        return this.accounts.concat(searchedAccounts);
    }

    async runDepthSearch() {
        const depthAccounts = this.accounts;
        // if we have no accounts yet, we start with the first account
        if (!depthAccounts.length) {
            for (let accountIndex = 0; accountIndex < this.accountGapLimit; accountIndex++) {
                depthAccounts.push({
                    index: accountIndex,
                    addresses: [],
                });
            }
        }
        for (const account of depthAccounts) {
            const searchedAccounts = await this.recoverAccounts(
                account.index,
                0,
                account.addresses.length,
                this.addressGapLimit,
            );
            depthAccounts[account.index].addresses = depthAccounts[account.index].addresses.concat(
                searchedAccounts[account.index].addresses,
            );
        }
        return depthAccounts;
    }

    // Generic low level function that can be used to implement different search algorithms
    async recoverAccounts(
        accountStartIndex: number = 0,
        accountGapLimit: number = 0,
        addressStartIndex: number = 0,
        addressGapLimit: number = 0,
    ) {
        const network = await NetworkEnv.getActiveNetwork();
        this.client = new IotaClient({
            url: network.customRpcUrl ? network.customRpcUrl : getFullnodeUrl(network.network),
        });
        const accounts: AccountFromFinder[] = [];
        let targetAccountIndex = accountStartIndex + accountGapLimit;
        for (
            let accountIndex = accountStartIndex;
            accountIndex < targetAccountIndex;
            accountIndex++
        ) {
            const account = this.accounts.find((acc) => acc.index === accountIndex) ?? {
                index: accountIndex,
                addresses: [],
            };

            let targetAddressIndex = addressStartIndex + addressGapLimit;
            for (
                let addressIndex = addressStartIndex;
                addressIndex < targetAddressIndex;
                addressIndex++
            ) {
                const addresses = await this.searchBalances(accountIndex, addressIndex);
                if (addresses.some((addr) => addr.balance && hasBalance(addr.balance))) {
                    targetAddressIndex = addressIndex + 1 + addressGapLimit;
                }
                account.addresses.push(addresses);
            }
            if (
                account.addresses.some((addrIndexSet) =>
                    addrIndexSet.some((address) => address.balance && hasBalance(address.balance)),
                )
            ) {
                targetAccountIndex = accountIndex + 1 + accountGapLimit;
            }
            accounts.push(account);
        }
        return accounts;
    }

    async searchBalances(accountIndex: number, addressIndex: number) {
        const changeIndexes = [0, 1]; // in the past the change indexes were used as 0=deposit & 1=internal
        const addresses = [];
        for (const changeIndex of changeIndexes) {
            const bipPath = {
                bip44CoinType: this.bip44CoinType,
                accountIndex: accountIndex,
                addressIndex: addressIndex,
                changeIndex,
            };
            const pubKeyHash = await this.getPublicKey(bipPath);

            const balance = await this.getBalance({
                owner: pubKeyHash,
                coinType: this.coinType,
            });
            addresses.push({
                pubKeyHash,
                bipPath,
                balance,
            });
        }
        return addresses;
    }

    async getPublicKey(options: MakeDerivationOptions) {
        const accountSource = await getAccountSourceByID(this.sourceID);

        if (!accountSource) {
            throw new Error('Could not find account source');
        }

        const pubKey = await accountSource.derivePubKey(options);
        return pubKey?.toIotaAddress();
    }

    async getBalance(params: GetBalanceParams): Promise<CoinBalance | undefined> {
        return this.client?.getBalance(params);
    }

    getResults(): AddressFromFinder[] {
        return this.accounts
            .flatMap((acc) => acc.addresses.flat())
            .filter((addr) => hasBalance(addr.balance));
    }
}

const accountsFinder = new AccountsFinder();
export default accountsFinder;
