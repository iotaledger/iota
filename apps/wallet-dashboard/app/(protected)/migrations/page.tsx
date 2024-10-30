// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { useFeature } from '@growthbook/growthbook-react';
import { Feature } from '@iota/core';
import { useRouter } from 'next/navigation';
import { useEffect } from 'react';

function MigrationDashboardPage(): JSX.Element {
    const router = useRouter();
    const stardustMigrationEnabled = useFeature<boolean>(Feature.StardustMigration).value;

    useEffect(() => {
        if (!stardustMigrationEnabled) {
            router.push('/');
        }
    }, [stardustMigrationEnabled, router]);

    return (
        <div className="flex items-center justify-center pt-12">
            <h1>MIGRATIONS</h1>
        </div>
    );
}

export default MigrationDashboardPage;
