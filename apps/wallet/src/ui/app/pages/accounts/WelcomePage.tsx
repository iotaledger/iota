// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading, useSourceFlow } from '_components';
import { useNavigate } from 'react-router-dom';
import { useFullscreenGuard, useInitializedGuard } from '_hooks';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import { IotaLogoWeb } from '@iota/apps-ui-icons';
import GetStartedImage from '_assets/images/onboarding/get-started.png';
import GetStartedImageDark from '_assets/images/onboarding/get-started-darkmode.png';
import { useTheme, Theme } from '@iota/core';
import { AmpliSourceFlow } from '_src/shared/analytics';

export function WelcomePage() {
    const { theme } = useTheme();
    const isFullscreenGuardLoading = useFullscreenGuard(true);
    const isInitializedLoading = useInitializedGuard(false);
    const navigate = useNavigate();
    const { setSourceFlow } = useSourceFlow();
    const CURRENT_YEAR = new Date().getFullYear();

    return (
        <Loading loading={isInitializedLoading || isFullscreenGuardLoading}>
            <div className="flex h-full w-full flex-col items-center justify-between bg-iota-neutral-100 px-md py-2xl shadow-wallet-content dark:bg-iota-neutral-6">
                <IotaLogoWeb
                    width={130}
                    height={32}
                    className="text-iota-neutral-10 dark:text-iota-neutral-92"
                />
                <div className="flex flex-col items-center gap-8 text-center">
                    <img
                        src={theme === Theme.Dark ? GetStartedImageDark : GetStartedImage}
                        alt="Get Started"
                        height={246}
                        width="auto"
                        className="aspect-[4/3] h-[246px] w-auto object-cover"
                    />
                    <h1 className="font-alliance-no2 text-[28px] font-medium leading-[120%] text-iota-neutral-10 dark:text-iota-neutral-92">
                        Your Gateway to the IOTA Ecosystem
                    </h1>
                    <Button
                        type={ButtonType.Primary}
                        text="Get Started"
                        onClick={() => {
                            setSourceFlow(AmpliSourceFlow.Onboarding);
                            navigate('/accounts/add-account');
                        }}
                    />
                </div>
                <div className="text-label-lg text-iota-neutral-60 dark:text-iota-neutral-40">
                    &copy; IOTA Foundation {CURRENT_YEAR}
                </div>
            </div>
        </Loading>
    );
}
