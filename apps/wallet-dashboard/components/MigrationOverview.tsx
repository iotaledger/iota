// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTheme, Theme } from '@iota/core';
import { useRouter } from 'next/navigation';
import { Banner } from './Banner';

export function MigrationOverview() {
    const { theme } = useTheme();
    const router = useRouter();

    const videoSrc =
        theme === Theme.Dark
            ? 'https://files.iota.org/media/tooling/wallet-dashboard-migration-dark.mp4'
            : 'https://files.iota.org/media/tooling/wallet-dashboard-migration-light.mp4';

    function handleButtonClick() {
        router.push('/migrate');
    }
    return (
        <Banner
            videoSrc={videoSrc}
            title="Migration"
            subtitle="Fast & Easy"
            onButtonClick={handleButtonClick}
            buttonText="Start Migration"
        />
    );
}
