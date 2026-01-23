// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import { useNavigate } from 'react-router-dom';
import { useFullscreenGuard, useInitializedGuard, useCreateAccountsMutation } from '_hooks';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import { IotaLogoWeb } from '@iota/apps-ui-icons';
import GetStartedImage from '_assets/images/onboarding/get-started.png';
import GetStartedImageDark from '_assets/images/onboarding/get-started-darkmode.png';
import { useTheme, Theme, ToS_LINK } from '@iota/core';

export function WelcomePage() {
    const createAccountsMutation = useCreateAccountsMutation();
    const { theme } = useTheme();
    const isFullscreenGuardLoading = useFullscreenGuard(true);
    const isInitializedLoading = useInitializedGuard(
        false,
        !(createAccountsMutation.isPending || createAccountsMutation.isSuccess),
    );
    const navigate = useNavigate();
    const CURRENT_YEAR = new Date().getFullYear();

    return (
        <Loading loading={isInitializedLoading || isFullscreenGuardLoading}>
            <div className="flex h-full w-full flex-col items-center justify-between bg-iota-neutral-100 px-md py-lg shadow-wallet-content dark:bg-iota-neutral-6">
                <IotaLogoWeb
                    width={130}
                    height={32}
                    className="text-iota-neutral-10 dark:text-iota-neutral-92"
                />
                <img
                    src={theme === Theme.Dark ? GetStartedImageDark : GetStartedImage}
                    alt="Get Started"
                    height={246}
                    width="auto"
                    className="aspect-[4/3] h-[246px] w-auto object-cover"
                />
                <div className="flex flex-col items-center gap-8 text-center">
                    <div className="flex flex-col gap-y-md">
                        <h1 className="font-alliance-no2 text-[28px] font-medium leading-[120%] text-iota-neutral-10 dark:text-iota-neutral-92">
                            Your Gateway to the IOTA Ecosystem
                        </h1>
                        <Button
                            type={ButtonType.Primary}
                            text="Get Started"
                            fullWidth
                            onClick={() => {
                                navigate('/accounts/add-account?sourceFlow=Onboarding');
                            }}
                            disabled={
                                createAccountsMutation.isPending || createAccountsMutation.isSuccess
                            }
                        />
                        <div className="flex flex-col gap-y-lg">
                            <div className="px-sm text-center text-label-lg text-iota-neutral-40 dark:text-iota-neutral-60">
                                By continuing, I agree to IOTA's{' '}
                                <a
                                    href={ToS_LINK}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="text-label-lg text-iota-primary-30 dark:text-iota-primary-80"
                                >
                                    Terms of Use
                                </a>{' '}
                                and use of Cookies and Analytics.
                            </div>
                            <div className="text-label-md text-iota-neutral-60 dark:text-iota-neutral-40">
                                &copy; IOTA Foundation {CURRENT_YEAR}
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </Loading>
    );
}
