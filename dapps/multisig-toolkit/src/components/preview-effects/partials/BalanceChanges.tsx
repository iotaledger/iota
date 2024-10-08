// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { type BalanceChange } from '@iota/iota-sdk/src/client';

import { PreviewCard } from '../PreviewCard';
import { onChainAmountToFloat } from '../utils';

export function BalanceChanges({ changes }: { changes: BalanceChange[] }) {
    return (
        <div className="grid grid-cols-2 gap-4 even:bg-gray-900">
            {changes.map((change, index) => (
                <ChangedBalance key={index} change={change} />
            ))}
        </div>
    );
}

function ChangedBalance({ change }: { change: BalanceChange }) {
    const { data: coinMetadata } = useIotaClientQuery('getCoinMetadata', {
        coinType: change.coinType,
    });

    const amount = () => {
        if (!coinMetadata) return '-';
        const amt = onChainAmountToFloat(change.amount, coinMetadata.decimals);

        return `${amt && amt > 0.0 ? '+' : ''}${amt}`;
    };
    if (!coinMetadata) return <div>Loading...</div>;

    return (
        <PreviewCard.Root>
            <PreviewCard.Body>
                <>
                    {coinMetadata.iconUrl && (
                        <img src={coinMetadata.iconUrl as string} alt={coinMetadata.name} />
                    )}
                    <p>
                        <span
                            className={`${
                                Number(amount()) > 0.0 ? 'text-green-300' : 'text-red-700'
                            }`}
                        >
                            {amount()}{' '}
                        </span>{' '}
                        {coinMetadata.symbol} ({change.coinType})
                    </p>
                </>
            </PreviewCard.Body>
            <PreviewCard.Footer owner={change.owner} />
        </PreviewCard.Root>
    );
}
