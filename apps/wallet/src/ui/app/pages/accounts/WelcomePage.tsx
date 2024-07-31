// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '_app/shared/ButtonUI';
import { Heading } from '_app/shared/heading';
import { Text } from '_app/shared/text';
import Loading from '_components/loading';
import Logo from '_components/logo';
import { useFullscreenGuard, useInitializedGuard } from '_hooks';
import WelcomeSplash from '_src/ui/assets/images/WelcomeSplash.svg';

import { useCreateAccountsMutation } from '../../hooks/useCreateAccountMutation';

export function WelcomePage() {
    const createAccountsMutation = useCreateAccountsMutation();
    const isFullscreenGuardLoading = useFullscreenGuard(true);
    const isInitializedLoading = useInitializedGuard(
        false,
        !(createAccountsMutation.isPending || createAccountsMutation.isSuccess),
    );
    return (
        <Loading loading={isInitializedLoading || isFullscreenGuardLoading}>
            <div className="bg-iota-lightest px-7 py-6 flex h-full flex-col items-center overflow-auto rounded-20 shadow-wallet-content">
                <div className="shrink-0">
                    <Logo />
                </div>
                <div className="mx-auto mt-2 text-center">
                    <Heading variant="heading2" color="gray-90" as="h1" weight="bold">
                        Welcome to Iota Wallet
                    </Heading>
                    <div className="mt-2">
                        <Text variant="pBody" color="steel-dark" weight="medium">
                            Connecting you to the decentralized web and Iota network.
                        </Text>
                    </div>
                </div>
                <div className="mt-3.5 flex h-full w-full items-center justify-center">
                    <WelcomeSplash role="img" />
                </div>
                <div className="mt-3.5 flex w-full flex-col items-center gap-3">
                    <Text variant="pBody" color="steel-dark" weight="medium">
                        Sign in with your preferred service
                    </Text>
                    <Button
                        to="/accounts/add-account?sourceFlow=Onboarding"
                        size="tall"
                        variant="secondary"
                        text="More Options"
                        disabled={
                            createAccountsMutation.isPending || createAccountsMutation.isSuccess
                        }
                    />
                </div>
            </div>
        </Loading>
    );
}
