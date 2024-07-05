// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AddressType } from './address.enums';

export const TEXT_COLORS: Record<AddressType, string> = {
    [AddressType.Primary]: 'text-neutral-40',
    [AddressType.Secondary]: 'text-neutral-60',
};
