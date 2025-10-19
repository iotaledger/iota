// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ArrowTopRight } from '@iota/apps-ui-icons';
import { Button, Dialog, DialogContent, DialogBody, Header, Panel } from '@iota/apps-ui-kit';
import { DISCORD_SUPPORT_LINK, Theme, useTheme } from '@iota/core';
import { Link, useNavigate } from 'react-router-dom';
import MissingFundsDarkmode from '_assets/images/missing_funds_darkmode.png';
import MissingFunds from '_assets/images/missing_funds.png';

interface MissingFundsDialogProps {
    open: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function MissingFundsDialog({ open, setOpen }: MissingFundsDialogProps) {
    const { theme } = useTheme();
    const navigate = useNavigate();

    const imgSrc = theme === Theme.Dark ? MissingFundsDarkmode : MissingFunds;

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="More Info" onClose={() => setOpen(false)} titleCentered />
                <DialogBody>
                    <div className="flex flex-col gap-sm text-center">
                        <Panel bgColor="bg-iota-secondary-90 dark:bg-iota-secondary-10">
                            <div className="flex h-[100px] w-full justify-between ">
                                <div className="flex w-full flex-col justify-between p-md">
                                    <div className="flex flex-col items-start gap-xxs text-start">
                                        <span className="text-title-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                                            Any questions?
                                        </span>
                                        <span className="text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                            We're here to help.
                                        </span>
                                    </div>
                                    <Link
                                        to={DISCORD_SUPPORT_LINK}
                                        target="_blank"
                                        rel="noreferrer"
                                        className="flex items-center gap-x-xxs text-iota-primary-30 underline dark:text-iota-primary-80"
                                    >
                                        <span className="shrink-0">Discord</span>
                                        <ArrowTopRight />
                                    </Link>
                                </div>
                                <img src={imgSrc} alt="Need help?" className="h-full" />
                            </div>
                        </Panel>
                        <Panel bgColor="bg-iota-warning-90 dark:bg-iota-warning-20">
                            <div className="flex flex-col items-start justify-start gap-xs p-md text-start">
                                <span className="text-title-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                                    Missing funds?
                                </span>
                                <span className="text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                    Some addresses are tagged to indicate that their balances may be
                                    inaccurate, often due to conditions like vesting or pending
                                    migration that require user action. These funds are still in
                                    your possession, even if not reflected in the balance.
                                </span>
                                <div className="flex w-full flex-wrap justify-start gap-xs text-body-sm text-iota-primary-30 dark:text-iota-primary-80">
                                    <Link
                                        to="https://docs.iota.org/users/iota-wallet-dashboard/how-to/migration"
                                        target="_blank"
                                        rel="noreferrer"
                                        className="flex items-center gap-x-xxs underline"
                                    >
                                        <span className="shrink-0">How to migrate</span>
                                        <ArrowTopRight />
                                    </Link>
                                    <Link
                                        to="https://docs.iota.org/users/iota-wallet-dashboard/how-to/vesting"
                                        target="_blank"
                                        rel="noreferrer"
                                        className="flex items-center gap-x-xxs underline"
                                    >
                                        <span className="shrink-0">Manage vesting</span>
                                        <ArrowTopRight />
                                    </Link>
                                </div>
                            </div>
                        </Panel>
                    </div>
                </DialogBody>
                <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs pt-sm--rs">
                    <Button
                        onClick={() => navigate('/tokens')}
                        fullWidth
                        text="Continue to Wallet"
                    />
                </div>
            </DialogContent>
        </Dialog>
    );
}
