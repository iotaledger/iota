// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatAddress } from '@iota/iota-sdk/utils';
import { truncateString } from '@iota/core';

export function formatAccountName(
    nickname: string | undefined | null,
    address: string | undefined,
): string {
    if (nickname) {
        return truncateString(nickname, 12);
    } else {
        return formatAddress(address || '');
    }
}
