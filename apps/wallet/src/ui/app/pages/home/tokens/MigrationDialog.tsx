// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ArrowTopRight } from '@iota/apps-ui-icons';
import { Button, Dialog, DialogContent, DialogBody, Header, Panel } from '@iota/apps-ui-kit';
import { Banner, BannerSize, Theme, useTheme } from '@iota/core';
import { WALLET_DASHBOARD_URL } from '_src/shared/constants';
import { Link } from 'react-router-dom';
import { ampli } from '_src/shared/analytics/ampli';

interface MigrationDialogProps {
    open: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function MigrationDialog({ open, setOpen }: MigrationDialogProps) {
    const { theme } = useTheme();

    const videoSrc =
        theme === Theme.Dark
            ? 'https://files.iota.org/media/tooling/wallet-dashboard-migration-dark.mp4'
            : 'https://files.iota.org/media/tooling/wallet-dashboard-migration-light.mp4';

    function navigateToDashboard() {
        ampli.openedApplication({
            applicationName: 'IOTA Wallet Dashboard',
        });
        window.open(WALLET_DASHBOARD_URL, '_blank', 'noopener noreferrer');
    }
    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Migration" onClose={() => setOpen(false)} titleCentered />
                <DialogBody>
                    <div className="flex flex-col gap-sm text-center">
                        <Banner
                            videoSrc={videoSrc}
                            title="Migration Needed"
                            subtitle="Migrate your Stardust assets using the IOTA Wallet Dashboard"
                            size={BannerSize.Small}
                        >
                            <div className="flex w-full flex-wrap justify-start gap-xs text-body-sm text-iota-primary-30 dark:text-iota-primary-80">
                                <Link
                                    to="https://docs.iota.org/users/iota-wallet-dashboard/how-to/migration"
                                    target="_blank"
                                    rel="noreferrer"
                                    className="flex items-center gap-x-xxs underline"
                                >
                                    <span className="shrink-0">Docs</span>
                                    <ArrowTopRight />
                                </Link>
                            </div>
                        </Banner>
                        <Panel bgColor="bg-iota-secondary-90 dark:bg-iota-secondary-10">
                            <div className="flex flex-col items-start justify-start gap-xs p-md text-start">
                                <span className="text-title-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                                    Step-by-step
                                </span>
                                <ol className="list-decimal space-y-xs pl-md text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                    <li>Connect your wallet to the IOTA Wallet Dashboard</li>
                                    <li>Go to the Migration tab in the sidebar</li>
                                    <li>
                                        Click on the Migrate All button to start the asset migration
                                    </li>
                                    <li>
                                        Confirm the transaction in your wallet to complete the
                                        migration
                                    </li>
                                </ol>
                            </div>
                        </Panel>
                    </div>
                </DialogBody>
                <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs pt-sm--rs">
                    <Button onClick={navigateToDashboard} fullWidth text="Go to Dashboard" />
                </div>
            </DialogContent>
        </Dialog>
    );
}
