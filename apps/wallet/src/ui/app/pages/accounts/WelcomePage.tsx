// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Loading from '_components/loading';
import { useNavigate } from 'react-router-dom';
import { useFullscreenGuard, useInitializedGuard } from '_hooks';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import { IotaLogoWeb } from '@iota/ui-icons';

import { useCreateAccountsMutation } from '../../hooks/useCreateAccountMutation';

export function WelcomePage() {
    const createAccountsMutation = useCreateAccountsMutation();
    const isFullscreenGuardLoading = useFullscreenGuard(true);
    const isInitializedLoading = useInitializedGuard(
        false,
        !(createAccountsMutation.isPending || createAccountsMutation.isSuccess),
    );
    const navigate = useNavigate();

    return (
        <Loading loading={isInitializedLoading || isFullscreenGuardLoading}>
            <div className="flex h-full w-full flex-col items-center justify-between overflow-auto rounded-20 bg-white px-xl py-2xl shadow-wallet-content">
                <IotaLogoWeb width={130} height={32} />
                <div className="flex flex-col items-center gap-8 text-center">
                    <div className="flex flex-col items-center">
                        <h4 className="text-[38px] text-headline-sm text-neutral-40">Welcome to</h4>
                        <h1 className="text-[38px] text-display-md text-neutral-10">IOTA Wallet</h1>
                    </div>
                    <h3 className="text-title-md text-neutral-40">
                        Connecting you to the decentralized
                        <br />
                        web and IOTA network
                    </h3>
                    <div>
                        <Button
                            type={ButtonType.Primary}
                            text={'Add Profile'}
                            onClick={() => {
                                navigate('/accounts/add-account?sourceFlow=Onboarding');
                            }}
                            disabled={
                                createAccountsMutation.isPending || createAccountsMutation.isSuccess
                            }
                        />
                    </div>
                </div>

                <div className="text-body-lg text-neutral-80">&copy; IOTA Foundation 2024</div>
            </div>
        </Loading>
    );
}
