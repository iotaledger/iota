// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { ExtendedDelegatedStake } from '@iota/core';
import { StakeDialogView } from '../enums/view.enums';
import { ampli, StakeSource } from '@/lib/utils/analytics';
import { HOMEPAGE_ROUTE, STAKING_ROUTE, VESTING_ROUTE } from '@/lib/constants/routes.constants';

export function useStakeDialog() {
    const [stakeDialogView, setStakeDialogView] = useState<StakeDialogView | undefined>();
    const [selectedStake, setSelectedStake] = useState<ExtendedDelegatedStake | null>(null);
    const [selectedValidator, setSelectedValidator] = useState<string>('');

    const isDialogStakeOpen = stakeDialogView !== undefined;

    function defineSourceFlow(): StakeSource | undefined {
        const path = window.location.pathname;
        switch (path) {
            case HOMEPAGE_ROUTE.path:
                return StakeSource.HomeDashboard;
            case STAKING_ROUTE.path:
                return StakeSource.StakingDashboard;
            case VESTING_ROUTE.path:
                return StakeSource.VestingDashboard;
            default:
                return;
        }
    }

    function handleCloseStakeDialog() {
        setSelectedValidator('');
        setSelectedStake(null);
        setStakeDialogView(undefined);
    }

    function handleNewStake() {
        setSelectedStake(null);
        setStakeDialogView(StakeDialogView.SelectValidator);
        ampli.clickedStakeIota({
            isCurrentlyStaking: true,
            sourceFlow: defineSourceFlow(),
        });
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
