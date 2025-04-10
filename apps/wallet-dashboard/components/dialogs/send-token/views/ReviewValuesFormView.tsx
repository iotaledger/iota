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
    Header,
} from '@iota/apps-ui-kit';
import { formatAddress } from '@iota/iota-sdk/utils';
import {
    CoinIcon,
    ImageIconSize,
    useFormatCoin,
    ExplorerLinkType,
    CoinFormat,
    useCoinMetadata,
    parseAmount,
} from '@iota/core';
import { Loader } from '@iota/apps-ui-icons';
import { ExplorerLink } from '@/components';
import { DialogLayoutBody, DialogLayoutFooter } from '../../layout';

interface ReviewValuesFormProps {
    formData: FormDataValues;
    senderAddress: string;
    isPending: boolean;
    executeTransfer: () => void;
    coinType: string;
    isPayAllIota?: boolean;
    onClose: () => void;
    onBack: () => void;
    totalGas: string | undefined;
}

export function ReviewValuesFormView({
    formData: { amount, to },
    senderAddress,
    isPending,
    executeTransfer,
    coinType,
    isPayAllIota,
    onClose,
    onBack,
    totalGas,
}: ReviewValuesFormProps): JSX.Element {
    const { data: metadata } = useCoinMetadata(coinType);
    const amountWithoutDecimals = parseAmount(amount, metadata?.decimals ?? 0);
    const [roundedAmount, symbol] = useFormatCoin({
        balance: amountWithoutDecimals,
        coinType,
        format: CoinFormat.ROUNDED,
    });

    const [gasFormatted, gasSymbol] = useFormatCoin({
        balance: totalGas,
        format: CoinFormat.FULL,
    });

    return (
        <>
            <Header title="Review & Send" onClose={onClose} onBack={onBack} />
            <DialogLayoutBody>
                <div className="flex w-full flex-col gap-md">
                    {Number(amount) !== 0 ? (
                        <Card type={CardType.Filled}>
                            <CardImage type={ImageType.BgSolid}>
                                <CoinIcon coinType={coinType} rounded size={ImageIconSize.Small} />
                            </CardImage>
                            <CardBody
                                title={`${isPayAllIota ? '~' : ''}${roundedAmount} ${symbol}`}
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
                            value={gasFormatted}
                            supportingLabel={gasSymbol}
                            fullwidth
                        />
                    </div>
                </div>
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <Button
                    type={ButtonType.Primary}
                    onClick={executeTransfer}
                    text="Send Now"
                    disabled={coinType === null || isPending}
                    fullWidth
                    icon={isPending ? <Loader className="animate-spin" /> : undefined}
                    iconAfterText
                />
            </DialogLayoutFooter>
        </>
    );
}
