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
interface RecoverAccountParams {
    accountStartIndex: number;
    accountGapLimit: number;
    addressStartIndex: number;
    addressGapLimit: number;
}
enum AllowedBip44CoinTypes {
    IOTA = 4218,
    Shimmer = 4219,
}
export enum SearchAlgorithm {
    BREADTH,
    DEPTH,
    ITERATIVE_DEEPENING_BREADTH_FIRST,
}
// Note: we exclude private keys for the account finder because more addresses cant be derived from them
type AllowedAccountTypes = Exclude<AccountType, AccountType.Imported>;
type GapConfigurationByCoinType = {
    [key in AllowedAccountTypes]: GapConfiguration;
};
const GAP_CONFIGURATION: { [key in AllowedBip44CoinTypes]: GapConfigurationByCoinType } = {
    // in IOTA we have chrysalis users which could have rotated addresses
    [AllowedBip44CoinTypes.IOTA]: {
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
    },
    // In shimmer we focus on accounts indexes and never rotate addresses
    [AllowedBip44CoinTypes.Shimmer]: {
        [AccountType.Ledger]: {
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
    // configured with parameters at init()
    private algorithm: SearchAlgorithm = SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST;
    private bip44CoinType: AllowedBip44CoinTypes = AllowedBip44CoinTypes.IOTA;
    private coinType: string = '';
    private sourceID: string = '';
    // configured through predefined values at init()
    private accountGapLimit: number = 0;
    private addressGapLimit: number = 0;
    private changeIndexes: number[] = [0];

    public client: IotaClient | null = null;

    accounts: AccountFromFinder[] = [];

    init(
        accountType: AllowedAccountTypes,
        algorithm: SearchAlgorithm,
        bip44CoinType: AllowedBip44CoinTypes,
        coinType: string,
        sourceID: string,
    ) {
        this.accounts = [];

        this.algorithm = algorithm;
        this.bip44CoinType = bip44CoinType;
        this.coinType = coinType;
        this.sourceID = sourceID;

        this.accountGapLimit = GAP_CONFIGURATION[bip44CoinType][accountType].accountGapLimit; // this could configured too by the user
        this.addressGapLimit = GAP_CONFIGURATION[bip44CoinType][accountType].addressGapLimit; // this could configured too by the user
        this.changeIndexes = CHANGE_INDEXES[bip44CoinType]; // this could configured too by the user;
    }

    async find() {
        if (
            this.algorithm === SearchAlgorithm.BREADTH ||
            this.algorithm === SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST
        ) {
            this.accounts = await this.runBreadthSearch();
            // if we wanted, we could increase the account gap limit for example at this point
        }
        if (
            this.algorithm === SearchAlgorithm.DEPTH ||
            this.algorithm === SearchAlgorithm.ITERATIVE_DEEPENING_BREADTH_FIRST
        ) {
            this.accounts = await this.runDepthSearch();
            // if we wanted, we could increase the address gap limit for example at this point
        }
    }

    async runBreadthSearch() {
        // during breadth search we always start by searching for new account indexes
        const initialAccountIndex = this.accounts.length
            ? this.accounts[this.accounts.length - 1].index
            : 0;

        const searchedAccounts = await this.recoverAccounts({
            accountStartIndex: initialAccountIndex, // we start from the last existing account index
            accountGapLimit: this.accountGapLimit, // we search for the full account gap limit
            addressStartIndex: 0, // we start from the first address index
            addressGapLimit: this.addressGapLimit, // we search for the full address gap limit
        });
        // we merge the results into the existing accounts and persist them
        return this.accounts.concat(searchedAccounts);
    }

    async runDepthSearch() {
        const depthAccounts = this.accounts;
        // if we have no accounts yet, we populate with empty accounts
        if (!depthAccounts.length) {
            for (let accountIndex = 0; accountIndex < this.accountGapLimit; accountIndex++) {
                depthAccounts.push({
                    index: accountIndex,
                    addresses: [],
                });
            }
        }
        // depth search is done by searching for more addresses for each account in isolation
        for (const account of depthAccounts) {
            // during depth search we search for 1 account at a time and start from the last address index
            const searchedAccounts = await this.recoverAccounts({
                accountStartIndex: account.index, // we search for the current account
                accountGapLimit: 0, // we only search for 1 account
                addressStartIndex: account.addresses.length, // we start from the last address index
                addressGapLimit: this.addressGapLimit, // we search for the full address gap limit
            });
            // we merge the results into the existing accounts and persist them
            depthAccounts[account.index].addresses = depthAccounts[account.index].addresses.concat(
                searchedAccounts[account.index].addresses,
            );
        }
        return depthAccounts;
    }

    // This function simulates what the old SDK used to do.
    // Generic low level function that can be used to implement different search algorithms
    // In this case we use it to implement all 3 search algorithms, but it could be used for other models
    // Additionally, this function could be changed so that it behaves differently, impacting the whole search
    async recoverAccounts(
        params: RecoverAccountParams = {
            accountStartIndex: 0,
            accountGapLimit: 0,
            addressStartIndex: 0,
            addressGapLimit: 0,
        },
    ) {
        const { accountStartIndex, accountGapLimit, addressStartIndex, addressGapLimit } = params;

        const network = await NetworkEnv.getActiveNetwork();
        this.client = new IotaClient({
            url: network.customRpcUrl ? network.customRpcUrl : getFullnodeUrl(network.network),
        });

        const accounts: AccountFromFinder[] = [];
        let targetAccountIndex = accountStartIndex + accountGapLimit;
        // we search for accounts in the given range
        for (
            let accountIndex = accountStartIndex;
            accountIndex < targetAccountIndex;
            accountIndex++
        ) {
            const account = this.accounts.find((acc) => acc.index === accountIndex) ?? {
                index: accountIndex,
                addresses: [],
            };
            // on each fixed account index, we search for addresses in the given range
            let targetAddressIndex = addressStartIndex + addressGapLimit;
            for (
                let addressIndex = addressStartIndex;
                addressIndex < targetAddressIndex;
                addressIndex++
            ) {
                const addresses = await this.searchBalances(accountIndex, addressIndex);
                // if any of the addresses has a balance, we increase the target address index to keep searching
                if (addresses.some((addr) => addr.balance && hasBalance(addr.balance))) {
                    targetAddressIndex = addressIndex + 1 + addressGapLimit;
                }
                // we add the addresses to the account
                account.addresses.push(addresses);
            }
            // if any of the addresses of the given account has a balance,
            // we increase the target account index to keep searching
            if (
                account.addresses.some((addrIndexSet) =>
                    addrIndexSet.some((address) => address.balance && hasBalance(address.balance)),
                )
            ) {
                targetAccountIndex = accountIndex + 1 + accountGapLimit;
            }
            // we add the account to the list of accounts
            accounts.push(account);
        }
        return accounts;
    }

    async searchBalances(accountIndex: number, addressIndex: number) {
        const addresses = [];
        for (const changeIndex of this.changeIndexes) {
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
