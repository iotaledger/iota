// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { FormDataValues } from '../interfaces';
import {
    Button,
    Card,
    CardType,
    CardImage,
    ImageType,
    CardBody,
    CardAction,
    CardActionType,
    KeyValueInfo,
    Divider,
    ButtonType,
} from '@iota/apps-ui-kit';
import { formatAddress } from '@iota/iota-sdk/utils';
import { GAS_SYMBOL, CoinIcon, ImageIconSize, useFormatCoin, ExplorerLinkType } from '@iota/core';
import { Loader } from '@iota/ui-icons';
import { ExplorerLink } from '@/components';

interface ReviewValuesFormProps {
    formData: FormDataValues;
    senderAddress: string;
    isPending: boolean;
    executeTransfer: () => void;
    coinType: string;
    isPayAllIota?: boolean;
}

export function ReviewValuesFormView({
    formData: { amount, to, formattedAmount, gasBudgetEst },
    senderAddress,
    isPending,
    executeTransfer,
    coinType,
    isPayAllIota,
}: ReviewValuesFormProps): JSX.Element {
    const [formatAmount, symbol] = useFormatCoin(formattedAmount, coinType);
    return (
        <div className="flex h-full flex-col">
            <div className="flex h-full w-full flex-col gap-md">
                <div className="flex h-full flex-col justify-between">
                    <div className="h-full flex-1">
                        <div className="flex w-full flex-col gap-md">
                            {Number(amount) !== 0 ? (
                                <Card type={CardType.Filled}>
                                    <CardImage type={ImageType.BgSolid}>
                                        <CoinIcon
                                            coinType={coinType}
                                            size={ImageIconSize.Small}
                                            rounded
                                            hasCoinWrapper
                                        />
                                    </CardImage>
                                    <CardBody
                                        title={`${isPayAllIota ? '~' : ''}${formatAmount} ${symbol}`}
                                        subtitle="Amount"
                                    />
                                    <CardAction type={CardActionType.SupportingText} />
                                </Card>
                            ) : null}
                            <div className="flex flex-col gap-md--rs p-sm--rs">
                                <KeyValueInfo
                                    keyText={'From'}
                                    value={
                                        <ExplorerLink
                                            type={ExplorerLinkType.Address}
                                            address={senderAddress}
                                        >
                                            {formatAddress(senderAddress)}
                                        </ExplorerLink>
                                    }
                                    fullwidth
                                />

                                <Divider />
                                <KeyValueInfo
                                    keyText={'To'}
                                    value={
                                        <ExplorerLink type={ExplorerLinkType.Address} address={to}>
                                            {formatAddress(to || '')}
                                        </ExplorerLink>
                                    }
                                    fullwidth
                                />

                                <Divider />
                                <KeyValueInfo
                                    keyText={'Est. Gas Fees'}
                                    value={gasBudgetEst}
                                    supportingLabel={GAS_SYMBOL}
                                    fullwidth
                                />
                            </div>
                        </div>
                    </div>
                    <Button
                        type={ButtonType.Primary}
                        onClick={executeTransfer}
                        text="Send Now"
                        disabled={coinType === null || isPending}
                        fullWidth
                        icon={isPending ? <Loader className="animate-spin" /> : undefined}
                        iconAfterText
                    />
                </div>
            </div>
        </div>
    );
}
