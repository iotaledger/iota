// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import type { ReactNode } from 'react';

export interface CardItemProps {
    title: ReactNode;
    children: ReactNode;
}

export function CardItem({ title, children }: CardItemProps) {
    return (
        <div
            className={
                'flex max-w-full flex-1 flex-col flex-nowrap items-center justify-center gap-1.5 px-2.5 py-3.5'
            }
        >
            <Text variant="captionSmall" weight="semibold" color="steel-darker">
                {title}
            </Text>

            {children}
        </div>
    );
}
