// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components';
import { useTimelockedUnstakeTransaction } from '@/hooks';
import { useSignAndExecuteTransactionBlock } from '@iota/dapp-kit';
import { DelegatedTimelockedStake, IotaValidatorSummary } from '@iota/iota-sdk/client';

interface UnstakePopupProps {
    accountAddress: string;
    delegatedStake: DelegatedTimelockedStake;
    validatorInfo: IotaValidatorSummary;
    closePopup: () => void;
}

function TimelockedUnstakePopup({
    accountAddress,
    delegatedStake,
    validatorInfo,
    closePopup,
}: UnstakePopupProps): JSX.Element {
    const objectIds = delegatedStake.stakes.map((stake) => stake.timelockedStakedIotaId);
    const { data: timelockedUnstake } = useTimelockedUnstakeTransaction(objectIds, accountAddress);
    const { mutateAsync: signAndExecuteTransactionBlock, isPending } =
        useSignAndExecuteTransactionBlock();

    async function handleTimelockedUnstake(): Promise<void> {
        if (!timelockedUnstake) return;
        await signAndExecuteTransactionBlock({
            transactionBlock: timelockedUnstake.transaction,
        });
        closePopup();
    }

    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            <p>Validator Name: {validatorInfo.name}</p>
            <p>Validator Address: {delegatedStake.validatorAddress}</p>
            <p>Rewards: {validatorInfo.rewardsPool}</p>
            <p>Total Stakes: {delegatedStake.stakes.length}</p>
            {delegatedStake.stakes.map((stake, index) => {
                return (
                    <div key={stake.timelockedStakedIotaId} className="m-4 flex flex-col">
                        <span>
                            Stake {index + 1}: {stake.timelockedStakedIotaId}
                        </span>
                        <span>Expiration time: {stake.expirationTimestampMs}</span>
                        <span>Label: {stake.label}</span>
                        <span>Status: {stake.status}</span>
                    </div>
                );
            })}
            <p>Gas Fees: {timelockedUnstake?.gasBudget?.toString() || '--'}</p>
            {isPending ? (
                <Button disabled>Loading...</Button>
            ) : (
                <Button onClick={handleTimelockedUnstake}>Confirm Unstake</Button>
            )}
        </div>
    );
}

export default TimelockedUnstakePopup;
