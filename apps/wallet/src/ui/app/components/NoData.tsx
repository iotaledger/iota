// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

interface NoDataProps {
    message: string;
}

export function NoData({ message }: NoDataProps) {
    return (
        <div className="flex h-full flex-col items-center justify-center gap-md text-center">
            <img src="_assets/images/no_data.svg" alt="No data" />
            <span className="text-label-lg text-neutral-60">{message}</span>
        </div>
    );
}
