// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { PermissionType } from '_messages/payloads/permissions';
import { Checkmark } from '@iota/ui-icons';

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
                <li key={aPermission} className="flex flex-row flex-nowrap items-center gap-2.5">
                    <Checkmark className="h-5 w-5" />
                    <div className="text-body-md text-neutral-40">
                        {PERMISSION_TYPE_TO_TEXT[aPermission]}
                    </div>
                </li>
            ))}
        </ul>
    );
}
