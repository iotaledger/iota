// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isMnemonicSerializedUiAccount } from '_src/background/accounts/MnemonicAccount';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { toast } from 'react-hot-toast';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import {
    ProtectAccountForm,
    VerifyPasswordModal,
    Loading,
    AccountsFormType,
    PageTemplate,
} from '_components';
import { useAccounts } from '../../hooks/useAccounts';
import { autoLockDataToMinutes } from '../../hooks/useAutoLockMinutes';
import { useAutoLockMinutesMutation } from '../../hooks/useAutoLockMinutesMutation';
import { useCreateAccountsMutation } from '../../hooks/useCreateAccountMutation';
import { isSeedSerializedUiAccount } from '_src/background/accounts/SeedAccount';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/LedgerAccount';
import { AllowedAccountSourceTypes } from '../../accounts-finder';

const ALLOWED_ACCOUNT_TYPES: AccountsFormType[] = [
    AccountsFormType.NewMnemonic,
    AccountsFormType.ImportMnemonic,
    AccountsFormType.ImportSeed,
    AccountsFormType.MnemonicSource,
    AccountsFormType.SeedSource,
    AccountsFormType.ImportPrivateKey,
    AccountsFormType.ImportLedger,
];

const REDIRECT_TO_ACCOUNTS_FINDER: AccountsFormType[] = [
    AccountsFormType.ImportMnemonic,
    AccountsFormType.ImportSeed,
    AccountsFormType.ImportLedger,
];

type AllowedAccountTypes = (typeof ALLOWED_ACCOUNT_TYPES)[number];

function isAllowedAccountType(accountType: string): accountType is AllowedAccountTypes {
    return ALLOWED_ACCOUNT_TYPES.includes(accountType as AccountsFormType);
}

export function ProtectAccountPage() {
    const [searchParams] = useSearchParams();
    const accountsFormType = searchParams.get('accountsFormType') || '';
    const successRedirect = searchParams.get('successRedirect') || '/tokens';
    const navigate = useNavigate();
    const { data: accounts } = useAccounts();
    const createMutation = useCreateAccountsMutation();
    const hasPasswordAccounts = useMemo(
        () => accounts && accounts.some(({ isPasswordUnlockable }) => isPasswordUnlockable),
        [accounts],
    );
    const [showVerifyPasswordView, setShowVerifyPasswordView] = useState<boolean | null>(null);
    useEffect(() => {
        if (
            typeof hasPasswordAccounts !== 'undefined' &&
            !(createMutation.isSuccess || createMutation.isPending)
        ) {
            setShowVerifyPasswordView(hasPasswordAccounts);
        }
    }, [hasPasswordAccounts, createMutation.isSuccess, createMutation.isPending]);
    const createAccountCallback = useCallback(
        async (password: string, type: AccountsFormType) => {
            try {
                const createdAccounts = await createMutation.mutateAsync({
                    type,
                    password,
                });
                if (
                    type === AccountsFormType.NewMnemonic &&
                    isMnemonicSerializedUiAccount(createdAccounts[0])
                ) {
                    navigate(`/accounts/backup/${createdAccounts[0].sourceID}`, {
                        replace: true,
                        state: {
                            onboarding: true,
                        },
                    });
                } else if (
                    REDIRECT_TO_ACCOUNTS_FINDER.includes(type) &&
                    (isMnemonicSerializedUiAccount(createdAccounts[0]) ||
                        isSeedSerializedUiAccount(createdAccounts[0]))
                ) {
                    const path = `/accounts/manage/accounts-finder/${createdAccounts[0].sourceID}`;
                    navigate(path, {
                        replace: true,
                        state: {
                            type: type,
                        },
                    });
                } else if (isLedgerAccountSerializedUI(createdAccounts[0])) {
                    const path = `/accounts/manage/accounts-finder/${AllowedAccountSourceTypes.LedgerDerived}`;
                    navigate(path, {
                        replace: true,
                        state: {
                            type: type,
                        },
                    });
                } else {
                    navigate(successRedirect, { replace: true });
                }
            } catch (e) {
                toast.error((e as Error).message ?? 'Failed to create account');
            }
        },
        [createMutation, navigate, successRedirect],
    );
    const autoLockMutation = useAutoLockMinutesMutation();
    if (!isAllowedAccountType(accountsFormType)) {
        return <Navigate to="/" replace />;
    }

    return (
        <PageTemplate
            title="Create Password"
            isTitleCentered
            showBackButton
            onClose={() => navigate(-1)}
        >
            <Loading loading={showVerifyPasswordView === null}>
                {showVerifyPasswordView ? (
                    <VerifyPasswordModal
                        open
                        onClose={() => navigate(-1)}
                        onVerify={(password) => createAccountCallback(password, accountsFormType)}
                    />
                ) : (
                    <ProtectAccountForm
                        cancelButtonText="Back"
                        submitButtonText="Create Wallet"
                        onSubmit={async ({ password, autoLock }) => {
                            await autoLockMutation.mutateAsync({
                                minutes: autoLockDataToMinutes(autoLock),
                            });
                            await createAccountCallback(password.input, accountsFormType);
                        }}
                    />
                )}
            </Loading>
        </PageTemplate>
    );
}
