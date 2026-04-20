// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NetworkSelector } from '_components';
import { FaucetRequestButton } from '_src/ui/app/shared/faucet/FaucetRequestButton';
import { PageTemplate } from '_src/ui/app/components/PageTemplate';

export function NetworkPage() {
    return (
        <PageTemplate title="Network">
            <div className="flex flex-col gap-lg">
                <NetworkSelector />
                <FaucetRequestButton />
            </div>
        </PageTemplate>
    );
}
