// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { IotaClient } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

const CLOCK_PACKAGE_ID = '0x06';

type ClockFields = {
    id: {
        id: string;
    };
    timestamp_ms: string;
};

export function useGetClockTimestamp() {
    const client = useIotaClient();
    return useQuery({
        queryKey: ['get-clock-timestamp', client],
        queryFn: async () => {
            return getClockTimestamp(client);
        },
        staleTime: 10 * 1000,
        refetchInterval: 10 * 1000, // refetch every 10 seconds to keep the clock updated but not overload the server
    });
}

export async function getClockTimestamp(client: IotaClient): Promise<number | undefined> {
    const clockRes = await client.getObject({
        id: CLOCK_PACKAGE_ID,
        options: { showContent: true },
    });

    if (!clockRes?.data?.content || !('fields' in clockRes.data.content)) {
        throw undefined;
    }

    const fields = clockRes.data.content.fields as ClockFields;
    return Number(fields.timestamp_ms);
}
