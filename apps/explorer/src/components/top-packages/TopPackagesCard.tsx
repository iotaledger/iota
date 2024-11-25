// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';

import { FilterList } from '~/components/ui';
import { useEnhancedRpcClient } from '~/hooks/useEnhancedRpc';
import { ErrorBoundary } from '../error-boundary/ErrorBoundary';
import { TopPackagesTable } from './TopPackagesTable';
import { Panel, Title } from '@iota/apps-ui-kit';

type DateFilter = '3d' | '1w' | '1m';
type ApiDateFilter = 'rank3Days' | 'rank7Days' | 'rank30Days';
export const FILTER_TO_API_FILTER: Record<DateFilter, ApiDateFilter> = {
    '3d': 'rank3Days',
    '1w': 'rank7Days',
    '1m': 'rank30Days',
};

export function TopPackagesCard(): JSX.Element {
    const rpc = useEnhancedRpcClient();
    const [selectedFilter, setSelectedFilter] = useState<DateFilter>('3d');

    const { data, isPending } = useQuery({
        queryKey: ['top-packages', selectedFilter],
        queryFn: async () => rpc.getMoveCallMetrics(),
    });

    const filteredData = data ? data[FILTER_TO_API_FILTER[selectedFilter]] : [];

    return (
        <Panel>
            <div className="relative">
                <div className="absolute right-0 mr-2 mt-2">
                    <FilterList
                        options={Object.keys(FILTER_TO_API_FILTER) as DateFilter[]}
                        selected={selectedFilter}
                        onSelected={(val) => setSelectedFilter(val)}
                        filtersAsChip
                    />
                </div>
                <Title
                    title="Popular Packages"
                    tooltipText="Popular packages is recomputed on epoch changes."
                />
                <ErrorBoundary>
                    <div className="p-md--rs">
                        <TopPackagesTable data={filteredData} isLoading={isPending} />
                    </div>
                </ErrorBoundary>
            </div>
        </Panel>
    );
}
