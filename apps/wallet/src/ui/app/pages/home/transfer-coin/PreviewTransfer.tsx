// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink, ExplorerLinkType, TxnAmount } from '_components';
import { useActiveAddress } from '_hooks';
import {
    NamedAddressTooltip,
    useCoinMetadata,
    useFormatCoin,
    useGetIotaNameRecord,
} from '@iota/core';
import { Divider, KeyValueInfo } from '@iota/apps-ui-kit';
import { CoinFormat, formatAddress, IOTA_TYPE_ARG, parseAmount } from '@iota/iota-sdk/utils';

export type PreviewTransferProps = {
    coinType: string;
    to: string;
    amount: string;
    coinBalance: string;
    gasBudget?: string;
};

export function PreviewTransfer({
    coinType,
    to,
    amount,
    coinBalance,
    gasBudget,
}: PreviewTransferProps) {
    const { data: nameRecord } = useGetIotaNameRecord(to);
    const accountAddress = useActiveAddress();
    const { data: metadata } = useCoinMetadata(coinType);
    const amountWithoutDecimals = parseAmount(amount, metadata?.decimals ?? 0);

    const approximation =
        amountWithoutDecimals === BigInt(coinBalance) && coinType === IOTA_TYPE_ARG;

    const [formattedGasBudgetEstimation, gasToken] = useFormatCoin({
        balance: gasBudget,
        format: CoinFormat.Full,
    });

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
                            eventType="address"
                        >
                            <span data-amp-mask>{formatAddress(accountAddress || '')}</span>
                        </ExplorerLink>
                    }
                    fullwidth
                />

                <Divider />
                <KeyValueInfo
                    keyText={'To'}
                    value={
                        <NamedAddressTooltip
                            address={nameRecord?.targetAddress || to}
                            name={nameRecord?.name}
                        >
                            <ExplorerLink
                                type={ExplorerLinkType.Address}
                                address={nameRecord?.targetAddress || to}
                                eventType="address"
                            >
                                <span data-amp-mask>
                                    {nameRecord ? nameRecord.name : formatAddress(to || '')}
                                </span>
                            </ExplorerLink>
                        </NamedAddressTooltip>
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
