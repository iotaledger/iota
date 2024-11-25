// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components';
import { useUnstakeTransaction } from '@/hooks';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { ExtendedDelegatedStake } from '@iota/core';

interface UnstakePopupProps {
    extendedStake: ExtendedDelegatedStake;
    closePopup: () => void;
}

function UnstakePopup({ extendedStake, closePopup }: UnstakePopupProps): JSX.Element {
    const account = useCurrentAccount();
    const { data: unstakeData } = useUnstakeTransaction(
        extendedStake.stakedIotaId,
        account?.address || '',
    );
    const { mutateAsync: signAndExecuteTransaction, isPending } = useSignAndExecuteTransaction();

    async function handleUnstake(): Promise<void> {
        if (!unstakeData) return;
        await signAndExecuteTransaction({
            transaction: unstakeData.transaction,
        });
        closePopup();
    }

    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            <p>Stake ID: {extendedStake.stakedIotaId}</p>
            <p>Validator: {extendedStake.validatorAddress}</p>
            <p>Stake: {extendedStake.principal}</p>
            {extendedStake.status === 'Active' && (
                <p>Estimated reward: {extendedStake.estimatedReward}</p>
            )}
            <p>Gas Fees: {unstakeData?.gasBudget?.toString() || '--'}</p>
            {isPending ? (
                <Button disabled>Loading...</Button>
            ) : (
                <Button onClick={handleUnstake}>Confirm Unstake</Button>
            )}
        </div>
    );
}

export default UnstakePopup;
