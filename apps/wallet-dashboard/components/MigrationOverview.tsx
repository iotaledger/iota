// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTheme, Theme } from '@iota/core';
import { useRouter } from 'next/navigation';
import { Banner } from './Banner';
import { useCurrentAccount } from '@iota/dapp-kit';
import { useGetStardustMigratableObjects } from '@/hooks';
import { MIGRATION_ROUTE } from '@/lib/constants/routes.constants';

export function MigrationOverview() {
    const { theme } = useTheme();
    const router = useRouter();
    const account = useCurrentAccount();
    const address = account?.address || '';
    const { data: { migratableBasicOutputs = [], migratableNftOutputs = [] } = {} } =
        useGetStardustMigratableObjects(address);

    const needsMigration = migratableBasicOutputs.length > 0 || migratableNftOutputs.length > 0;

    const videoSrc =
        theme === Theme.Dark
            ? 'https://files.iota.org/media/tooling/wallet-dashboard-migration-dark.mp4'
            : 'https://files.iota.org/media/tooling/wallet-dashboard-migration-light.mp4';

    function handleButtonClick() {
        router.push(MIGRATION_ROUTE.path);
    }
    return needsMigration ? (
        <div style={{ gridArea: 'migration' }} className="with-migration flex grow overflow-hidden">
            <Banner
                videoSrc={videoSrc}
                title="Migration"
                subtitle="Fast & Easy"
                onButtonClick={handleButtonClick}
                buttonText="Start Migration"
            />
        </div>
    ) : null;
}
