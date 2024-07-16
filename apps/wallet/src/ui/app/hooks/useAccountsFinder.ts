// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useBackgroundClient } from './useBackgroundClient';
import { IOTA_COIN_TYPE_ID, GAS_TYPE_ARG } from '../redux/slices/iota-objects/Coin';
import { AccountsFinder, type AllowedAccountSourceTypes } from '_src/ui/app/accounts-finder';
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
    accountSourceType: AllowedAccountSourceTypes;
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
    accountSourceType: accountType,
}: UseAccountFinderOptions) {
    const backgroundClient = useBackgroundClient();
    const ledgerIotaClient = useIotaLedgerClient();
    const client = useIotaClient();

    const accountFinder = useMemo(() => {
        return new AccountsFinder({
            client,
            accountSourceType: accountType,
            bip44CoinType: coinType,
            coinType: gasType,
            accountGapLimit,
            addressGapLimit,
            getPublicKey: async (bipPath) => {
                if (sourceStrategy.type == 'ledger') {
                    // Retrieve the public key using the ledger client
                    const client = ledgerIotaClient.iotaLedgerClient!;
                    const derivationPath = makeDerivationPath(bipPath);
                    const publicKeyResult = await client?.getPublicKey(derivationPath);
                    const publicKey = new Ed25519PublicKey(publicKeyResult.publicKey);
                    return publicKey.toBase64();
                } else {
                    // Retrieve the public key using the background client
                    const { publicKey } = await backgroundClient.deriveBipPathAccountsFinder(
                        sourceStrategy.sourceID,
                        {
                            ...bipPath,
                            coinType,
                        },
                    );
                    return publicKey;
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
        const foundAddresses = await accountFinder.find();

        let sourceStrategyToPersist: SourceStrategyToPersist | undefined = undefined;

        if (sourceStrategy.type == 'ledger') {
            const addresses = await Promise.all(
                foundAddresses.map(async (address) => {
                    const derivationPath = makeDerivationPath(address.bipPath);
                    const publicKey = new Ed25519PublicKey(address.publicKey);
                    return {
                        address: publicKey.toIotaAddress(),
                        publicKey: publicKey.toBase64(),
                        derivationPath,
                    };
                }),
            );

            sourceStrategyToPersist = {
                ...sourceStrategy,
                addresses,
            };
        } else {
            const bipPaths = foundAddresses.map((address) => address.bipPath);
            sourceStrategyToPersist = {
                ...sourceStrategy,
                bipPaths,
            };
        }

        // Persist accounts
        await backgroundClient.persistAccountsFinder(sourceStrategyToPersist);
    }

    return {
        find,
    };
}
