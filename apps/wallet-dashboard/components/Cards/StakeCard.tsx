// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Box, Button } from '@/components/index';
import { ExtendedDelegatedStake } from '@iota/core';

interface StakeCardProps {
    stake: ExtendedDelegatedStake;
    onDetailsClick: (stake: ExtendedDelegatedStake) => void;
}

function StakeCard({ stake, onDetailsClick }: StakeCardProps): JSX.Element {
    return (
        <Box>
            <div>Validator: {stake.validatorAddress}</div>
            <div>Stake: {stake.principal}</div>
            {stake.status === 'Active' && <p>Estimated reward: {stake.estimatedReward}</p>}
            <div>Status: {stake.status}</div>
            <Button onClick={() => onDetailsClick(stake)}>Details</Button>
        </Box>
    );
}

export default StakeCard;
