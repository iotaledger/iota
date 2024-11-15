// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Button,
    ButtonType,
    Card,
    CardBody,
    CardImage,
    CardType,
    Header,
    ImageShape,
    ImageType,
} from '@iota/apps-ui-kit';
import { ValidatorApyData } from '@iota/core';
import { Validator } from './Validator';
import { IotaLogoMark } from '@iota/ui-icons';
import { StakingTransactionDetails } from './StakingTransactionDetails';
import { Layout, LayoutBody, LayoutFooter } from './Layout';

interface SuccessScreenViewProps {
    validatorAddress: string;
    gasBudget: string | number | null | undefined;
    onConfirm: () => void;
    amount: string;
    symbol: string | undefined;
    validatorApy: ValidatorApyData;
    onClose: () => void;
}

export function SuccessScreenView({
    validatorAddress,
    onConfirm,
    amount,
    symbol,
    onClose,
    validatorApy: { apy, isApyApproxZero },
    gasBudget,
}: SuccessScreenViewProps): React.JSX.Element {
    return (
        <Layout>
            <Header title="Transaction" onClose={onClose} />
            <LayoutBody>
                <div className="flex flex-col gap-y-md">
                    <Validator address={validatorAddress} isSelected showAction={false} />

                    <Card type={CardType.Outlined}>
                        <CardImage type={ImageType.BgSolid} shape={ImageShape.Rounded}>
                            <IotaLogoMark className="h-5 w-5 text-neutral-10" />
                        </CardImage>
                        <CardBody title={`${amount} ${symbol}`} subtitle="Stake" />
                    </Card>

                    <StakingTransactionDetails
                        apy={apy}
                        isApyApproxZero={isApyApproxZero}
                        gasBudget={gasBudget}
                    />
                </div>
            </LayoutBody>

            <LayoutFooter>
                <div className="flex w-full">
                    <Button
                        type={ButtonType.Primary}
                        fullWidth
                        onClick={onConfirm}
                        text="Confirm & Exit"
                    />
                </div>
            </LayoutFooter>
        </Layout>
    );
}
