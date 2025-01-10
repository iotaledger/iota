// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useBackgroundClient } from './useBackgroundClient';
import { AccountsFinder, type AllowedAccountSourceTypes } from '_src/ui/app/accounts-finder';
import { useIotaClient } from '@iota/dapp-kit';
import { useIotaLedgerClient } from '_components';
import { useMemo } from 'react';
import type {
    SourceStrategyToFind,
    SourceStrategyToPersist,
} from '_src/shared/messaging/messages/payloads/accounts-finder';
import { makeDerivationPath } from '_src/background/account-sources/bip44Path';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { IOTA_BIP44_COIN_TYPE } from '../redux/slices/iota-objects/coin';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export interface UseAccountFinderOptions {
    accountSourceType: AllowedAccountSourceTypes;
    bip44CoinType?: number;
    coinType?: string;
    accountGapLimit?: number;
    addressGapLimit?: number;
    sourceStrategy: SourceStrategyToFind;
}

export function useAccountsFinder({
    bip44CoinType = IOTA_BIP44_COIN_TYPE,
    coinType = IOTA_TYPE_ARG,
    addressGapLimit,
    accountGapLimit,
    sourceStrategy,
    accountSourceType,
}: UseAccountFinderOptions) {
    const backgroundClient = useBackgroundClient();
    const ledgerIotaClient = useIotaLedgerClient();
    const client = useIotaClient();

    const accountFinder = useMemo(() => {
        return new AccountsFinder({
            client,
            accountSourceType,
            bip44CoinType,
            coinType,
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
                            bip44CoinType,
                        },
                    );
                    return publicKey;
                }
            },
        });
    }, [
        client,
        backgroundClient,
        accountSourceType,
        coinType,
        bip44CoinType,
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
