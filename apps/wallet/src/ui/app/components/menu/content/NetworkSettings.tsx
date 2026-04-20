// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NetworkSelector } from '_components';
import { PageTemplate } from '_src/ui/app/components/PageTemplate';

export function NetworkSettings() {
    return (
        <PageTemplate title="Network">
            <NetworkSelector />
        </PageTemplate>
    );
}
