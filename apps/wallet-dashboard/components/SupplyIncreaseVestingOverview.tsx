// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ReactNode } from 'react';
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { useGetSupplyIncreaseVestingObjects } from '@/hooks';
import {
    ButtonType,
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LabelText,
    LabelTextSize,
    Panel,
    Title,
} from '@iota/apps-ui-kit';
import { StakeDialog, useStakeDialog } from './dialogs';
import { TIMELOCK_IOTA_TYPE, useCountdownByTimestamp, useFormatCoin } from '@iota/core';
import { Clock, Vesting } from '@iota/apps-ui-icons';
import { useQueryClient } from '@tanstack/react-query';
import { SupplyIncreaseUserType } from '@/lib/interfaces';

interface SupplyIncreaseVestingOverviewProps {
    customButton?: ReactNode;
}

export function SupplyIncreaseVestingOverview({
    customButton,
}: SupplyIncreaseVestingOverviewProps = {}) {
    const account = useCurrentAccount();
    const address = account?.address || '';
    const iotaClient = useIotaClient();
    const queryClient = useQueryClient();
    const {
        nextPayout,
        supplyIncreaseVestingSchedule,
        isSupplyIncreaseVestingScheduleEmpty,
        supplyIncreaseVestingStakedMapped,
        userType,
    } = useGetSupplyIncreaseVestingObjects(address);

    const {
        stakeDialogView,
        setStakeDialogView,
        selectedStake,
        selectedValidator,
        setSelectedValidator,
        handleCloseStakeDialog,
        handleNewStake,
    } = useStakeDialog();

    const formattedLastPayoutExpirationTime = useCountdownByTimestamp(
        Number(nextPayout?.expirationTimestampMs),
        { showSeconds: false, showMinutes: false, hideZeroUnits: true },
    );
    const [formattedNextPayout, nextPayoutSymbol, nextPayoutResult] = useFormatCoin({
        balance: nextPayout?.amount,
    });

    const [formattedAvailableStaking, availableStakingSymbol] = useFormatCoin({
        balance: supplyIncreaseVestingSchedule.availableStaking,
    });

    function handleOnSuccess(digest: string): void {
        iotaClient
            .waitForTransaction({
                digest,
            })
            .then(() => {
                queryClient.invalidateQueries({
                    queryKey: ['get-timelocked-staked-objects', account?.address],
                });
                queryClient.invalidateQueries({
                    queryKey: [
                        'get-all-owned-objects',
                        account?.address,
                        {
                            StructType: TIMELOCK_IOTA_TYPE,
                        },
                    ],
                });
            });
    }

    // Show simplified UI for Staker users
    if (userType === SupplyIncreaseUserType.Staker) {
        return !isSupplyIncreaseVestingScheduleEmpty ||
            supplyIncreaseVestingStakedMapped.length > 0 ? (
            <div style={{ gridArea: 'vesting' }} className="with-vesting flex grow overflow-hidden">
                <Panel>
                    <div className="flex w-full flex-col items-center justify-between gap-md p-md sm:flex-row">
                        <InfoBox
                            title="Your vesting period has ended"
                            supportingText="Claim your rewards and migrate your stake now to make your tokens fully compatible with your favorite wallets and ready for use."
                            type={InfoBoxType.Warning}
                            style={InfoBoxStyle.Default}
                            icon={<Vesting />}
                        />
                        {customButton && (
                            <div className="flex shrink-0 flex-col items-center justify-center">
                                {customButton}
                            </div>
                        )}
                    </div>
                </Panel>
            </div>
        ) : null;
    }

    // Show full UI for Entity users (investors)
    return !isSupplyIncreaseVestingScheduleEmpty || supplyIncreaseVestingStakedMapped.length > 0 ? (
        <div style={{ gridArea: 'vesting' }} className="with-vesting flex grow overflow-hidden">
            <Panel>
                <Title title="Vesting" />
                <div className="flex h-full w-full items-center gap-md p-md--rs">
                    <div className="w-1/2">
                        <Card type={CardType.Filled}>
                            <CardBody
                                title=""
                                subtitle={
                                    <LabelText
                                        size={LabelTextSize.Large}
                                        label="Next reward"
                                        text={
                                            nextPayoutResult.isPending
                                                ? '-'
                                                : `${formattedNextPayout} `
                                        }
                                        supportingLabel={nextPayoutSymbol}
                                    />
                                }
                            />
                            <CardAction
                                type={CardActionType.Button}
                                buttonType={ButtonType.Ghost}
                                title={formattedLastPayoutExpirationTime}
                                icon={<Clock />}
                            />
                        </Card>
                    </div>
                    <div className="w-1/2">
                        <Card type={CardType.Filled}>
                            <CardBody
                                title=""
                                subtitle={
                                    <LabelText
                                        size={LabelTextSize.Large}
                                        label="Available for staking"
                                        text={formattedAvailableStaking}
                                        supportingLabel={availableStakingSymbol}
                                    />
                                }
                            />
                            <CardAction
                                type={CardActionType.Button}
                                buttonType={ButtonType.Primary}
                                title={'Stake'}
                                onClick={() => handleNewStake()}
                                buttonDisabled={!supplyIncreaseVestingSchedule.availableStaking}
                            />
                        </Card>
                    </div>
                </div>
            </Panel>
            <StakeDialog
                isTimelockedStaking={true}
                stakedDetails={selectedStake}
                onSuccess={handleOnSuccess}
                handleClose={handleCloseStakeDialog}
                view={stakeDialogView}
                setView={setStakeDialogView}
                selectedValidator={selectedValidator}
                setSelectedValidator={setSelectedValidator}
                maxStakableTimelockedAmount={BigInt(supplyIncreaseVestingSchedule.availableStaking)}
            />
        </div>
    ) : null;
}
