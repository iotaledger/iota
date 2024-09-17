// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTimeAgo } from '@iota/core';

type TxTimeTypeProps = {
    timestamp: number | undefined;
};

export function TxTimeType({ timestamp }: TxTimeTypeProps): JSX.Element {
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
