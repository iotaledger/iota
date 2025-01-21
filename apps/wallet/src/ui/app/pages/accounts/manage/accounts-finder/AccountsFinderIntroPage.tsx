// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';
import { Overlay } from '_components';
import { useActiveAccount } from '_hooks';
import { getKey } from '_helpers';
import { Theme, useTheme } from '@iota/core';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import BalanceFinderIntroImage from '_assets/images/balance_finder_intro.svg';
import BalanceFinderIntroDarkImage from '_assets/images/balance_finder_intro_darkmode.svg';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import { AllowedAccountSourceTypes } from '_src/ui/app/accounts-finder';

export function AccountsFinderIntroPage() {
    const { theme } = useTheme();
    const navigate = useNavigate();
    const activeAccount = useActiveAccount();

    const isLedgerAccount = activeAccount && isLedgerAccountSerializedUI(activeAccount);
    const accountSourceId = activeAccount && getKey(activeAccount);

    const ledgerPath = `/accounts/manage/accounts-finder/${AllowedAccountSourceTypes.LedgerDerived}`;
    const accountPath = isLedgerAccount
        ? ledgerPath
        : `/accounts/manage/accounts-finder/${accountSourceId}`;

    return (
        <Overlay showModal>
            <div className="flex h-full flex-col items-center justify-between">
                <div>
                    {theme === Theme.Dark ? (
                        <BalanceFinderIntroDarkImage />
                    ) : (
                        <BalanceFinderIntroImage />
                    )}
                </div>
                <div className="flex h-full flex-col items-center justify-between">
                    <div className="flex flex-col gap-y-sm p-md text-center">
                        <span className="text-label-lg text-neutral-40 dark:text-neutral-60">
                            Wallet Setup
                        </span>
                        <span className="text-headline-md text-neutral-10 dark:text-neutral-92">
                            Balance Finder
                        </span>
                        <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                            Easily find and import all your accounts with balances, in one place.
                        </span>
                    </div>
                    <div className="flex w-full flex-row gap-x-xs">
                        <Button
                            type={ButtonType.Secondary}
                            text="Skip"
                            onClick={() => navigate('/')}
                            fullWidth
                        />
                        <Button
                            type={ButtonType.Primary}
                            text="Start"
                            fullWidth
                            onClick={() => navigate(accountPath)}
                        />
                    </div>
                </div>
            </div>
        </Overlay>
    );
}
