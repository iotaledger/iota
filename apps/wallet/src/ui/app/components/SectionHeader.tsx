// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '../shared/text';

interface SectionHeaderProps {
    title: string;
}

export function SectionHeader({ title }: SectionHeaderProps) {
    return (
        <div className="flex items-center justify-center gap-3">
            <div className="bg-gray-45 flex h-px flex-1 flex-shrink-0" />
            <Text variant="caption" weight="semibold" color="steel">
                {title}
            </Text>
            <div className="bg-gray-45 flex h-px flex-1 flex-shrink-0" />
        </div>
    );
}
