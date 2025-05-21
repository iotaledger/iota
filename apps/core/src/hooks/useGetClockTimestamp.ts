// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { IotaClient } from '@iota/iota-sdk/client';
import { IOTA_CLOCK_OBJECT_ID } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';

type ClockFields = {
    id: {
        id: string;
    };
    timestamp_ms: string;
};

export function useGetClockTimestamp() {
    const client = useIotaClient();
    const { data } = useQuery({
        queryKey: ['get-clock-timestamp', client],
        queryFn: async () => getClockTimestamp(client),
        staleTime: 10 * 1000,
        refetchInterval: 10 * 1000, // refetch every 10 seconds to keep the clock updated but not overload the servery,
        placeholderData: Infinity,
    });

    return {
        data: data!,
    };
}

export async function getClockTimestamp(client: IotaClient): Promise<number> {
    const clockRes = await client.getObject({
        id: IOTA_CLOCK_OBJECT_ID,
        options: { showContent: true },
    });

    if (!clockRes?.data?.content || !('fields' in clockRes.data.content)) {
        return Infinity;
    }

    const fields = clockRes.data.content.fields as ClockFields;
    return Number(fields.timestamp_ms);
}
