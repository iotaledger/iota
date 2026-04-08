// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { UpgradePolicy } from '@iota/iota-sdk/transactions';

export interface UpgradePolicyInfo {
    label: string;
    description: string;
    isImmutable: boolean;
}

export const UPGRADE_DOCS_URL =
    'https://docs.iota.org/developer/iota-101/move-overview/package-upgrades/custom-policies';

export const UPGRADE_POLICIES: Record<number, { label: string; description: string }> = {
    [UpgradePolicy.COMPATIBLE]: {
        label: 'Compatible',
        description:
            'Permits changes to all function implementations, removal of ability constraints on generic type parameters, and modifications to private, public(friend), and entry function signatures. Public function signatures and existing types cannot be changed.',
    },
    [UpgradePolicy.ADDITIVE]: {
        label: 'Additive',
        description:
            'Allows adding new functionalities (e.g., new public functions or structs) but restricts changes to existing functionalities.',
    },
    [UpgradePolicy.DEP_ONLY]: {
        label: 'Dependency-only',
        description: "Limits modifications to the package's dependencies only.",
    },
};
