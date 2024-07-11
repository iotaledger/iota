// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { StatsWrapper } from './FormattedStatsAmount';
import { useGetNetworkMetrics } from '~/hooks/useGetNetworkMetrics';

export function Checkpoint(): JSX.Element {
    const { data, isPending } = useGetNetworkMetrics();

    return (
        <StatsWrapper
            label="Checkpoint"
            tooltip="The current checkpoint"
            unavailable={isPending}
            size="sm"
            orientation="horizontal"
        >
            {data?.currentCheckpoint ? BigInt(data?.currentCheckpoint).toLocaleString() : null}
        </StatsWrapper>
    );
}
