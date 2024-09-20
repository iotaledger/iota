// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    getRecognizedUnRecognizedTokenChanges,
    type BalanceChange,
    type BalanceChangeSummary,
} from '@iota/core';
import { useMemo } from 'react';

import { Badge, BadgeType, Divider, Header, KeyValueInfo, Panel } from '@iota/apps-ui-kit';
import { CoinItem, ExplorerLink, ExplorerLinkType } from '_src/ui/app/components';
import { formatAddress } from '@iota/iota-sdk/utils';

interface BalanceChangesProps {
    changes?: BalanceChangeSummary;
}

function BalanceChangeEntry({ change }: { change: BalanceChange }) {
    const { amount, coinType, unRecognizedToken } = change;
    return (
        <CoinItem
            coinType={coinType}
            balance={BigInt(amount)}
            icon={
                unRecognizedToken ? (
                    <Badge type={BadgeType.PrimarySoft} label="Unrecognized" />
                ) : undefined
            }
        />
    );
}

function BalanceChangeEntries({ changes }: { changes: BalanceChange[] }) {
    const { recognizedTokenChanges, unRecognizedTokenChanges } = useMemo(
        () => getRecognizedUnRecognizedTokenChanges(changes),
        [changes],
    );

    return (
        <>
            {recognizedTokenChanges.map((change) => (
                <BalanceChangeEntry change={change} key={change.coinType + change.amount} />
            ))}
            {unRecognizedTokenChanges.length > 0 && (
                <>
                    {unRecognizedTokenChanges.map((change, index) => (
                        <BalanceChangeEntry change={change} key={change.coinType + index} />
                    ))}
                </>
            )}
        </>
    );
}

export function BalanceChanges({ changes }: BalanceChangesProps) {
    if (!changes) return null;

    return (
        <>
            {Object.entries(changes).map(([owner, changes]) => {
                return (
                    <Panel key={owner} hasBorder>
                        <div className="flex flex-col gap-y-sm overflow-hidden rounded-xl">
                            <Header title="Balance Changes" />
                            <BalanceChangeEntries changes={changes} />
                            <div className="flex flex-col gap-y-sm px-md pb-md">
                                <Divider />
                                <KeyValueInfo
                                    keyText="Owner"
                                    value={
                                        <ExplorerLink
                                            type={ExplorerLinkType.Address}
                                            address={owner}
                                        >
                                            {formatAddress(owner)}
                                        </ExplorerLink>
                                    }
                                    fullwidth
                                />
                            </div>
                        </div>
                    </Panel>
                );
            })}
        </>
    );
}
