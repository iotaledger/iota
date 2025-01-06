// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import { getIotaClient } from '_src/shared/iota-client';
import { getDefaultNetwork, Network, type IotaClient } from '@iota/iota-sdk/client';

import type { BackgroundClient } from './background-client';
import { BackgroundServiceSigner } from './background-client/BackgroundServiceSigner';
import { queryClient } from './helpers/queryClient';
import { type WalletSigner } from './WalletSigner';

const ACCOUNT_TYPES_WITH_BACKGROUND_SIGNER: AccountType[] = [
    AccountType.MnemonicDerived,
    AccountType.SeedDerived,
    AccountType.PrivateKeyDerived,
];

class ApiProvider {
    private _apiFullNodeProvider?: IotaClient;
    private _signerByAddress: Map<string, WalletSigner> = new Map();
    network = getDefaultNetwork();

    public setNewJsonRpcProvider(
        network: Network = getDefaultNetwork(),
        customRPC?: string | null,
    ) {
        this.network = network;
        this._apiFullNodeProvider = getIotaClient(
            network === Network.Custom
                ? { network, customRpcUrl: customRPC || '' }
                : { network, customRpcUrl: null },
        );

        this._signerByAddress.clear();

        // We also clear the query client whenever set set a new API provider:
        queryClient.resetQueries();
        queryClient.clear();
    }

    public get instance() {
        if (!this._apiFullNodeProvider) {
            this.setNewJsonRpcProvider();
        }
        return {
            // eslint-disable-next-line @typescript-eslint/no-non-null-assertion
            fullNode: this._apiFullNodeProvider!,
        };
    }

    public getSignerInstance(
        account: SerializedUIAccount,
        backgroundClient: BackgroundClient,
    ): WalletSigner {
        if (!this._apiFullNodeProvider) {
            this.setNewJsonRpcProvider();
        }
        if (ACCOUNT_TYPES_WITH_BACKGROUND_SIGNER.includes(account.type)) {
            return this.getBackgroundSignerInstance(account, backgroundClient);
        }
        if (AccountType.LedgerDerived === account.type) {
            // Ideally, Ledger transactions would be signed in the background
            // and exist as an asynchronous keypair; however, this isn't possible
            // because you can't connect to a Ledger device from the background
            // script. Similarly, the signer instance can't be retrieved from
            // here because ApiProvider is a global and results in very buggy
            // behavior due to the reactive nature of managing Ledger connections
            // and displaying relevant UI updates. Refactoring ApiProvider to
            // not be a global instance would help out here, but that is also
            // a non-trivial task because we need access to ApiProvider in the
            // background script as well.
            throw new Error("Signing with Ledger via ApiProvider isn't supported");
        }
        throw new Error('Encountered unknown account type');
    }

    public getBackgroundSignerInstance(
        account: SerializedUIAccount,
        backgroundClient: BackgroundClient,
    ): WalletSigner {
        const key = account.id;
        if (!this._signerByAddress.has(account.id)) {
            this._signerByAddress.set(
                key,
                new BackgroundServiceSigner(account, backgroundClient, this._apiFullNodeProvider!),
            );
        }
        return this._signerByAddress.get(key)!;
    }
}

export const walletApiProvider = new ApiProvider();
