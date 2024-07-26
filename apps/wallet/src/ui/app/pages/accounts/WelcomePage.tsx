// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// import { Button } from '_app/shared/ButtonUI';
import Loading from '_components/loading';
import { useFullscreenGuard, useInitializedGuard } from '_hooks';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import { IotaLogoWeb } from '@iota/icons';

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
            <div className="flex h-full w-full flex-col items-center justify-between overflow-auto rounded-20 bg-white px-xl py-2xl shadow-wallet-content">
                <div>
                    <IotaLogoWeb width={130} height={32} />
                </div>
                <div className={'flex flex-col items-center gap-8 text-center'}>
                    <div className="flex flex-col items-center gap-4">
                        <h4 className={'text-headline-sm text-neutral-40'}>Welcome to</h4>
                        <h1 className={'text-display-md text-neutral-10'}>IOTA Wallet</h1>
                        <h3 className={'text-title-md text-neutral-40'}>
                            Connecting you to the decentralized
                            <br />
                            web and IOTA network
                        </h3>
                    </div>

                    <div>
                        <Button type={ButtonType.Primary} text={'Add Profile'} />
                    </div>
                </div>

                <div className={'text-body-lg text-neutral-80'}>&copy; IOTA Foundation 2024</div>
            </div>
        </Loading>
    );
}
