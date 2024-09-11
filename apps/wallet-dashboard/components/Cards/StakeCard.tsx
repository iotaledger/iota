// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Box, Button } from '@/components/index';
import { ExtendedDelegatedStake } from '@iota/core';

interface StakeCardProps {
    extendedStake: ExtendedDelegatedStake;
    onDetailsClick: (extendedStake: ExtendedDelegatedStake) => void;
}

function StakeCard({ extendedStake, onDetailsClick }: StakeCardProps): JSX.Element {
    return (
        <Box>
            <div>Validator: {extendedStake.validatorAddress}</div>
            <div>Stake: {extendedStake.principal}</div>
            {extendedStake.status === 'Active' && (
                <p>Estimated reward: {extendedStake.estimatedReward}</p>
            )}
            <div>Status: {extendedStake.status}</div>
            <Button onClick={() => onDetailsClick(extendedStake)}>Details</Button>
        </Box>
    );
}

export default StakeCard;
