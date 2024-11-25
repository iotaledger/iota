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
    ImageType,
} from '@iota/apps-ui-kit';
import { CoinIcon, ImageIconSize, ValidatorApyData } from '@iota/core';
import { Validator } from './Validator';
import { StakingRewardDetails } from './StakingRewardDetails';
import { Layout, LayoutBody, LayoutFooter } from './Layout';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

interface FinishStakingViewProps {
    validatorAddress: string;
    gasBudget: string | number | null | undefined;
    onConfirm: () => void;
    amount: string;
    symbol: string | undefined;
    validatorApy?: ValidatorApyData | null;
    onClose: () => void;
    showActiveStatus?: boolean;
}

export function FinishStakingView({
    validatorAddress,
    onConfirm,
    amount,
    symbol,
    onClose,
    validatorApy,
    gasBudget,
    showActiveStatus,
}: FinishStakingViewProps): React.JSX.Element {
    return (
        <Layout>
            <Header title="Transaction" onClose={onClose} />
            <LayoutBody>
                <div className="flex flex-col gap-y-lg">
                    <Validator
                        address={validatorAddress}
                        showActiveStatus={showActiveStatus}
                        isSelected
                        showAction={false}
                    />

                    <Card type={CardType.Outlined}>
                        <CardImage type={ImageType.BgSolid}>
                            <CoinIcon
                                hasCoinWrapper
                                coinType={IOTA_TYPE_ARG}
                                rounded
                                size={ImageIconSize.Small}
                            />
                        </CardImage>
                        <CardBody title={`${amount} ${symbol}`} subtitle="Stake" />
                    </Card>

                    <StakingRewardDetails validatorApy={validatorApy} gasBudget={gasBudget} />
                </div>
            </LayoutBody>

            <LayoutFooter>
                <div className="flex w-full">
                    <Button type={ButtonType.Primary} fullWidth onClick={onConfirm} text="Finish" />
                </div>
            </LayoutFooter>
        </Layout>
    );
}
