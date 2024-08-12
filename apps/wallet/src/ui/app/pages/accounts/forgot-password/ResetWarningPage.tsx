// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '_app/shared/ButtonUI';
import { useAccounts } from '_src/ui/app/hooks/useAccounts';
import { Navigate, useNavigate } from 'react-router-dom';

import { RecoverAccountsGroup } from '_components/accounts/RecoverAccountsGroup';
import { useAccountGroups } from '../../../hooks/useAccountGroups';
import { Heading } from '../../../shared/heading';
import { Text } from '../../../shared/text';
import { getGroupTitle } from '../manage/AccountGroup';
import { useForgotPasswordContext } from './ForgotPasswordPage';

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
    return (
        <div className="flex h-full w-full flex-col items-center overflow-auto">
            <div className="flex flex-col items-center gap-2 text-center">
                <Heading variant="heading1" color="gray-90" as="h1" weight="bold">
                    Reset Password
                </Heading>
                <Text variant="pBody" color="gray-90">
                    To ensure wallet security, the following accounts will be removed as part of the
                    password reset process. You will need to connect/import them again.
                </Text>
            </div>
            <div className="bg-hero-darkest/5 mb-10 mt-5 flex w-full flex-1 flex-col gap-8 overflow-auto rounded-lg px-4 py-6">
                {accountGroupsToRemove.map(([sourceID, accounts]) => (
                    <RecoverAccountsGroup
                        key={sourceID}
                        accounts={accounts}
                        title={getGroupTitle(accounts[0])}
                    />
                ))}
            </div>
            <div className="flex w-full gap-3">
                <Button variant="outline" size="tall" text="Back" onClick={() => navigate(-1)} />
                <Button
                    type="submit"
                    variant="primary"
                    size="tall"
                    text="Continue"
                    onClick={() => navigate('../reset')}
                />
            </div>
        </div>
    );
}
