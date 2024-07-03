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

interface SearchRange {
    [x: number]: {
        start: number;
        end: number;
        accountGapLimit: number;
    };
}

class AccountsFinder {
    private isSearchFirstTime: boolean = true;
    private xLastIndex: number = 0;
    private searchRange: SearchRange = {};
    private accountGapLimit: number = 0;
    private addressGapLimit: number = 0;
    private coinType: number = 0;
    private gasTypeArg: string = '';
    private sourceID: string = '';
    private client: IotaClient | null = null;

    accounts: AccountFromFinder[] = [];
    counter: number = 0;

    init() {
        this.accounts = [];
    }

    initSearchRange() {
        for (let x = 0; x < this.accountGapLimit; x++) {
            this.addSearchRangeColumn(x);
        }

        this.isSearchFirstTime = false;
    }

    increaseSearchRange() {
        for (let x = 0; x <= this.xLastIndex; x++) {
            const { end } = this.searchRange[x];

            this.searchRange[x].start = end + 1;
            this.searchRange[x].end = end + this.addressGapLimit;
        }

        this.addSearchRangeColumns(this.xLastIndex);
    }

    addSearchRangeColumns(x: number) {
        const leftColumns = this.xLastIndex - x;
        const columnsToAdd = this.accountGapLimit - leftColumns;

        const fromIndex = this.xLastIndex + 1;
        const toIndex = this.xLastIndex + columnsToAdd;

        for (let i = fromIndex; i <= toIndex; i++) {
            this.addSearchRangeColumn(i);
        }
    }

    addSearchRangeColumn(x: number) {
        const accountGapLimitIndex = this.accountGapLimit - 1;
        this.searchRange[x] = {
            start: 0,
            end: accountGapLimitIndex + this.addressGapLimit,
            accountGapLimit: accountGapLimitIndex,
        };
        this.xLastIndex = x;
    }

    updateSearchRange(x: number, y: number) {
        const { end, accountGapLimit } = this.searchRange[x];

        if (y <= accountGapLimit) {
            // increase limit in column
            const accountGapLimitDiff = accountGapLimit - y;
            const accountGapNeedAdd = this.accountGapLimit - accountGapLimitDiff;
            this.searchRange[x].accountGapLimit += accountGapNeedAdd;
            this.searchRange[x].end += accountGapNeedAdd;

            this.addSearchRangeColumns(x);
        } else if (y <= end) {
            const addressGapLimitDiff = end - y;
            const addressGapNeedAdd = this.addressGapLimit - addressGapLimitDiff;
            this.searchRange[x].end += addressGapNeedAdd;
        }
    }

    async searchByColumn(x: number) {
        const res: string[] = [];
        const account: AccountFromFinder = {
            index: x,
            addresses: [],
        };
        for (let y = this.searchRange[x].start; y <= this.searchRange[x].end; y++) {
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

        if (this.isSearchFirstTime) {
            this.initSearchRange();
        } else {
            this.increaseSearchRange();
        }

        for (let x = 0; x <= this.xLastIndex; x++) {
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
