// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import { useNavigate } from 'react-router-dom';
import { useFullscreenGuard, useInitializedGuard, useCreateAccountsMutation } from '_hooks';
import { Button, ButtonSize, ButtonType } from '@iota/apps-ui-kit';
import { IotaLogoWeb } from '@iota/apps-ui-icons';
import OnboardImage from '_images/onboard-s1.png';
import { setCookieConsentAccepted } from '_shared/utils';

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
            <div className="flex h-full w-full flex-col items-center justify-between bg-iota-neutral-100 px-md py-lg shadow-wallet-content dark:bg-iota-neutral-6">
                <IotaLogoWeb
                    width={130}
                    height={32}
                    className="text-iota-neutral-10 dark:text-iota-neutral-92"
                />
                <img src={OnboardImage} alt="IOTA Wallet Onboarding" className="w-full" />
                <div className="flex flex-col items-center text-center">
                    <div className="mb-md flex flex-col items-center">
                        <span className="text-headline-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                            Your Gateway to
                        </span>
                        <span className="text-headline-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                            the IOTA Ecosystem
                        </span>
                    </div>
                    <Button
                        type={ButtonType.Primary}
                        size={ButtonSize.Medium}
                        fullWidth
                        text="Get Started"
                        onClick={async () => {
                            await setCookieConsentAccepted();
                            navigate('/accounts/add-account?sourceFlow=Onboarding');
                        }}
                        disabled={
                            createAccountsMutation.isPending || createAccountsMutation.isSuccess
                        }
                    />
                    <div className="mt-md text-label-lg text-iota-neutral-40">
                        By continuing, I agree to IOTA’s{' '}
                        <a href="#" className="text-iota-primary-30">
                            Terms of Use
                        </a>{' '}
                        and{' '}
                        <a href="#" className="text-iota-primary-30">
                            Privacy Policy
                        </a>
                        . Use of{' '}
                        <a href="#" className="text-iota-primary-30">
                            Cookies and Analytics
                        </a>
                        .
                    </div>
                    <div className="mt-lg text-label-md text-iota-neutral-60 dark:text-iota-neutral-40">
                        &copy; IOTA Foundation {CURRENT_YEAR}
                    </div>
                </div>
            </div>
        </Loading>
    );
}
