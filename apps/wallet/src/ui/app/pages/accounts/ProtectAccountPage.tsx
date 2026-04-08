// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isMnemonicSerializedUiAccount } from '_src/background/accounts/mnemonicAccount';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import {
    ProtectAccountForm,
    VerifyPasswordModal,
    Loading,
    AccountsFormType,
    PageTemplate,
    type ProtectAccountFormValues,
} from '_components';
import {
    useAccounts,
    autoLockDataToMinutes,
    useAutoLockMinutesMutation,
    useCreateAccountsMutation,
} from '_hooks';
import { isSeedSerializedUiAccount } from '_src/background/accounts/seedAccount';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import { useFeature } from '@growthbook/growthbook-react';
import { Feature, toast } from '@iota/core';
import { isPasskeyAccountSerializedUI } from '_src/background/accounts/passkeyAccount';
import { trackAutoLockUpdated } from '_src/shared/analytics/helpers';

const ALLOWED_ACCOUNT_TYPES: AccountsFormType[] = [
    AccountsFormType.NewMnemonic,
    AccountsFormType.ImportMnemonic,
    AccountsFormType.ImportSeed,
    AccountsFormType.MnemonicSource,
    AccountsFormType.SeedSource,
    AccountsFormType.ImportPrivateKey,
    AccountsFormType.Passkey,
    AccountsFormType.ImportPasskey,
    AccountsFormType.ImportLedger,
    AccountsFormType.ImportKeystone,
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
    const accountsFormType = (searchParams.get('accountsFormType') as AccountsFormType) || '';
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

    const featureAccountFinderEnabled = useFeature<boolean>(Feature.AccountFinder).value;

    const createAccountCallback = useCallback(
        async (password: string, autoLockToTrack?: ProtectAccountFormValues['autoLock']) => {
            try {
                const createdAccounts = await createMutation.mutateAsync({
                    type: accountsFormType,
                    password,
                });
                if (autoLockToTrack) {
                    trackAutoLockUpdated(autoLockToTrack);
                }

                if (
                    accountsFormType === AccountsFormType.NewMnemonic &&
                    isMnemonicSerializedUiAccount(createdAccounts[0])
                ) {
                    navigate(`/accounts/backup/${createdAccounts[0].sourceID}`, {
                        replace: true,
                        state: {
                            onboarding: true,
                        },
                    });
                } else if (
                    featureAccountFinderEnabled &&
                    REDIRECT_TO_ACCOUNTS_FINDER.includes(accountsFormType) &&
                    (isMnemonicSerializedUiAccount(createdAccounts[0]) ||
                        isSeedSerializedUiAccount(createdAccounts[0]))
                ) {
                    const path = '/accounts/manage/accounts-finder/intro';
                    navigate(path, {
                        replace: true,
                        state: {
                            type: accountsFormType,
                        },
                    });
                } else if (
                    featureAccountFinderEnabled &&
                    isLedgerAccountSerializedUI(createdAccounts[0])
                ) {
                    const path = '/accounts/manage/accounts-finder/intro';
                    navigate(path, {
                        replace: true,
                        state: {
                            type: accountsFormType,
                        },
                    });
                } else if (
                    accountsFormType === AccountsFormType.ImportPasskey &&
                    isPasskeyAccountSerializedUI(createdAccounts[0])
                ) {
                    const url = `/accounts/import-passkey?accountID=${createdAccounts[0].id}`;
                    navigate(url, {
                        replace: true,
                        state: {
                            type: accountsFormType,
                        },
                    });
                } else {
                    navigate(successRedirect, { replace: true });
                }
            } catch (e) {
                toast.error((e as Error).message ?? 'Failed to create account');
            }
        },
        [featureAccountFinderEnabled, createMutation, navigate, successRedirect],
    );
    const autoLockMutation = useAutoLockMinutesMutation();
    if (!isAllowedAccountType(accountsFormType)) {
        return <Navigate to="/" replace />;
    }

    async function handleOnSubmit({ password, autoLock }: ProtectAccountFormValues) {
        try {
            const minutes = autoLockDataToMinutes(autoLock);
            const hasAutoLock = typeof minutes === 'number' && minutes > 0;

            if (hasAutoLock) {
                await autoLockMutation.mutateAsync({ minutes });
            }

            await createAccountCallback(password.input, hasAutoLock ? autoLock : undefined);
        } catch (e) {
            toast.error((e as Error)?.message || 'Something went wrong');
        }
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
                        onVerify={async (password) => {
                            await createAccountCallback(password);
                        }}
                    />
                ) : (
                    <ProtectAccountForm
                        cancelButtonText="Back"
                        submitButtonText="Create Wallet"
                        onSubmit={handleOnSubmit}
                    />
                )}
            </Loading>
        </PageTemplate>
    );
}
