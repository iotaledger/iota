// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink, ExplorerLinkType, TxnAmount } from '_components';
import { useActiveAddress } from '_hooks';
import { CoinFormat, parseAmount, useCoinMetadata, useFormatCoin } from '@iota/core';
import { Divider, KeyValueInfo } from '@iota/apps-ui-kit';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

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
    const [formattedGasBudgetEstimation, gasToken] = useFormatCoin(
        gasBudget,
        IOTA_TYPE_ARG,
        CoinFormat.FULL,
    );

    return (
        <div className="flex w-full flex-col gap-md">
            <TxnAmount
                amount={amountWithoutDecimals}
                coinType={coinType}
                subtitle="Amount"
                approximation={approximation}
            />
            <div className="flex flex-col gap-md--rs p-sm--rs">
                <KeyValueInfo
                    keyText={'From'}
                    value={
                        <ExplorerLink
                            type={ExplorerLinkType.Address}
                            address={accountAddress || ''}
                        >
                            {formatAddress(accountAddress || '')}
                        </ExplorerLink>
                    }
                    fullwidth
                />

                <Divider />
                <KeyValueInfo
                    keyText={'To'}
                    value={
                        <ExplorerLink type={ExplorerLinkType.Address} address={to || ''}>
                            {formatAddress(to || '')}
                        </ExplorerLink>
                    }
                    fullwidth
                />

                <Divider />
                <KeyValueInfo
                    keyText={'Est. Gas Fees'}
                    value={formattedGasBudgetEstimation}
                    supportingLabel={gasToken}
                    fullwidth
                />
            </div>
        </div>
    );
}
