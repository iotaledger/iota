// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { useGetSupplyIncreaseVestingObjects } from '@/hooks';
import {
    ButtonType,
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardType,
    LabelText,
    LabelTextSize,
    Panel,
    Title,
} from '@iota/apps-ui-kit';
import { StakeDialog, useStakeDialog } from './dialogs';
import { TIMELOCK_IOTA_TYPE, useCountdownByTimestamp, useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import SvgClock from '@iota/ui-icons/src/Clock';
import { useQueryClient } from '@tanstack/react-query';

export function SupplyIncreaseVestingOverview() {
    const account = useCurrentAccount();
    const address = account?.address || '';
    const iotaClient = useIotaClient();
    const queryClient = useQueryClient();
    const {
        nextPayout,
        supplyIncreaseVestingSchedule,
        supplyIncreaseVestingMapped,
        supplyIncreaseVestingStakedMapped,
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
        { showSeconds: false, showMinutes: false },
    );
    const [formattedNextPayout, nextPayoutSymbol, nextPayoutResult] = useFormatCoin(
        nextPayout?.amount,
        IOTA_TYPE_ARG,
    );

    const [formattedAvailableStaking, availableStakingSymbol] = useFormatCoin(
        supplyIncreaseVestingSchedule.availableStaking,
        IOTA_TYPE_ARG,
    );

    const showSupplyIncreaseVestingOverview =
        supplyIncreaseVestingMapped.length > 0 || supplyIncreaseVestingStakedMapped.length > 0;

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

    return showSupplyIncreaseVestingOverview ? (
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
                                                : `${formattedNextPayout}   `
                                        }
                                        supportingLabel={nextPayoutSymbol}
                                    />
                                }
                            />
                            <CardAction
                                type={CardActionType.Button}
                                buttonType={ButtonType.Ghost}
                                title={formattedLastPayoutExpirationTime}
                                icon={<SvgClock />}
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
