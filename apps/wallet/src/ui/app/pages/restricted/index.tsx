// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaLogoWeb } from '@iota/apps-ui-icons';
import { PageMainLayout } from '_src/ui/app/shared/page-main-layout/PageMainLayout';
import { useInitializedGuard } from '_hooks';

export function RestrictedPage() {
    useInitializedGuard(true);

    const CURRENT_YEAR = new Date().getFullYear();

    return (
        <PageMainLayout>
            <div className="flex h-full w-full flex-col items-center justify-between bg-neutral-100 px-md py-2xl shadow-wallet-content dark:bg-neutral-6">
                <IotaLogoWeb
                    width={130}
                    height={32}
                    className="text-neutral-10 dark:text-neutral-92"
                />
                <div className="flex flex-col items-center text-center">
                    <span className="text-title-lg text-neutral-40 dark:text-neutral-60">
                        Regrettably this service is currently not available. Please try again later.
                    </span>
                </div>
                <div className="text-body-lg text-neutral-60">
                    &copy; IOTA Foundation {CURRENT_YEAR}
                </div>
            </div>
        </PageMainLayout>
    );
}
