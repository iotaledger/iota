// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Heading } from '@iota/ui';
import { type ReactNode } from 'react';

import { Card } from '~/components/ui';

interface EpochStatsProps {
    label: string;
    children: ReactNode;
}

export function EpochStats({ label, children }: EpochStatsProps): JSX.Element {
    return (
        <Card spacing="lg" rounded="2xl">
            <div className="flex flex-col gap-8">
                {label && (
                    <Heading color="steel-darker" variant="heading4/semibold">
                        {label}
                    </Heading>
                )}
                <div className="grid grid-cols-2 gap-8">{children}</div>
            </div>
        </Card>
    );
}
