// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { TxnAddress } from '_components/receipt-card/TxnAddress';
import { TxnAmount } from '_components/receipt-card/TxnAmount';
import { useActiveAddress } from '_src/ui/app/hooks/useActiveAddress';
import { GAS_SYMBOL } from '_src/ui/app/redux/slices/iota-objects/Coin';
import { parseAmount, useCoinMetadata } from '@iota/core';

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
        <div className="divide-steel/20 flex w-full flex-col divide-x-0 divide-y divide-solid px-2.5">
            <TxnAmount
                amount={amountWithoutDecimals.toString()}
                label="Sending"
                coinType={coinType}
                approximation={approximation}
            />
            <TxnAddress address={accountAddress || ''} label="From" />
            <TxnAddress address={to} label="To" />
            <div className="mb-5 flex w-full justify-between gap-2 pt-3.5">
                <div className="flex gap-1">
                    <Text variant="body" color="gray-80" weight="medium">
                        Estimated Gas Fees
                    </Text>
                </div>
                <Text variant="body" color="gray-90" weight="medium">
                    {gasBudget} {GAS_SYMBOL}
                </Text>
            </div>
        </div>
    );
}
