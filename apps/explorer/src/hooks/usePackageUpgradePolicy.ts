// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetTransaction } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useMemo } from 'react';
import { UPGRADE_POLICIES, type UpgradePolicyInfo } from '~/lib';

const UPGRADE_CAP_TYPE = '0x2::package::UpgradeCap';

const IMMUTABLE_POLICY: UpgradePolicyInfo = {
    label: 'Immutable',
    description: 'Prevents any upgrades to the package. The UpgradeCap has been destroyed.',
    isImmutable: true,
};

const CUSTOM_POLICY: UpgradePolicyInfo = {
    label: 'Custom',
    description:
        'The UpgradeCap is wrapped inside a custom policy object. The package may still be upgradeable under custom conditions.',
    isImmutable: false,
};

const MAKE_IMMUTABLE_FUNCTION = {
    package: '0x2',
    module: 'package',
    function: 'make_immutable',
} as const;

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

    // When the UpgradeCap is not accessible (deleted or wrapped), check whether
    // it was destroyed via `make_immutable` or wrapped in a custom policy object.
    const upgradeCapMissing =
        !!upgradeCapObjectId &&
        !isUpgradeCapPending &&
        (!!upgradeCapData?.error || !upgradeCapData?.data);

    const { data: lastCapTxData, isPending: isLastCapTxPending } = useIotaClientQuery(
        'queryTransactionBlocks',
        {
            filter: { InputObject: upgradeCapObjectId! },
            options: { showInput: true },
            order: 'descending',
            limit: 1,
        },
        {
            enabled: upgradeCapMissing,
        },
    );

    const upgradePolicy = useMemo<UpgradePolicyInfo | null>(() => {
        const isUpgradeCapLoading = !!upgradeCapObjectId && isUpgradeCapPending;
        const isLastCapTxLoading = upgradeCapMissing && isLastCapTxPending;

        if (!txDigest || isTxPending || isUpgradeCapLoading || isLastCapTxLoading) {
            return null;
        }

        if (!upgradeCapObjectId) {
            return IMMUTABLE_POLICY;
        }

        // UpgradeCap exists and is accessible: read the policy field
        if (upgradeCapData?.data) {
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
        }

        // UpgradeCap is missing: determine if it was destroyed or wrapped
        const lastTx = lastCapTxData?.data?.[0];
        if (lastTx?.transaction?.data?.transaction?.kind === 'ProgrammableTransaction') {
            const transactions = lastTx.transaction.data.transaction.transactions;
            const normalizedPkg = normalizeIotaAddress(MAKE_IMMUTABLE_FUNCTION.package);
            const wasMadeImmutable = transactions.some(
                (tx) =>
                    'MoveCall' in tx &&
                    normalizeIotaAddress(tx.MoveCall.package) === normalizedPkg &&
                    tx.MoveCall.module === MAKE_IMMUTABLE_FUNCTION.module &&
                    tx.MoveCall.function === MAKE_IMMUTABLE_FUNCTION.function,
            );
            return wasMadeImmutable ? IMMUTABLE_POLICY : CUSTOM_POLICY;
        }

        return null;
    }, [
        txDigest,
        upgradeCapObjectId,
        upgradeCapData,
        upgradeCapMissing,
        lastCapTxData,
        isTxPending,
        isUpgradeCapPending,
        isLastCapTxPending,
    ]);

    const isPending =
        !!txDigest &&
        (isTxPending ||
            (!!upgradeCapObjectId && isUpgradeCapPending) ||
            (upgradeCapMissing && isLastCapTxPending));

    return {
        upgradePolicy,
        isPending,
    };
}
