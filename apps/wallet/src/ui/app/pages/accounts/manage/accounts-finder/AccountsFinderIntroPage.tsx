// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';
import { Overlay } from '_components';
import { useActiveAccount, useAppSelector } from '_hooks';
import { getSourceId } from '_helpers';
import { Theme, useTheme } from '@iota/core';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import BalanceFinderIntroImage from '_assets/images/balance_finder_intro.png';
import BalanceFinderIntroDarkImage from '_assets/images/balance_finder_intro_darkmode.png';
import { useEffect, useState } from 'react';
import { ExtensionViewType } from '_src/ui/app/redux/slices/app/appType';

export function AccountsFinderIntroPage() {
    const { theme } = useTheme();
    const navigate = useNavigate();
    const activeAccount = useActiveAccount();
    const [skipSeconds, setSkipSeconds] = useState(5);
    const isAppPopup = useAppSelector(
        (state) => state.app.extensionViewType === ExtensionViewType.Popup,
    );

    const skipActionAllowed = skipSeconds > 0;
    const accountSourceId = activeAccount && getSourceId(activeAccount);
    const imgSrc = theme === Theme.Dark ? BalanceFinderIntroDarkImage : BalanceFinderIntroImage;

    const accountPath = `/accounts/manage/accounts-finder/${accountSourceId}`;

    useEffect(() => {
        if (!skipActionAllowed) return;

        const id = setInterval(() => {
            setSkipSeconds((s) => s - 1);
        }, 1_000);

        return () => clearInterval(id);
    }, [skipActionAllowed]);

    function navigateToAccountFinder() {
        if (isAppPopup) {
            const currentBaseUrl = window.location.href.split('#')[0];
            window.open(`${currentBaseUrl}#${accountPath}`, '_blank', 'noopener noreferrer');
        } else {
            navigate(accountPath);
        }
    }

    return (
        <Overlay showModal>
            <div className="flex h-full flex-col items-center justify-between">
                <img src={imgSrc} alt="Balance Finder Intro" />
                <div className="flex h-full flex-col items-center justify-between">
                    <div className="flex flex-col gap-y-sm p-md text-center">
                        <span className="text-label-lg text-iota-neutral-40 dark:text-iota-neutral-60">
                            Wallet Setup
                        </span>
                        <span className="text-headline-md text-iota-neutral-10 dark:text-iota-neutral-92">
                            Balance Finder
                        </span>
                        <div className="flex flex-col gap-y-xs text-start text-body-md">
                            <span className=" text-iota-neutral-40 dark:text-iota-neutral-60">
                                <span className="text-iota-neutral-10 dark:text-iota-neutral-92">
                                    Run multiple searches{' '}
                                </span>
                                to ensure all assets are located. Some funds and addresses may not
                                appear immediately.
                            </span>
                            <span className=" text-iota-neutral-40 dark:text-iota-neutral-60">
                                <span className="text-iota-neutral-10 dark:text-iota-neutral-92">
                                    Missing funds{' '}
                                </span>
                                may be due to unmigrated tokens or locked vested funds.
                            </span>
                        </div>
                    </div>
                    <div className="flex w-full flex-row gap-x-xs">
                        <Button
                            type={ButtonType.Secondary}
                            text={skipActionAllowed ? `Skip (${skipSeconds}s)` : 'Skip'}
                            onClick={() => navigate('/tokens')}
                            fullWidth
                            disabled={skipActionAllowed}
                        />
                        <Button
                            type={ButtonType.Primary}
                            text="Find your assets"
                            fullWidth
                            onClick={navigateToAccountFinder}
                        />
                    </div>
                </div>
            </div>
        </Overlay>
    );
}
