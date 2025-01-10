// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type LedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import type IotaLedgerClient from '@iota/ledgerjs-hw-app-iota';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { useQuery, type UseQueryOptions } from '@tanstack/react-query';

import { useIotaLedgerClient } from './IotaLedgerClientProvider';
import { AccountType } from '_src/background/accounts/account';

type LedgerAccountKeys = 'address' | 'publicKey' | 'type' | 'derivationPath';

export type DerivedLedgerAccount = Pick<LedgerAccountSerializedUI, LedgerAccountKeys>;
interface UseDeriveLedgerAccountOptions
    extends Pick<UseQueryOptions<DerivedLedgerAccount[], unknown>, 'select'> {
    numAccountsToDerive: number;
}

export function useDeriveLedgerAccounts(options: UseDeriveLedgerAccountOptions) {
    const { numAccountsToDerive, ...useQueryOptions } = options;
    const { iotaLedgerClient } = useIotaLedgerClient();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['derive-ledger-accounts'],
        queryFn: () => {
            if (!iotaLedgerClient) {
                throw new Error("The IOTA application isn't open on a connected Ledger device");
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
            type: AccountType.LedgerDerived,
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
