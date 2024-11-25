// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/LedgerAccount';
import { useIotaClient } from '@iota/dapp-kit';

import { walletApiProvider } from '../ApiProvider';
import { useIotaLedgerClient } from '_components';
import { LedgerSigner } from '../LedgerSigner';
import { type WalletSigner } from '../WalletSigner';
import { useBackgroundClient } from './useBackgroundClient';

export function useSigner(account: SerializedUIAccount | null): WalletSigner | null {
    const { connectToLedger } = useIotaLedgerClient();
    const api = useIotaClient();
    const background = useBackgroundClient();
    if (!account) {
        return null;
    }
    if (isLedgerAccountSerializedUI(account)) {
        return new LedgerSigner(connectToLedger, account.derivationPath, api);
    }
    return walletApiProvider.getSignerInstance(account, background);
}
