// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_src/ui/app/shared/text';
import { type ReactNode } from 'react';

interface BadgeLabelProps {
    label: ReactNode;
}

export function BadgeLabel({ label }: BadgeLabelProps) {
    return (
        <div className="border-gray-45 bg-gray-40 rounded-2xl border border-solid px-1.5 py-1">
            <Text variant="captionSmallExtra" color="steel-dark">
                {label}
            </Text>
        </div>
    );
}
