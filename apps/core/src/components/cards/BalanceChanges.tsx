// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useMemo } from 'react';
import { Divider, Header, KeyValueInfo, Panel } from '@iota/apps-ui-kit';
import type { BalanceChangeSummary, RenderExplorerLink } from '../../types';
import { ExplorerLinkType } from '../../enums';
import { formatAddress } from '@iota/iota-sdk/utils';
import { CoinItem } from '../coin';
import { RecognizedBadge } from '@iota/ui-icons';
import { getRecognizedUnRecognizedTokenChanges } from '../../utils';
import { BalanceChange } from '../../interfaces';

interface BalanceChangesProps {
    changes?: BalanceChangeSummary;
    renderExplorerLink: RenderExplorerLink;
}

export function BalanceChanges({ changes, renderExplorerLink: ExplorerLink }: BalanceChangesProps) {
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

function BalanceChangeEntry({ change }: { change: BalanceChange }) {
    const { amount, coinType, unRecognizedToken } = change;
    return (
        <CoinItem
            coinType={coinType}
            balance={BigInt(amount)}
            icon={
                unRecognizedToken ? undefined : (
                    <RecognizedBadge className="h-4 w-4 text-primary-40" />
                )
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
