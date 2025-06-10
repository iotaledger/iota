// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ArrowTopRight } from '@iota/apps-ui-icons';
import { Button, Dialog, DialogContent, DialogBody, Header, Panel } from '@iota/apps-ui-kit';
import { DISCORD_SUPPORT_LINK, Theme, useTheme } from '@iota/core';
import { Link, useNavigate } from 'react-router-dom';
import MissingAssetsDarkmode from '_assets/images/missing_assets_darkmode.png';
import MissingAssets from '_assets/images/missing_assets.png';

interface MissingAssetsDialogProps {
    open: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function MissingAssetsDialog({ open, setOpen }: MissingAssetsDialogProps) {
    const { theme } = useTheme();
    const navigate = useNavigate();

    const imgSrc = theme === Theme.Dark ? MissingAssetsDarkmode : MissingAssets;

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="More Info" onClose={() => setOpen(false)} titleCentered />
                <DialogBody>
                    <div className="flex flex-col gap-sm text-center">
                        <Panel bgColor="bg-secondary-90 dark:bg-secondary-10">
                            <div className="flex h-[100px] w-full justify-between ">
                                <div className="flex w-full flex-col justify-between p-md">
                                    <div className="flex flex-col items-start gap-xxs text-start">
                                        <span className="text-title-sm text-neutral-10 dark:text-neutral-92">
                                            Any questions?
                                        </span>
                                        <span className="text-body-sm text-neutral-40 dark:text-neutral-60">
                                            We're here to help.
                                        </span>
                                    </div>
                                    <Link
                                        to={DISCORD_SUPPORT_LINK}
                                        target="_blank"
                                        rel="noreferrer"
                                        className="flex items-center gap-x-xxs text-primary-30 underline dark:text-primary-80"
                                    >
                                        <span className="shrink-0">Discord</span>
                                        <ArrowTopRight />
                                    </Link>
                                </div>
                                <img src={imgSrc} alt="Need help?" className="h-full" />
                            </div>
                        </Panel>
                        <Panel bgColor="bg-warning-90 dark:bg-warning-20">
                            <div className="flex flex-col items-start justify-start gap-xs p-md text-start">
                                <span className="text-title-sm text-neutral-10 dark:text-neutral-92">
                                    Missing assets?
                                </span>
                                <span className="text-body-sm text-neutral-40 dark:text-neutral-60">
                                    Some assets may not show up in the balance but are in you
                                    possession. Other’s require user actions to show up.
                                </span>
                                <div className="flex w-full flex-wrap justify-start gap-xs text-body-sm text-primary-30 dark:text-primary-80">
                                    <Link
                                        to="https://docs.iota.org/about-iota/iota-wallet/getting-started#use-the-balance-finder"
                                        target="_blank"
                                        rel="noreferrer"
                                        className="flex items-center gap-x-xxs underline"
                                    >
                                        <span className="shrink-0">Run Balance finder</span>
                                        <ArrowTopRight />
                                    </Link>
                                    <Link
                                        to={DISCORD_SUPPORT_LINK}
                                        target="_blank"
                                        rel="noreferrer"
                                        className="flex items-center gap-x-xxs underline"
                                    >
                                        <span className="shrink-0">Support</span>
                                        <ArrowTopRight />
                                    </Link>
                                    <Link
                                        to="https://docs.iota.org/about-iota/iota-wallet/FAQ"
                                        target="_blank"
                                        rel="noreferrer"
                                        className="flex items-center gap-x-xxs underline"
                                    >
                                        <span className="shrink-0">FAQs</span>
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
