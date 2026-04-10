// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface ProgressBarProps {
    progress: number;
}

export function ProgressBar({ progress }: ProgressBarProps): JSX.Element {
    return (
        <div className="relative w-full rounded-full bg-iota-primary-90 dark:bg-iota-primary-10">
            <div
                className="h-1 rounded-full bg-iota-primary-30 dark:bg-iota-primary-80"
                style={{ width: `${progress}%` }}
            />
        </div>
    );
}
