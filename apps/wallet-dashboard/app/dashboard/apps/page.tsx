// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { AppList } from '@/components';

function AppsPage(): JSX.Element {
    return (
        <div className="flex items-center justify-center pt-12">
            <h1>APPS</h1>
            <AppList />
        </div>
    );
}

export default AppsPage;
