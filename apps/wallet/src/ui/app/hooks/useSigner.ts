// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/account';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import { useIotaClient } from '@iota/dapp-kit';

import { walletApiProvider } from '../apiProvider';
import { useIotaLedgerClient } from '_components';
import { LedgerSigner } from '../ledgerSigner';
import { type WalletSigner } from '../walletSigner';
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
