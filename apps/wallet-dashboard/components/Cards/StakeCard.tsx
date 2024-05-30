// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Box, Button } from '@/components/index';
import { Stake } from '@/lib/interfaces';

interface StakeCardProps {
    stake: Stake;
    onDetailsClick: (stake: {
        id: string;
        validator: string;
        stake: string;
        rewards: string;
        status: string;
    }) => void;
}

function StakeCard({ stake, onDetailsClick }: StakeCardProps): JSX.Element {
    return (
        <Box key={stake.id}>
            <div>Validator: {stake.validator}</div>
            <div>Stake: {stake.stake}</div>
            <div>Rewards: {stake.rewards}</div>
            <div>Status: {stake.status}</div>
            <Button onClick={() => onDetailsClick(stake)}>Details</Button>
        </Box>
    );
}

export default StakeCard;
