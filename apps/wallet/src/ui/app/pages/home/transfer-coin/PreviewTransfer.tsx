// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinIcon } from '_components';
import { useActiveAddress } from '_src/ui/app/hooks/useActiveAddress';
import { GAS_SYMBOL } from '_src/ui/app/redux/slices/iota-objects/Coin';
import { parseAmount, useCoinMetadata, useFormatCoin } from '@iota/core';
import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    Divider,
    ImageType,
    KeyValueInfo,
} from '@iota/apps-ui-kit';
import { useAddressLink } from '_app/hooks/useAddressLink';

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
    const [formatted, symbol] = useFormatCoin(
        Math.abs(Number(amountWithoutDecimals.toString())),
        coinType,
    );

    const addrFrom = useAddressLink(accountAddress);
    const addrTo = useAddressLink(to);

    return (
        <div className="flex w-full flex-col gap-md">
            <Card type={CardType.Filled}>
                <CardImage type={ImageType.BgSolid}>
                    <div className="flex h-10 w-10 items-center justify-center rounded-full border border-shader-neutral-light-8  text-neutral-10">
                        <CoinIcon coinType={coinType} />
                    </div>
                </CardImage>
                <CardBody
                    title={`${approximation ? '~' : ''}${formatted} ${symbol}`}
                    subtitle="Amount"
                />
                <CardAction type={CardActionType.SupportingText} />
            </Card>
            <div className="flex flex-col gap-md--rs p-sm--rs">
                <KeyValueInfo
                    keyText={'From'}
                    valueText={addrFrom.address}
                    valueLink={addrFrom.explorerHref}
                />
                <Divider />
                <KeyValueInfo
                    keyText={'To'}
                    valueText={addrTo.address}
                    valueLink={addrTo.explorerHref}
                />
                <Divider />
                <KeyValueInfo keyText={'Est. Gas Fees'} valueText={`${gasBudget} ${GAS_SYMBOL}`} />
            </div>
        </div>
    );
}
