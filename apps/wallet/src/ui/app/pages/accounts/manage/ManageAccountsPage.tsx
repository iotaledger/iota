// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { type AccountType } from '_src/background/accounts/Account';
import { useInitializedGuard } from '_src/ui/app/hooks';
import { useAccountGroups } from '_src/ui/app/hooks/useAccountGroups';
import { useNavigate } from 'react-router-dom';

import { Overlay } from '_components';
import { AccountGroup } from './AccountGroup';

export function ManageAccountsPage() {
    const navigate = useNavigate();
    const groupedAccounts = useAccountGroups();
    useInitializedGuard(true);
    return (
        <Overlay showModal title="Manage Accounts" closeOverlay={() => navigate('/home')}>
            <div className="flex min-h-full flex-1 flex-col justify-between gap-10">
                {Object.entries(groupedAccounts).map(([type, accountGroups]) =>
                    Object.entries(accountGroups).map(([key, accounts]) => {
                        return (
                            <AccountGroup
                                key={`${type}-${key}`}
                                accounts={accounts}
                                accountSourceID={key}
                                type={type as AccountType}
                            />
                        );
                    }),
                )}
            </div>
        </Overlay>
    );
}
