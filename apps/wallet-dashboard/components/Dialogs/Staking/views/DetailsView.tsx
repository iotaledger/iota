// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ExtendedDelegatedStake,
    ImageIcon,
    ImageIconSize,
    useFormatCoin,
    formatPercentageDisplay,
    useValidatorInfo,
} from '@iota/core';
import {
    Header,
    Button,
    ButtonType,
    Card,
    CardBody,
    CardImage,
    CardType,
    Panel,
    KeyValueInfo,
    Badge,
    BadgeType,
    Divider,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { DialogLayout, DialogLayoutFooter, DialogLayoutBody } from '../../layout';
import toast from 'react-hot-toast';

interface StakeDialogProps {
    handleClose: () => void;
    handleStake: () => void;
    stakedDetails: ExtendedDelegatedStake;
    showActiveStatus?: boolean;
    handleUnstake?: () => void;
}

export function DetailsView({
    handleClose,
    handleUnstake,
    handleStake,
    stakedDetails,
    showActiveStatus,
}: StakeDialogProps): JSX.Element {
    const totalStake = BigInt(stakedDetails?.principal || 0n);
    const validatorAddress = stakedDetails?.validatorAddress;
    const {
        isAtRisk,
        isPendingValidators,
        errorValidators,
        validatorSummary,
        apy,
        isApyApproxZero,
        newValidator,
        commission,
    } = useValidatorInfo({
        validatorAddress,
    });

    const iotaEarned = BigInt(stakedDetails?.estimatedReward || 0n);
    const [iotaEarnedFormatted, iotaEarnedSymbol] = useFormatCoin(iotaEarned, IOTA_TYPE_ARG);
    const [totalStakeFormatted, totalStakeSymbol] = useFormatCoin(totalStake, IOTA_TYPE_ARG);

    const validatorName = validatorSummary?.name || '--';

    const subtitle = showActiveStatus ? (
        <div className="flex items-center gap-1">
            {formatAddress(validatorAddress)}
            {newValidator && <Badge label="New" type={BadgeType.PrimarySoft} />}
            {isAtRisk && <Badge label="At Risk" type={BadgeType.PrimarySolid} />}
        </div>
    ) : (
        formatAddress(validatorAddress)
    );

    if (isPendingValidators) {
        return (
            <div className="flex h-full w-full items-center justify-center p-2">
                <LoadingIndicator />
            </div>
        );
    }

    if (errorValidators) {
        toast.error('An error occurred fetching validator information');
    }

    return (
        <DialogLayout>
            <Header title="Validator" onClose={handleClose} onBack={handleClose} titleCentered />
            <DialogLayoutBody>
                <div className="flex w-full flex-col gap-md">
                    <Card type={CardType.Filled}>
                        <CardImage>
                            <ImageIcon
                                src={validatorSummary?.imageUrl ?? null}
                                label={validatorName}
                                fallback={validatorName}
                                size={ImageIconSize.Large}
                            />
                        </CardImage>
                        <CardBody title={validatorName} subtitle={subtitle} isTextTruncated />
                    </Card>
                    <Panel hasBorder>
                        <div className="flex flex-col gap-y-sm p-md">
                            <KeyValueInfo
                                keyText="Your Stake"
                                value={totalStakeFormatted}
                                supportingLabel={totalStakeSymbol}
                                fullwidth
                            />
                            <KeyValueInfo
                                keyText="Earned"
                                value={iotaEarnedFormatted}
                                supportingLabel={iotaEarnedSymbol}
                                fullwidth
                            />
                            <Divider />
                            <KeyValueInfo
                                keyText="APY"
                                value={formatPercentageDisplay(apy, '--', isApyApproxZero)}
                                fullwidth
                            />
                            <KeyValueInfo
                                keyText="Commission"
                                value={`${commission ? commission.toString() : '--'}%`}
                                fullwidth
                            />
                        </div>
                    </Panel>
                </div>
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <div className="flex w-full gap-sm">
                    <Button
                        type={ButtonType.Secondary}
                        onClick={handleUnstake}
                        text="Unstake"
                        fullWidth
                    />
                    <Button
                        type={ButtonType.Primary}
                        text="Stake"
                        onClick={handleStake}
                        fullWidth
                    />
                </div>
            </DialogLayoutFooter>
        </DialogLayout>
    );
}
