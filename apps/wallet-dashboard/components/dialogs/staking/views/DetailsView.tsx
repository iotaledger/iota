// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ExtendedDelegatedStake,
    ImageIcon,
    ImageIconSize,
    useFormatCoin,
    formatPercentageDisplay,
    useValidatorInfo,
    toast,
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
    TooltipPosition,
    InfoBox,
    InfoBoxType,
    InfoBoxStyle,
} from '@iota/apps-ui-kit';
import { formatAddress } from '@iota/iota-sdk/utils';
import { DialogLayout, DialogLayoutFooter, DialogLayoutBody } from '../../layout';
import { useIsValidatorCommitteeMember } from '@/hooks';
import { Warning } from '@iota/apps-ui-icons';

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
    const { isCommitteeMember } = useIsValidatorCommitteeMember();

    const iotaEarned = BigInt(stakedDetails?.estimatedReward || 0n);
    const [iotaEarnedFormatted, iotaEarnedSymbol] = useFormatCoin({ balance: iotaEarned });
    const [totalStakeFormatted, totalStakeSymbol] = useFormatCoin({ balance: totalStake });

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

    const isValidatorCommitteeMember = isCommitteeMember(validatorAddress);

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
                    {!isValidatorCommitteeMember && (
                        <InfoBox
                            type={InfoBoxType.Warning}
                            title="Earn with validators in the committee"
                            supportingText="You are delegating to a validator that is not part of the committee. Stake to a member of the current committee to start earning rewards again."
                            icon={<Warning />}
                            style={InfoBoxStyle.Elevated}
                        />
                    )}
                    <Panel hasBorder>
                        <div className="flex flex-col gap-y-sm p-md">
                            <KeyValueInfo
                                keyText="Member of Committee"
                                tooltipPosition={TooltipPosition.Bottom}
                                tooltipText="If the validator is part of the current committee."
                                value={isValidatorCommitteeMember ? 'Yes' : 'No'}
                                fullwidth
                            />
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
