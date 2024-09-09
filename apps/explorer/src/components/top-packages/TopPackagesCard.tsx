// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';

import { FilterList, TabHeader } from '~/components/ui';
import { useEnhancedRpcClient } from '~/hooks/useEnhancedRpc';
import { ErrorBoundary } from '../error-boundary/ErrorBoundary';
import { TopPackagesTable } from './TopPackagesTable';
import { Panel } from '@iota/apps-ui-kit';

type DateFilter = '3D' | '7D' | '30D';
type ApiDateFilter = 'rank3Days' | 'rank7Days' | 'rank30Days';
export const FILTER_TO_API_FILTER: Record<DateFilter, ApiDateFilter> = {
    '3D': 'rank3Days',
    '7D': 'rank7Days',
    '30D': 'rank30Days',
};

export function TopPackagesCard(): JSX.Element {
    const rpc = useEnhancedRpcClient();
    const [selectedFilter, setSelectedFilter] = useState<DateFilter>('3D');

    const { data, isPending } = useQuery({
        queryKey: ['top-packages', selectedFilter],
        queryFn: async () => rpc.getMoveCallMetrics(),
    });

    const filteredData = data ? data[FILTER_TO_API_FILTER[selectedFilter]] : [];

    return (
        <Panel>
            <div className="relative">
                <div className="absolute right-0 mt-1">
                    <FilterList
                        lessSpacing
                        options={['3D', '7D', '30D']}
                        value={selectedFilter}
                        onChange={(val) => setSelectedFilter(val)}
                    />
                </div>
                <TabHeader
                    title="Popular Packages"
                    tooltip="Popular packages is recomputed on epoch changes."
                >
                    <ErrorBoundary>
                        <TopPackagesTable data={filteredData} isLoading={isPending} />
                    </ErrorBoundary>
                </TabHeader>
            </div>
        </Panel>
    );
}
