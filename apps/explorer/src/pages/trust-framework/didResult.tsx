// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PageHeader, PageLayout } from '~/components';

export function DidResult() {
    return (
        <PageLayout
            content={
                <div className="flex w-full items-center justify-center">
                    <PageHeader type="Object" title="DID Result Page" />
                </div>
            }
        />
    );
}
