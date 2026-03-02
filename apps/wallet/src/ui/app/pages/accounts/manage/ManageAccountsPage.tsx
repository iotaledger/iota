// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import { Button, ButtonSize, ButtonType } from '@iota/apps-ui-kit';
import { type AccountType } from '_src/background/accounts/account';
import { useInitializedGuard, useAccountGroups } from '_hooks';
import { useNavigate } from 'react-router-dom';
import { Overlay, useUnlockAccounts } from '_components';
import { AccountGroup } from './AccountGroup';
import { LockLocked } from '@iota/apps-ui-icons';
import { AmpliSourceFlow } from '_src/shared/analytics';
import { useSourceFlow } from '_components/accounts/AccountsFormContext';

export function ManageAccountsPage() {
    const navigate = useNavigate();
    const groupedAccounts = useAccountGroups();
    const { lockAccounts } = useUnlockAccounts();
    const outerRef = useRef<HTMLDivElement>(null);
    const { setSourceFlow } = useSourceFlow();
    useInitializedGuard(true);

    function handleAdd() {
        setSourceFlow(AmpliSourceFlow.ManageAccounts);
        navigate('/accounts/add-account');
    }

    function handleLock() {
        lockAccounts();
    }

    return (
        <Overlay
            showModal
            title="Manage Accounts"
            showBackButton
            titleCentered={false}
            hideCloseIcon
            headerAction={
                <Button
                    type={ButtonType.Secondary}
                    size={ButtonSize.Small}
                    onClick={handleLock}
                    icon={<LockLocked className="h-5 w-5" />}
                    text="Lock"
                    testId="lock-wallet"
                />
            }
        >
            <div className="flex h-full w-full flex-col">
                <div className="flex flex-1 flex-col overflow-y-auto">
                    <div ref={outerRef} className="relative">
                        {Object.entries(groupedAccounts).map(([type, accountGroups]) =>
                            Object.entries(accountGroups).map(
                                ([key, { sourceId, accounts }], index) => {
                                    return (
                                        <AccountGroup
                                            outerRef={outerRef}
                                            key={`${type}-${key}`}
                                            accounts={accounts}
                                            accountSourceID={sourceId}
                                            type={type as AccountType}
                                            isLast={
                                                index === Object.entries(accountGroups).length - 1
                                            }
                                        />
                                    );
                                },
                            ),
                        )}
                        <div id="manage-account-item-portal-container"></div>
                    </div>
                </div>
                <div className="pt-sm">
                    <Button
                        type={ButtonType.Primary}
                        text="Add Profile"
                        onClick={handleAdd}
                        fullWidth
                    />
                </div>
            </div>
        </Overlay>
    );
}
