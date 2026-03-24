// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ArrowTopRight } from '@iota/apps-ui-icons';
import { Button, Dialog, DialogContent, DialogBody, Header } from '@iota/apps-ui-kit';
import { Banner, BannerSize, Theme, useTheme } from '@iota/core';
import { WALLET_DASHBOARD_URL } from '_src/shared/constants';
import { ampli } from '_src/shared/analytics/ampli';

interface SupplyIncreaseVestingStakingDialogProps {
    open: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function SupplyIncreaseVestingStakingDialog({
    open,
    setOpen,
}: SupplyIncreaseVestingStakingDialogProps) {
    const { theme } = useTheme();

    const videoSrc =
        theme === Theme.Dark
            ? 'https://files.iota.org/media/tooling/wallet-dashboard-staking-dark.mp4'
            : 'https://files.iota.org/media/tooling/wallet-dashboard-staking-light.mp4';

    function navigateToDashboard() {
        ampli.openedApplication({
            applicationName: 'IOTA Wallet Dashboard',
        });
        window.open(WALLET_DASHBOARD_URL, '_blank', 'noopener noreferrer');
    }

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Action Required" onClose={() => setOpen(false)} titleCentered />
                <DialogBody>
                    <div className="flex flex-col gap-sm text-center">
                        <Banner
                            videoSrc={videoSrc}
                            title="Vesting has ended"
                            subtitle="Claim your rewards and migrate your stake now to make your tokens fully compatible with your favorite wallets and ready for use."
                            size={BannerSize.Small}
                        ></Banner>
                    </div>
                </DialogBody>
                <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs pt-sm--rs">
                    <Button
                        onClick={navigateToDashboard}
                        fullWidth
                        text="Go to Dashboard"
                        icon={<ArrowTopRight />}
                    />
                </div>
            </DialogContent>
        </Dialog>
    );
}
