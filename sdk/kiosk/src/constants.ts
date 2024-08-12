// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// eslint-disable-next-line import/no-cycle
import type { KioskConfiguration } from '@iota/iota-sdk/client';
import { getAllNetworks } from '@iota/iota-sdk/client';
import {
    resolveFloorPriceRule,
    resolveKioskLockRule,
    resolvePersonalKioskRule,
    resolveRoyaltyRule,
} from './tx/rules//resolve.js';
import type { ObjectArgument, RuleResolvingParams } from './types/index.js';

/**
 * The base rule package ids that can be extended
 */
export type BaseRulePackageIds = {
    royaltyRulePackageId?: string;
    kioskLockRulePackageId?: string;
    personalKioskRulePackageId?: string;
    floorPriceRulePackageId?: string;
};

/**
 * The Transfer Policy rule.
 */
export type TransferPolicyRule = {
    rule: string;
    packageId: string;
    resolveRuleFunction: (rule: RuleResolvingParams) => ObjectArgument | void;
    hasLockingRule?: boolean;
};

/**
 * Constructs a list of rule resolvers based on the params.
 */
export function getBaseRules({
    royaltyRulePackageId,
    kioskLockRulePackageId,
    personalKioskRulePackageId,
    floorPriceRulePackageId,
}: BaseRulePackageIds): TransferPolicyRule[] {
    const rules = [];

    if (royaltyRulePackageId) {
        rules.push({
            rule: `${royaltyRulePackageId}::royalty_rule::Rule`,
            packageId: royaltyRulePackageId,
            resolveRuleFunction: resolveRoyaltyRule,
        });
    }

    if (kioskLockRulePackageId) {
        rules.push({
            rule: `${kioskLockRulePackageId}::kiosk_lock_rule::Rule`,
            packageId: kioskLockRulePackageId,
            resolveRuleFunction: resolveKioskLockRule,
            hasLockingRule: true,
        });
    }

    if (personalKioskRulePackageId) {
        rules.push({
            rule: `${personalKioskRulePackageId}::personal_kiosk_rule::Rule`,
            packageId: personalKioskRulePackageId,
            resolveRuleFunction: resolvePersonalKioskRule,
        });
    }

    if (floorPriceRulePackageId) {
        rules.push({
            rule: `${floorPriceRulePackageId}::floor_price_rule::Rule`,
            packageId: floorPriceRulePackageId,
            resolveRuleFunction: resolveFloorPriceRule,
        });
    }

    return rules;
}

export const rules: TransferPolicyRule[] = Object.values(getAllNetworks())
    .filter((network) => network.kiosk)
    .flatMap((network) => getBaseRules(network.kiosk as KioskConfiguration));
