// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PermissionType } from '_src/shared/messaging/messages/payloads/permissions';
import { SummaryListItem } from './SummaryListItem';
import { Checkmark } from '@iota/apps-ui-icons';

export interface DAppPermissionListProps {
    permissions: PermissionType[];
}

const PERMISSION_TYPE_TO_TEXT: Record<PermissionType, string> = {
    viewAccount: 'Share wallet address',
    suggestTransactions: 'Suggest transactions to approve',
};

export function DAppPermissionList({ permissions }: DAppPermissionListProps) {
    return (
        <div className="flex flex-col gap-y-xs">
            {permissions.map((permissionKey) => (
                <SummaryListItem
                    key={permissionKey}
                    icon={<Checkmark className="h-5 w-5 text-neutral-10 dark:text-neutral-92" />}
                    text={PERMISSION_TYPE_TO_TEXT[permissionKey]}
                />
            ))}
        </div>
    );
}
