// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import { useNavigate } from 'react-router-dom';
import { useFullscreenGuard, useInitializedGuard, useCreateAccountsMutation } from '_hooks';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import { IotaLogoWeb } from '@iota/ui-icons';

export function WelcomePage() {
    const createAccountsMutation = useCreateAccountsMutation();
    const isFullscreenGuardLoading = useFullscreenGuard(true);
    const isInitializedLoading = useInitializedGuard(
        false,
        !(createAccountsMutation.isPending || createAccountsMutation.isSuccess),
    );
    const navigate = useNavigate();
    const CURRENT_YEAR = new Date().getFullYear();

    return (
        <Loading loading={isInitializedLoading || isFullscreenGuardLoading}>
            <div className="flex h-full w-full flex-col items-center justify-between bg-neutral-100 px-md py-2xl shadow-wallet-content dark:bg-neutral-6">
                <IotaLogoWeb
                    width={130}
                    height={32}
                    className="text-neutral-10 dark:text-neutral-92"
                />
                <div className="flex flex-col items-center gap-8 text-center">
                    <div className="flex flex-col items-center gap-4">
                        <span className="text-headline-sm text-neutral-40">Welcome to</span>
                        <h1 className="text-display-lg text-neutral-10 dark:text-neutral-92">
                            IOTA Wallet
                        </h1>
                        <span className="text-title-lg text-neutral-40">
                            Your Gateway to the IOTA Ecosystem
                        </span>
                    </div>
                    <Button
                        type={ButtonType.Primary}
                        text="Add Profile"
                        onClick={() => {
                            navigate('/accounts/add-account?sourceFlow=Onboarding');
                        }}
                        disabled={
                            createAccountsMutation.isPending || createAccountsMutation.isSuccess
                        }
                    />
                </div>
                <div className="text-body-lg text-neutral-60">
                    &copy; IOTA Foundation {CURRENT_YEAR}
                </div>
            </div>
        </Loading>
    );
}
