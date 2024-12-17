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
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { CoinIcon, ImageIconSize, useFormatCoin, ExplorerLinkType, CoinFormat } from '@iota/core';
import { Loader } from '@iota/ui-icons';
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
}

export function ReviewValuesFormView({
    formData: { amount, to, gasBudgetEst },
    senderAddress,
    isPending,
    executeTransfer,
    coinType,
    isPayAllIota,
    onClose,
    onBack,
}: ReviewValuesFormProps): JSX.Element {
    const [roundedAmount, symbol] = useFormatCoin(amount, coinType, CoinFormat.ROUNDED);
    const [gasEstimated, gasSymbol] = useFormatCoin(gasBudgetEst, IOTA_TYPE_ARG);

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
                            value={gasEstimated}
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
