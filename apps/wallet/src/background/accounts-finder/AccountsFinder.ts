// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AccountFromFinder, type AddressFromFinder } from '_src/shared/accounts';
import { findAccounts, hasBalance } from './accounts-finder';
import NetworkEnv from '../NetworkEnv';
import { IotaClient, getFullnodeUrl } from '@iota/iota.js/client';
import { getAccountSourceByID } from '../account-sources';

class AccountsFinder {
    accounts: AccountFromFinder[] = [];

    init() {
        this.accounts = [];
    }

    async findMore(
        coinType: number,
        gasTypeArg: string,
        sourceID: string,
        accountGapLimit: number,
        addressGaspLimit: number,
    ) {
        const network = await NetworkEnv.getActiveNetwork();
        const client = new IotaClient({
            url: network.customRpcUrl ? network.customRpcUrl : getFullnodeUrl(network.network),
        });

        const accountSource = await getAccountSourceByID(sourceID);

        if (!accountSource) {
            throw new Error('Could not find account source');
        }

        this.accounts = await findAccounts(
            0,
            accountGapLimit,
            addressGaspLimit,
            this.accounts,
            coinType,
            async (_, params) => {
                return await client.getBalance(params);
            },
            gasTypeArg,
            async (options) => {
                const pubKey = await accountSource?.derivePubKey(options);
                return pubKey.toIotaAddress();
            },
        );
    }

    getResults(): AddressFromFinder[] {
        return this.accounts
            .flatMap((acc) => acc.addresses.flat())
            .filter((addr) => hasBalance(addr.balance));
    }
}

const accountsFinder = new AccountsFinder();
export default accountsFinder;
