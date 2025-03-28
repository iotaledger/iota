// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, Dialog, DialogContent, DialogBody, Header } from '@iota/apps-ui-kit';
import { Theme, useTheme } from '@iota/core';
import SupplyIncreaseVestingStakingImage from '_assets/images/vested_staking_dialog.png';
import SupplyIncreaseVestingStakingDarkImage from '_assets/images/vested_staking_dialog_darkmode.png';
import { WALLET_DASHBOARD_URL } from '_src/shared/constants';

interface SupplyIncreaseVestingStakingDialogProps {
    open: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function SupplyIncreaseVestingStakingDialog({
    open,
    setOpen,
}: SupplyIncreaseVestingStakingDialogProps) {
    const { theme } = useTheme();

    const imgSrc =
        theme === Theme.Dark
            ? SupplyIncreaseVestingStakingDarkImage
            : SupplyIncreaseVestingStakingImage;

    function navigateToDashboard() {
        window.open(WALLET_DASHBOARD_URL, '_blank', 'noopener noreferrer');
    }
    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Vested Staking" onClose={() => setOpen(false)} titleCentered />
                <DialogBody>
                    <div className="flex flex-col gap-lg text-center">
                        <img src={imgSrc} alt="Supply Increase Vesting Staking" />
                        <div className="flex flex-col items-center justify-center gap-y-sm pb-md">
                            <span className="text-headline-sm text-neutral-10 dark:text-neutral-92">
                                Vested Staking Available
                            </span>
                            <span className="max-w-56 text-body-md text-neutral-40 dark:text-neutral-60">
                                Earn rewards by staking your vested tokens
                            </span>
                        </div>
                    </div>
                </DialogBody>
                <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs pt-sm--rs">
                    <Button onClick={navigateToDashboard} fullWidth text="Go to Dashboard" />
                </div>
            </DialogContent>
        </Dialog>
    );
}
