// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type LedgerAccountSerializedUI } from '_src/background/accounts/LedgerAccount';
import type IotaLedgerClient from '@mysten/ledgerjs-hw-app-iota';
import { Ed25519PublicKey } from '@mysten/iota.js/keypairs/ed25519';
import { useQuery, type UseQueryOptions } from '@tanstack/react-query';

import { useIotaLedgerClient } from './IotaLedgerClientProvider';

export type DerivedLedgerAccount = Pick<
	LedgerAccountSerializedUI,
	'address' | 'publicKey' | 'type' | 'derivationPath'
>;
type UseDeriveLedgerAccountOptions = {
	numAccountsToDerive: number;
} & Pick<UseQueryOptions<DerivedLedgerAccount[], unknown>, 'select'>;

export function useDeriveLedgerAccounts(options: UseDeriveLedgerAccountOptions) {
	const { numAccountsToDerive, ...useQueryOptions } = options;
	const { iotaLedgerClient } = useIotaLedgerClient();

	return useQuery({
		// eslint-disable-next-line @tanstack/query/exhaustive-deps
		queryKey: ['derive-ledger-accounts'],
		queryFn: () => {
			if (!iotaLedgerClient) {
				throw new Error("The Iota application isn't open on a connected Ledger device");
			}
			return deriveAccountsFromLedger(iotaLedgerClient, numAccountsToDerive);
		},
		...useQueryOptions,
		gcTime: 0,
	});
}

async function deriveAccountsFromLedger(
	iotaLedgerClient: IotaLedgerClient,
	numAccountsToDerive: number,
) {
	const ledgerAccounts: DerivedLedgerAccount[] = [];
	const derivationPaths = getDerivationPathsForLedger(numAccountsToDerive);

	for (const derivationPath of derivationPaths) {
		const publicKeyResult = await iotaLedgerClient.getPublicKey(derivationPath);
		const publicKey = new Ed25519PublicKey(publicKeyResult.publicKey);
		const iotaAddress = publicKey.toIotaAddress();
		ledgerAccounts.push({
			type: 'ledger',
			address: iotaAddress,
			derivationPath,
			publicKey: publicKey.toBase64(),
		});
	}

	return ledgerAccounts;
}

function getDerivationPathsForLedger(numDerivations: number) {
	return Array.from({
		length: numDerivations,
	}).map((_, index) => `m/44'/4218'/${index}'/0'/0'`);
}
