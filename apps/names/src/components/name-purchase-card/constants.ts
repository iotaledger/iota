// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NameAvailabilityStatus } from './enums';

export const BG_COLORS: Record<NameAvailabilityStatus, string> = {
    [NameAvailabilityStatus.Available]: 'bg-names-neutral-10',
    [NameAvailabilityStatus.Reserved]: 'bg-names-neutral-10',
    [NameAvailabilityStatus.Unavailable]: 'bg-names-error-20',
};

export const TEXT_COLORS: Record<NameAvailabilityStatus, string> = {
    [NameAvailabilityStatus.Available]: 'text-names-tertiary-80',
    [NameAvailabilityStatus.Reserved]: 'text-names-warning-80',
    [NameAvailabilityStatus.Unavailable]: 'text-names-error-80',
};
