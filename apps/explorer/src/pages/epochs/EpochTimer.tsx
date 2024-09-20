// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '@iota/ui';

import { useEpochProgress } from '~/pages/epochs/utils';
import { ProgressCircle } from '~/components/ui';

export function EpochTimer(): JSX.Element | null {
    const { epoch, progress, label } = useEpochProgress();
    if (!epoch) return null;
    return (
        <div className="flex w-full items-center justify-center gap-1.5 rounded-full border border-gray-45 px-2.5 py-2 shadow-notification">
            <div className="w-5 text-steel-darker">
                <ProgressCircle progress={progress} />
            </div>
            <Text variant="pBodySmall/medium" color="steel-darker">
                Epoch {epoch} in progress. {label}
            </Text>
        </div>
    );
}
