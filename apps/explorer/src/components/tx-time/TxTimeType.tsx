// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

import { useTimeAgo } from '@iota/core';

type Prop = {
    timestamp: number | undefined;
};

export function TxTimeType({ timestamp }: Prop) {
    const timeAgo = useTimeAgo({
        timeFrom: timestamp || null,
        shortedTimeLabel: true,
    });

    return (
        <section>
            <div className="w-20 text-caption">{timeAgo}</div>
        </section>
    );
}
