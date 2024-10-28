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
                <div className="absolute right-0 mr-2 mt-2">
                    <FilterList
                        options={['3D', '7D', '30D']}
                        selected={selectedFilter}
                        onSelected={(val) => setSelectedFilter(val)}
                    />
                </div>
                <Title
                    title="Popular Packages"
                    tooltipText="Popular packages is recomputed on epoch changes."
                />
                <ErrorBoundary>
                    <TopPackagesTable data={filteredData} isLoading={isPending} />
                </ErrorBoundary>
            </div>
        </Panel>
    );
}
