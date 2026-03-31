// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type LedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import type IotaLedgerClient from '@iota/ledgerjs-hw-app-iota';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { useMutation, useQuery } from '@tanstack/react-query';

import { useIotaLedgerClient } from './IotaLedgerClientProvider';
import { AccountType } from '_src/background/accounts/account';
import { useState } from 'react';

type LedgerAccountKeys = 'address' | 'publicKey' | 'type' | 'derivationPath';

export type DerivedLedgerAccount = Pick<LedgerAccountSerializedUI, LedgerAccountKeys>;
interface UseDeriveLedgerAccountOptions {
    chunkSize: number;
}

export function useDeriveLedgerAccounts({ chunkSize }: UseDeriveLedgerAccountOptions) {
    const { iotaLedgerClient } = useIotaLedgerClient();
    const [accounts, setAccounts] = useState<DerivedLedgerAccount[]>([]);
    const [chunk, setChunk] = useState(0);

    const mainPublicKey = useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['derive-main-public-key-ledger'],
        queryFn: async () => {
            if (!iotaLedgerClient) {
                throw new Error("The IOTA application isn't open on a connected Ledger device");
            }
            const data = await deriveMainAccountFromLedger(iotaLedgerClient);
            await advance.mutateAsync();
            return data;
        },
        gcTime: 0,
        staleTime: 0,
        enabled: !!iotaLedgerClient,
    });

    const advance = useMutation({
        mutationFn: async () => {
            if (!iotaLedgerClient) {
                throw new Error("The IOTA application isn't open on a connected Ledger device");
            }

            const newAccounts = await deriveAccountsFromLedger(iotaLedgerClient, chunk, chunkSize);

            setAccounts((accs) => [...accs, ...newAccounts]);
            setChunk((c) => c + 1);
        },
    });

    return { mainPublicKey, accounts, advance };
}

async function deriveMainAccountFromLedger(iotaLedgerClient: IotaLedgerClient) {
    const mainPublicKeyResult = await iotaLedgerClient.getPublicKey(`m/44'/4218'/0'/0'/0'`);
    const mainPublicKey = new Ed25519PublicKey(mainPublicKeyResult.publicKey);

    return mainPublicKey.toBase64();
}

async function deriveAccountsFromLedger(
    iotaLedgerClient: IotaLedgerClient,
    chunk: number,
    chunkSize: number,
) {
    const accounts: DerivedLedgerAccount[] = [];
    const derivationPaths = getDerivationPathsForLedger(chunk * chunkSize, chunkSize);

    for (const derivationPath of derivationPaths) {
        const publicKeyResult = await iotaLedgerClient.getPublicKey(derivationPath);
        const publicKey = new Ed25519PublicKey(publicKeyResult.publicKey);
        const iotaAddress = publicKey.toIotaAddress();
        accounts.push({
            type: AccountType.LedgerDerived,
            address: iotaAddress,
            derivationPath,
            publicKey: publicKey.toBase64(),
        });
    }

    return accounts;
}

function getDerivationPathsForLedger(startIndex: number, numDerivations: number) {
    return Array.from({
        length: numDerivations,
    }).map((_, index) => `m/44'/4218'/${startIndex + index}'/0'/0'`);
}
