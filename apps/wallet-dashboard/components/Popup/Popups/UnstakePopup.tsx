// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components';
import { useUnstakeTransaction } from '@/hooks';
import { useCurrentAccount } from '@iota/dapp-kit';
import { DelegatedStakeWithValidator } from '@iota/core';

interface UnstakePopupProps {
    stake: DelegatedStakeWithValidator;
    closePopup: () => void;
}

function UnstakePopup({ stake, closePopup }: UnstakePopupProps): JSX.Element {
    const account = useCurrentAccount();
    const { unstake, gasBudget, isPending } = useUnstakeTransaction(
        stake.stakedIotaId,
        account?.address || '',
    );

    const handleUnstake = (): void => {
        unstake(closePopup);
    };

    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            <p>Stake ID: {stake.stakedIotaId}</p>
            <p>Validator: {stake.validatorAddress}</p>
            <p>Stake: {stake.principal}</p>
            {stake.status === 'Active' && <p>Estimated reward: {stake.estimatedReward}</p>}
            <p>Gas Fees: {gasBudget}</p>
            {isPending ? (
                <Button disabled>Loading...</Button>
            ) : (
                <Button onClick={handleUnstake}>Confirm Unstake</Button>
            )}
        </div>
    );
}

export default UnstakePopup;
