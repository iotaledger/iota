// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatAmountParts } from '@mysten/core';

import { Stats, type StatsProps } from '~/ui/Stats';

interface FormattedStatsAmountProps {
    amount?: string | number | bigint;
}

// Simple wrapper around stats to avoid text wrapping:
export function StatsWrapper(props: StatsProps): JSX.Element {
    return (
        <div className="flex-shrink-0">
            <Stats {...props} />
        </div>
    );
}

export function FormattedStatsAmount({
    amount,
    ...props
}: FormattedStatsAmountProps & StatsProps): JSX.Element {
    const [formattedAmount, postfix] = formatAmountParts(amount);

    return (
        <StatsWrapper {...props} postfix={postfix}>
            {formattedAmount}
        </StatsWrapper>
    );
}
