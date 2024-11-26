// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function determineCountDownText({
    timeAgo,
    label,
    endLabel,
}: {
    timeAgo: string;
    label?: string;
    endLabel?: string;
}): string {
    const showLabel = timeAgo !== endLabel;
    return showLabel ? `${label} ${timeAgo}` : timeAgo;
}
