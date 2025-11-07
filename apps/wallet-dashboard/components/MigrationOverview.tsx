// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTheme, Theme, Banner } from '@iota/core';
import { useRouter } from 'next/navigation';
import { useCurrentAccount } from '@iota/dapp-kit';
import { useGetStardustMigratableObjects } from '@/hooks';
import { MIGRATION_ROUTE } from '@/lib/constants/routes.constants';
import { Button, ButtonSize, ButtonType } from '@iota/apps-ui-kit';

export function MigrationOverview() {
    const { theme } = useTheme();
    const router = useRouter();
    const account = useCurrentAccount();
    const address = account?.address || '';
    const { migratableBasicOutputs = [], migratableNftOutputs = [] } =
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
            <Banner videoSrc={videoSrc} title="Migration" subtitle="Fast & Easy">
                <Button
                    onClick={handleButtonClick}
                    size={ButtonSize.Small}
                    type={ButtonType.Outlined}
                    text="Start Migration"
                />
            </Banner>
        </div>
    ) : null;
}
