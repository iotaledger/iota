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
    [x: number]: {
        start: number;
        end: number;
        accountMax: number;
    };
}

class AccountsFinder {
    private isFirstSearch: boolean = true;
    private lastColumn: number = 0;
    private searchBoundaries: SearchBoundaries = {};
    private accountGapLimit: number = 0;
    private addressGapLimit: number = 0;
    private coinType: number = 0;
    private gasTypeArg: string = '';
    private sourceID: string = '';
    public client: IotaClient | null = null;

    accounts: AccountFromFinder[] = [];

    init() {
        this.accounts = [];
    }

    setupInitialBoundaries() {
        for (let x = 0; x < this.accountGapLimit; x++) {
            this.addColumnBoundary(x);
        }

        this.isFirstSearch = false;
    }

    expandSearchBoundaries() {
        for (let x = 0; x <= this.lastColumn; x++) {
            const { end } = this.searchBoundaries[x];

            this.searchBoundaries[x].start = end + 1;
            this.searchBoundaries[x].end = end + this.addressGapLimit;
        }

        this.addColumnsToBoundaries(this.lastColumn);
    }

    addColumnsToBoundaries(columnIndex: number) {
        const columnsLeft = this.lastColumn - columnIndex;
        const columnsToAdd = this.accountGapLimit - columnsLeft;

        const fromIndex = this.lastColumn + 1;
        const toIndex = this.lastColumn + columnsToAdd;

        for (let i = fromIndex; i <= toIndex; i++) {
            this.addColumnBoundary(i);
        }
    }

    addColumnBoundary(columnIndex: number) {
        const accountLimitIndex = this.accountGapLimit - 1;
        this.searchBoundaries[columnIndex] = {
            start: 0,
            end: accountLimitIndex + this.addressGapLimit,
            accountMax: accountLimitIndex,
        };
        this.lastColumn = columnIndex;
    }

    updateSearchRange(x: number, y: number) {
        const { end, accountMax } = this.searchBoundaries[x];

        if (y <= accountMax) {
            // increase limit in column
            const accountLimitDiff = accountMax - y;
            const accountsToAdd = this.accountGapLimit - accountLimitDiff;
            this.searchBoundaries[x].accountMax += accountsToAdd;
            this.searchBoundaries[x].end += accountsToAdd;

            // add more columns
            this.addColumnsToBoundaries(x);
        } else if (y <= end) {
            const addressLimitDiff = end - y;
            const addressesToAdd = this.addressGapLimit - addressLimitDiff;
            this.searchBoundaries[x].end += addressesToAdd;
        }
    }

    async searchByColumn(x: number) {
        const res: string[] = [];
        const account: AccountFromFinder = {
            index: x,
            addresses: [],
        };
        const { start, end } = this.searchBoundaries[x];
        for (let y = start; y <= end; y++) {
            const changeIndexes = [0, 1]; // in the past the change indexes were used as 0=deposit & 1=internal

            for (const changeIndex of changeIndexes) {
                const bipPath = {
                    coinType: this.coinType,
                    accountIndex: x,
                    addressIndex: y,
                    changeIndex,
                };
                const pubKeyHash = await this.getPublicKey(bipPath);

                const balance = await this.getBalance({
                    owner: pubKeyHash,
                    coinType: this.gasTypeArg,
                });

                if (balance && hasBalance(balance)) {
                    this.updateSearchRange(x, y);

                    if (!account.addresses[y]) {
                        account.addresses[y] = [];
                    }

                    account.addresses[y][changeIndex] = {
                        pubKeyHash,
                        bipPath: {
                            addressIndex: y,
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

        for (let x = 0; x <= this.lastColumn; x++) {
            await this.searchByColumn(x);
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
