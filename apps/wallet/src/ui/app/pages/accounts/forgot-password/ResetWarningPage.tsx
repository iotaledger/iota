// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAccounts, useAccountGroups } from '_hooks';
import { Navigate, useNavigate } from 'react-router-dom';
import { PageTemplate, RecoverAccountsGroup } from '_components';
import { getGroupTitle } from '../manage/AccountGroup';
import { useForgotPasswordContext } from './ForgotPasswordPage';
import { Button, ButtonHtmlType } from '@iota/apps-ui-kit';

export function ResetWarningPage() {
    const navigate = useNavigate();
    const accountGroups = useAccountGroups();
    const { value } = useForgotPasswordContext();
    const accountGroupsToRemove = Object.entries(accountGroups).flatMap(([groupType, aGroup]) =>
        Object.entries(aGroup).filter(
            ([sourceID]) => !value.find(({ accountSourceID }) => accountSourceID === sourceID),
        ),
    );
    const { isPending } = useAccounts();
    if (!value.length) {
        return <Navigate to="/accounts/forgot-password" replace />;
    }
    if (!accountGroupsToRemove.length && !isPending) {
        return <Navigate to="../reset" replace />;
    }

    function handleClose() {
        navigate('/');
    }

    function handleNext() {
        navigate('../reset');
    }

    return (
        <PageTemplate
            title="Reset your Password"
            isTitleCentered
            onClose={handleClose}
            showBackButton
        >
            <div className="flex h-full flex-col gap-lg overflow-auto">
                <span className="text-center text-label-lg text-neutral-40 dark:text-neutral-60">
                    To protect the security of your wallet, the accounts listed will be deleted as
                    part of the password reset procedure. Please reconnect or reimport them once the
                    process is complete.
                </span>
                <div className="flex w-full flex-1 flex-col gap-lg overflow-auto">
                    {accountGroupsToRemove.map(([sourceID, accounts]) => (
                        <RecoverAccountsGroup
                            key={sourceID}
                            accounts={accounts}
                            title={getGroupTitle(accounts[0])}
                        />
                    ))}
                </div>

                <Button htmlType={ButtonHtmlType.Submit} text="Continue" onClick={handleNext} />
            </div>
        </PageTemplate>
    );
}
