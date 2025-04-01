// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, Dialog, DialogContent, DialogBody, Header } from '@iota/apps-ui-kit';
import { Theme, useTheme } from '@iota/core';
import MigrationImage from '_assets/images/migration_dialog.png';
import MigrationDarkImage from '_assets/images/migration_dialog_darkmode.png';
import { WALLET_DASHBOARD_URL } from '_src/shared/constants';

interface MigrationDialogProps {
    open: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function MigrationDialog({ open, setOpen }: MigrationDialogProps) {
    const { theme } = useTheme();

    const imgSrc = theme === Theme.Dark ? MigrationDarkImage : MigrationImage;

    function navigateToDashboard() {
        window.open(WALLET_DASHBOARD_URL, '_blank', 'noopener noreferrer');
    }
    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Migration" onClose={() => setOpen(false)} titleCentered />
                <DialogBody>
                    <div className="flex flex-col gap-lg text-center">
                        <img src={imgSrc} alt="Migration" />
                        <div className="flex flex-col items-center justify-center gap-y-sm pb-md">
                            <span className="text-headline-sm text-neutral-10 dark:text-neutral-92">
                                Fast and Easy Migration
                            </span>
                            <span className="max-w-56 text-body-md text-neutral-40 dark:text-neutral-60">
                                Migrate your tokens to the new network to enjoy the latest features.
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
