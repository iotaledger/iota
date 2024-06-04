// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Box, Button } from '@/components/index';
import { Stake } from '@/lib/types';

interface StakeCardProps {
    stake: Stake;
    onDetailsClick: (stake: Stake) => void;
}

function StakeCard({ stake, onDetailsClick }: StakeCardProps): JSX.Element {
    return (
        <Box key={stake.id}>
            <div>Validator: {stake.validator}</div>
            <div>Stake: {stake.stake}</div>
            {stake.status === 'Active' && <p>Estimated reward: {stake.estimatedReward}</p>}
            <div>Status: {stake.status}</div>
            <Button onClick={() => onDetailsClick(stake)}>Details</Button>
        </Box>
    );
}

export default StakeCard;
