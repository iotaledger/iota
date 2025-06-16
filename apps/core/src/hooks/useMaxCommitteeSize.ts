// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useMaxCommitteeSize() {
    const client = useIotaClient();

    return useQuery({
        queryKey: ['protocol-config-max-committee-members-count'],
        queryFn: async () => {
            const config = await client.getProtocolConfig();
            const max_committee_members_count = config.attributes['max_committee_members_count'];
            return max_committee_members_count && 'u64' in max_committee_members_count
                ? Number(max_committee_members_count.u64)
                : Infinity;
        },
        enabled: !!client,
    });
}
