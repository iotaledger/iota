// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { ExtendedDelegatedStake } from '@iota/core';

import { StakeDialogView } from '@/components/Dialogs/Staking/StakeDialog';

export function useStakeDialog() {
    const [stakeDialogView, setStakeDialogView] = useState<StakeDialogView | undefined>();
    const [selectedStake, setSelectedStake] = useState<ExtendedDelegatedStake | null>(null);
    const [selectedValidator, setSelectedValidator] = useState<string>('');

    const isDialogStakeOpen = stakeDialogView !== undefined;

    function handleCloseStakeDialog() {
        setSelectedValidator('');
        setSelectedStake(null);
        setStakeDialogView(undefined);
    }

    function handleNewStake() {
        setSelectedStake(null);
        setStakeDialogView(StakeDialogView.SelectValidator);
    }

    return {
        isDialogStakeOpen,
        stakeDialogView,
        setStakeDialogView,
        selectedStake,
        setSelectedStake,
        selectedValidator,
        setSelectedValidator,
        handleCloseStakeDialog,
        handleNewStake,
    };
}
