// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/LedgerAccount';
import { useSuiClient } from '@mysten/dapp-kit';

import { walletApiProvider } from '../ApiProvider';
import { useSuiLedgerClient } from '../components/ledger/SuiLedgerClientProvider';
import { LedgerSigner } from '../LedgerSigner';
import { type WalletSigner } from '../WalletSigner';
import { useBackgroundClient } from './useBackgroundClient';

export function useSigner(account: SerializedUIAccount | null): WalletSigner | null {
	const { connectToLedger } = useSuiLedgerClient();
	const api = useSuiClient();
	const background = useBackgroundClient();

	if (!account) {
		return null;
	}
	if (isLedgerAccountSerializedUI(account)) {
		return new LedgerSigner(connectToLedger, account.derivationPath, api);
	}
	return walletApiProvider.getSignerInstance(account, background);
}
