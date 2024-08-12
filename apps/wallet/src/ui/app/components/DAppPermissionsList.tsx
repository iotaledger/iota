// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { PermissionType } from '_messages/payloads/permissions';
import { CheckFill12 } from '@iota/icons';

import { Text } from '_app/shared/text';

export interface DAppPermissionsListProps {
    permissions: PermissionType[];
}

const PERMISSION_TYPE_TO_TEXT: Record<PermissionType, string> = {
    viewAccount: 'Share wallet address',
    suggestTransactions: 'Suggest transactions to approve',
};

export function DAppPermissionsList({ permissions }: DAppPermissionsListProps) {
    return (
        <ul className="m-0 flex list-none flex-col gap-3 p-0">
            {permissions.map((aPermission) => (
                <li key={aPermission} className="flex flex-row flex-nowrap items-center gap-2">
                    <CheckFill12 className="text-steel" />
                    <Text variant="bodySmall" weight="medium" color="steel-darker">
                        {PERMISSION_TYPE_TO_TEXT[aPermission]}
                    </Text>
                </li>
            ))}
        </ul>
    );
}
