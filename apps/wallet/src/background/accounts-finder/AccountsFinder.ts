// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AccountFromFinder, type AddressFromFinder } from '_src/shared/accounts';
import { recoverAccounts } from './accounts-finder';
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

        this.accounts = await recoverAccounts(
            0,
            accountGapLimit,
            addressGaspLimit,
            this.accounts,
            coinType,
            client,
            gasTypeArg,
            async (options) => {
                const pubKey = await accountSource?.derivePubKey(options);
                return pubKey.toIotaAddress();
            },
        );
    }

    getResults(accountGapLimit: number): AddressFromFinder[] {
        const addresses = this.accounts.flatMap((acc) => acc.addresses.flat());
        return addresses.slice(addresses.length - accountGapLimit);
    }
}

const accountsFinder = new AccountsFinder();
export default accountsFinder;
