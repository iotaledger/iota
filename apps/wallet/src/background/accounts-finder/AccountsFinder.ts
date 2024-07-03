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

interface SearchBoundaries {
    [accountIndex: number]: {
        start: number;
        end: number;
        accountMax: number;
    };
}

class AccountsFinder {
    /** indicate if user is searching for the first time */
    private isFirstSearch: boolean = true;

    /**
     * This property controls what range we need to use for search.
     * Params start, end, accountMax are used to control the search range.
     * - key - index of column or X position.
     * - start - index of start Y position
     * - end - end Y index for search
     * - accountMax - index of accountGapLimit.
     */
    private searchBoundaries: SearchBoundaries = {};

    /**
     * Index of last column in searchBoundaries.
     * This property uses to know how many columns we have in searchBoundaries.
     * */
    private lastAccountIndex: number = 0;

    /** account gap limit provided by findMore function */
    private accountGapLimit: number = 0;

    /** address gap limit provided by findMore function */
    private addressGapLimit: number = 0;

    /** 4218 for IOTA or 4219 for Shimmer */
    private coinType: number = 0;

    private gasTypeArg: string = '';
    private sourceID: string = '';
    public client: IotaClient | null = null;

    /** Found accounts with balances. */
    accounts: AccountFromFinder[] = [];

    /**
     * Initializes or resets the accounts array.
     */
    init() {
        this.accounts = [];
    }

    /**
     * Init the search boundaries for the first time.
     */
    setupInitialBoundaries() {
        for (let accountIndex = 0; accountIndex < this.accountGapLimit; accountIndex++) {
            this.addColumnBoundary(accountIndex);
        }

        this.isFirstSearch = false;
    }

    /**
     * Expand the search boundaries when we need to search more.
     * This function change search range for columns that already exist and add more columns to search.
     */
    expandSearchBoundaries() {
        for (let accountIndex = 0; accountIndex <= this.lastAccountIndex; accountIndex++) {
            const { end } = this.searchBoundaries[accountIndex];

            this.searchBoundaries[accountIndex].start = end + 1;
            this.searchBoundaries[accountIndex].end = end + this.addressGapLimit;
        }

        this.addColumnsToBoundaries(this.lastAccountIndex);
    }

    /**
     * Calculate how many columns we need to add to search boundaries.
     * If we have a match, we need to add always "this.accountGapLimit" number of columns.
     * @param currAccountIndex - index of current active column.
     */
    addColumnsToBoundaries(currAccountIndex: number) {
        const columnsLeft = this.lastAccountIndex - currAccountIndex;
        const columnsToAdd = this.accountGapLimit - columnsLeft;

        const fromIndex = this.lastAccountIndex + 1;
        const toIndex = this.lastAccountIndex + columnsToAdd;

        for (let i = fromIndex; i <= toIndex; i++) {
            this.addColumnBoundary(i);
        }
    }

    /**
     * Add a new column to search boundaries.
     * @param columnIndex - index of new column.
     */
    addColumnBoundary(columnIndex: number) {
        const accountLimitIndex = this.accountGapLimit - 1;
        this.searchBoundaries[columnIndex] = {
            start: 0,
            end: accountLimitIndex + this.addressGapLimit,
            accountMax: accountLimitIndex,
        };
        this.lastAccountIndex = columnIndex;
    }

    /**
     * Calculate if we need to add new columns or increase the search range.
     * @param currentAccountIndex - index of column/account that we are searching.
     * @param currentAddressIndex - index of row/address that we are searching.
     */
    updateSearchRange(currentAccountIndex: number, currentAddressIndex: number) {
        const { end, accountMax } = this.searchBoundaries[currentAccountIndex];

        if (currentAddressIndex <= accountMax) {
            // increase limit in column
            const accountLimitDiff = accountMax - currentAddressIndex;
            const accountsToAdd = this.accountGapLimit - accountLimitDiff;
            this.searchBoundaries[currentAccountIndex].accountMax += accountsToAdd;
            this.searchBoundaries[currentAccountIndex].end += accountsToAdd;

            // add more columns
            this.addColumnsToBoundaries(currentAccountIndex);
        } else if (currentAddressIndex <= end) {
            const addressLimitDiff = end - currentAddressIndex;
            const addressesToAdd = this.addressGapLimit - addressLimitDiff;
            this.searchBoundaries[currentAccountIndex].end += addressesToAdd;
        }
    }

    /**
     * Take the current column and check if balance exists.
     * @param accountIndex - index of column/account
     */
    async searchByColumn(accountIndex: number) {
        const res: string[] = [];
        const account: AccountFromFinder = {
            index: accountIndex,
            addresses: [],
        };
        const { start, end } = this.searchBoundaries[accountIndex];
        for (let addressIndex = start; addressIndex <= end; addressIndex++) {
            const changeIndexes = [0, 1]; // in the past the change indexes were used as 0=deposit & 1=internal

            for (const changeIndex of changeIndexes) {
                const bipPath = {
                    coinType: this.coinType,
                    accountIndex: accountIndex,
                    addressIndex: addressIndex,
                    changeIndex,
                };
                const pubKeyHash = await this.getPublicKey(bipPath);

                const balance = await this.getBalance({
                    owner: pubKeyHash,
                    coinType: this.gasTypeArg,
                });

                if (balance && hasBalance(balance)) {
                    this.updateSearchRange(accountIndex, addressIndex);

                    if (!account.addresses[addressIndex]) {
                        account.addresses[addressIndex] = [];
                    }

                    account.addresses[addressIndex][changeIndex] = {
                        pubKeyHash,
                        bipPath: {
                            addressIndex: addressIndex,
                            accountIndex: account.index,
                            changeIndex,
                        },
                        balance,
                    };

                    this.accounts.push(account);
                }
            }
        }

        return res;
    }

    /**
     * This function calls each time when user press "Search" button
     * @param coinType
     * @param gasTypeArg
     * @param sourceID
     * @param accountGapLimit
     * @param addressGaspLimit
     */
    async findMore(
        coinType: number,
        gasTypeArg: string,
        sourceID: string,
        accountGapLimit: number,
        addressGaspLimit: number,
    ) {
        const network = await NetworkEnv.getActiveNetwork();
        this.client = new IotaClient({
            url: network.customRpcUrl ? network.customRpcUrl : getFullnodeUrl(network.network),
        });

        this.coinType = coinType;
        this.gasTypeArg = gasTypeArg;
        this.accountGapLimit = accountGapLimit;
        this.addressGapLimit = addressGaspLimit;
        this.sourceID = sourceID;

        if (this.isFirstSearch) {
            this.setupInitialBoundaries();
        } else {
            this.expandSearchBoundaries();
        }

        for (let accountIndex = 0; accountIndex <= this.lastAccountIndex; accountIndex++) {
            await this.searchByColumn(accountIndex);
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
