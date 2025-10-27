// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonSize, ButtonType } from '@iota/apps-ui-kit';
import { StakeDialog, useStakeDialog } from '../dialogs';
import { Banner, Theme, useTheme } from '@iota/core';

export function StartStaking() {
    const { theme } = useTheme();
    const {
        isDialogStakeOpen,
        stakeDialogView,
        setStakeDialogView,
        selectedStake,
        selectedValidator,
        setSelectedValidator,
        handleCloseStakeDialog,
        handleNewStake,
    } = useStakeDialog();

    const videoSrc =
        theme === Theme.Dark
            ? 'https://files.iota.org/media/tooling/wallet-dashboard-staking-dark.mp4'
            : 'https://files.iota.org/media/tooling/wallet-dashboard-staking-light.mp4';

    return (
        <>
            <Banner videoSrc={videoSrc} title="Start Staking" subtitle="Earn Rewards">
                <Button
                    onClick={handleNewStake}
                    size={ButtonSize.Small}
                    type={ButtonType.Outlined}
                    text="Stake"
                />
            </Banner>
            {isDialogStakeOpen && stakeDialogView && (
                <StakeDialog
                    stakedDetails={selectedStake}
                    handleClose={handleCloseStakeDialog}
                    view={stakeDialogView}
                    setView={setStakeDialogView}
                    selectedValidator={selectedValidator}
                    setSelectedValidator={setSelectedValidator}
                />
            )}
        </>
    );
}
