// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_src/ui/app/shared/text';
import { TransferObject16 } from '@iota/icons';

export interface NoActivityCardProps {
    message: string;
}

export function NoActivityCard({ message }: NoActivityCardProps) {
    return (
        <div className="flex h-full flex-col items-center justify-center gap-4 px-10 text-center">
            <TransferObject16 className="text-gray-45 text-3xl" />
            <Text variant="pBody" weight="medium" color="steel">
                {message}
            </Text>
        </div>
    );
}
