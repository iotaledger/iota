// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useBackgroundClient } from './useBackgroundClient';
import { useQueryClient } from '@tanstack/react-query';
import { IOTA_COIN_TYPE_ID, GAS_TYPE_ARG } from '../redux/slices/iota-objects/Coin';
import { AccountsFinder, type AllowedAccountTypes } from '_src/ui/app/accounts-finder';
import { useIotaClient } from '@iota/dapp-kit';
import { useIotaLedgerClient } from '../components/ledger/IotaLedgerClientProvider';
import { useMemo } from 'react';
import type {
    SourceStrategyToFind,
    SourceStrategyToPersist,
} from '_src/shared/messaging/messages/payloads/accounts-finder';
import { makeDerivationPath } from '_src/background/account-sources/bip44Path';
import { Ed25519PublicKey } from '@iota/iota.js/keypairs/ed25519';

export interface UseAccountFinderOptions {
    accountType: AllowedAccountTypes;
    coinType?: number;
    gasType?: string;
    accountGapLimit?: number;
    addressGapLimit?: number;
    sourceStrategy: SourceStrategyToFind;
}

export function useAccountsFinder({
    coinType = IOTA_COIN_TYPE_ID,
    gasType = GAS_TYPE_ARG,
    addressGapLimit,
    accountGapLimit,
    sourceStrategy,
    accountType,
}: UseAccountFinderOptions) {
    const backgroundClient = useBackgroundClient();
    const queryClient = useQueryClient();
    const ledgerIotaClinet = useIotaLedgerClient();
    const client = useIotaClient();

    const accountFinder = useMemo(() => {
        if (sourceStrategy.type === 'ledger' && !ledgerIotaClinet.iotaLedgerClient) {
            ledgerIotaClinet.connectToLedger();
        }
        return new AccountsFinder({
            client,
            accountType,
            bip44CoinType: coinType,
            coinType: gasType,
            accountGapLimit,
            addressGapLimit,
            getPublicKey: async (bipPath) => {
                if (sourceStrategy.type == 'ledger') {
                    // Retrieve the public key using the ledger client
                    const client = ledgerIotaClinet.iotaLedgerClient!;
                    const derivationPath = makeDerivationPath(bipPath);
                    const publicKeyResult = await client?.getPublicKey(derivationPath);
                    const publicKey = new Ed25519PublicKey(publicKeyResult.publicKey);
                    return publicKey.toIotaAddress();
                } else {
                    // Retrieve the public key using the background client
                    const { address } = await backgroundClient.deriveBipPathAccountsFinder(
                        sourceStrategy.sourceID,
                        {
                            ...bipPath,
                            coinType,
                        },
                    );
                    return address;
                }
            },
        });
    }, [
        client,
        backgroundClient,
        accountType,
        coinType,
        gasType,
        accountGapLimit,
        addressGapLimit,
        sourceStrategy,
    ]);

    async function find() {
        const bipPaths = await accountFinder.find();

        let sourceStrategyToPersist: SourceStrategyToPersist | undefined = undefined;

        if (sourceStrategy.type == 'ledger') {
            // Generate addresses with ledger client
            const client = ledgerIotaClinet.iotaLedgerClient!;
            const addresses = await Promise.all(
                bipPaths.map(async (bipPath) => {
                    const derivationPath = makeDerivationPath(bipPath);
                    const publicKeyResult = await client.getPublicKey(derivationPath);
                    const publicKey = new Ed25519PublicKey(publicKeyResult.publicKey);
                    return {
                        address: publicKey.toIotaAddress(),
                        publicKey: publicKey.toString(),
                        derivationPath,
                    };
                }),
            );

            sourceStrategyToPersist = {
                ...sourceStrategy,
                addresses,
            };
        } else {
            sourceStrategyToPersist = {
                ...sourceStrategy,
                bipPaths,
            };
        }

        // Persist accounts
        await backgroundClient.persistAccountsFinder(sourceStrategyToPersist);

        queryClient.invalidateQueries({
            queryKey: ['accounts-finder-results'],
        });
    }

    return {
        find,
    };
}
