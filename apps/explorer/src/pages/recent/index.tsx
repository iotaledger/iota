// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Activity, ErrorBoundary, PageLayout } from '~/components';
import { useSearchParamsMerged } from '~/components/ui';

const TRANSACTIONS_LIMIT = 20;

export function Recent(): JSX.Element {
    const [searchParams] = useSearchParamsMerged();

    return (
        <PageLayout
            content={
                <div data-testid="transaction-page" id="transaction" className="mx-auto">
                    <ErrorBoundary>
                        <Activity
                            initialLimit={TRANSACTIONS_LIMIT}
                            initialTab={searchParams.get('tab')}
                        />
                    </ErrorBoundary>
                </div>
            }
        />
    );
}
