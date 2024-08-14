// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_src/ui/app/shared/text';

export function PortfolioName({ name }: { name: string }) {
    return (
        <div className="flex w-full items-center justify-center gap-4 truncate">
            <div className="bg-gray-45 h-px flex-1" />
            <div className="truncate">
                <Text variant="caption" weight="semibold" color="steel-darker" truncate>
                    {name} Portfoliof
                </Text>
            </div>
            <div className="bg-gray-45 h-px flex-1" />
        </div>
    );
}
