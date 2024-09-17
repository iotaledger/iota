// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink, ExplorerLinkType, TxnAmount } from '_components';
import { useActiveAddress } from '_src/ui/app/hooks/useActiveAddress';
import { GAS_SYMBOL } from '_src/ui/app/redux/slices/iota-objects/Coin';
import { parseAmount, useCoinMetadata } from '@iota/core';
import { Divider, KeyValueInfo } from '@iota/apps-ui-kit';

export type PreviewTransferProps = {
    coinType: string;
    to: string;
    amount: string;
    approximation?: boolean;
    gasBudget?: string;
};

export function PreviewTransfer({
    coinType,
    to,
    amount,
    approximation,
    gasBudget,
}: PreviewTransferProps) {
    const accountAddress = useActiveAddress();
    const { data: metadata } = useCoinMetadata(coinType);
    const amountWithoutDecimals = parseAmount(amount, metadata?.decimals ?? 0);

    return (
        <div className="flex w-full flex-col gap-md">
            <TxnAmount
                amount={amountWithoutDecimals}
                coinType={coinType}
                subtitle="Amount"
                approximation={approximation}
            />
            <div className="flex flex-col gap-md--rs p-sm--rs">
                <ExplorerLink type={ExplorerLinkType.Address} address={accountAddress || ''}>
                    <KeyValueInfo keyText={'From'} valueText={accountAddress || ''} fullwidth />
                </ExplorerLink>
                <Divider />
                <ExplorerLink type={ExplorerLinkType.Address} address={accountAddress || ''}>
                    <KeyValueInfo keyText={'To'} valueText={accountAddress || ''} fullwidth />
                </ExplorerLink>

                <Divider />
                <KeyValueInfo
                    keyText={'Est. Gas Fees'}
                    valueText={`${gasBudget} ${GAS_SYMBOL}`}
                    fullwidth
                />
            </div>
        </div>
    );
}
