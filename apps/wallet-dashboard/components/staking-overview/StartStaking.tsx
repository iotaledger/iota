// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonSize, ButtonType, Panel } from '@iota/apps-ui-kit';
import { Theme, useTheme } from '@/contexts';
import { useState } from 'react';
import { StakeDialog } from '../Dialogs';
import { StakeDialogView } from '../Dialogs/Staking/StakeDialog';

export function StartStaking() {
    const { theme } = useTheme();
    const [isDialogStakeOpen, setIsDialogStakeOpen] = useState(false);

    function handleNewStake() {
        setIsDialogStakeOpen(true);
    }

    const videoSrc =
        theme === Theme.Dark
            ? 'https://files.iota.org/media/tooling/wallet-dashboard-staking-dark.mp4'
            : 'https://files.iota.org/media/tooling/wallet-dashboard-staking-light.mp4';

    return (
        <Panel bgColor="bg-secondary-90 dark:bg-secondary-10">
            <div className="flex h-full w-full justify-between">
                <div className="flex h-full w-full flex-col justify-between p-lg">
                    <div className="flex flex-col gap-xxs">
                        <span className="text-headline-sm text-neutral-10 dark:text-neutral-92">
                            Start Staking
                        </span>
                        <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                            Earn Rewards
                        </span>
                    </div>
                    <div>
                        <Button
                            onClick={handleNewStake}
                            size={ButtonSize.Small}
                            type={ButtonType.Outlined}
                            text="Stake"
                        />
                    </div>
                </div>
                <div className="relative w-full overflow-hidden">
                    <video
                        src={videoSrc}
                        autoPlay
                        loop
                        muted
                        className="absolute -top-16 h-80 w-full"
                    ></video>
                </div>
            </div>
            <StakeDialog
                isOpen={isDialogStakeOpen}
                handleClose={() => setIsDialogStakeOpen(false)}
                view={StakeDialogView.SelectValidator}
            />
        </Panel>
    );
}
