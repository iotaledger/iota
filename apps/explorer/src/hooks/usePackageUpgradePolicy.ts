// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetTransaction } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { UpgradePolicy } from '@iota/iota-sdk/transactions';
import { useMemo } from 'react';

const UPGRADE_CAP_TYPE = '0x2::package::UpgradeCap';

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

const IMMUTABLE_POLICY: UpgradePolicyInfo = {
    label: 'Immutable',
    description: 'Prevents any upgrades to the package. The UpgradeCap has been destroyed.',
    isImmutable: true,
};

export function usePackageUpgradePolicy(txDigest: string | null | undefined): {
    upgradePolicy: UpgradePolicyInfo | null;
    isPending: boolean;
} {
    const { data: txnData, isPending: isTxPending } = useGetTransaction(txDigest ?? '');

    const upgradeCapObjectId = useMemo(() => {
        if (!txnData?.objectChanges) return undefined;
        const upgradeCapChange = txnData.objectChanges.find(
            (change) =>
                change.type === 'created' &&
                'objectType' in change &&
                change.objectType === UPGRADE_CAP_TYPE,
        );
        return upgradeCapChange && 'objectId' in upgradeCapChange
            ? upgradeCapChange.objectId
            : undefined;
    }, [txnData?.objectChanges]);

    const { data: upgradeCapData, isPending: isUpgradeCapPending } = useIotaClientQuery(
        'getObject',
        {
            id: upgradeCapObjectId!,
            options: { showContent: true },
        },
        {
            enabled: !!upgradeCapObjectId,
        },
    );

    const upgradePolicy = useMemo<UpgradePolicyInfo | null>(() => {
        const isUpgradeCapLoading = !!upgradeCapObjectId && isUpgradeCapPending;

        if (!txDigest || isTxPending || isUpgradeCapLoading) {
            return null;
        }

        if (!upgradeCapObjectId || upgradeCapData?.error || !upgradeCapData?.data) {
            return IMMUTABLE_POLICY;
        }

        const content = upgradeCapData.data.content;
        if (content?.dataType === 'moveObject' && content.fields) {
            const fields = content.fields as Record<string, unknown>;
            const policy = Number(fields.policy);
            const policyInfo = UPGRADE_POLICIES[policy];
            return {
                label: policyInfo?.label ?? `Unknown (${policy})`,
                description: policyInfo?.description ?? '',
                isImmutable: false,
            };
        }

        return null;
    }, [txDigest, upgradeCapObjectId, upgradeCapData, isTxPending, isUpgradeCapPending]);

    const isPending = !!txDigest && (isTxPending || (!!upgradeCapObjectId && isUpgradeCapPending));

    return {
        upgradePolicy,
        isPending,
    };
}
