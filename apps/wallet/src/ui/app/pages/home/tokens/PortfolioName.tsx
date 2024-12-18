// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_src/ui/app/shared/text';

export function PortfolioName({ name }: { name: string }) {
    return (
        <div className="flex w-full items-center justify-center gap-4 truncate">
            <div className="h-px flex-1 bg-gray-45" />
            <div className="truncate">
                <Text variant="caption" weight="semibold" color="steel-darker" truncate>
                    {name} Portfolio
                </Text>
            </div>
            <div className="h-px flex-1 bg-gray-45" />
        </div>
    );
}
