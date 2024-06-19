// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { isMnemonicSerializedUiAccount } from '_src/background/accounts/MnemonicAccount';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { toast } from 'react-hot-toast';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';

import { ProtectAccountForm } from '../../components/accounts/ProtectAccountForm';
import { VerifyPasswordModal } from '../../components/accounts/VerifyPasswordModal';
import Loading from '../../components/loading';
import { useAccounts } from '../../hooks/useAccounts';
import { autoLockDataToMinutes } from '../../hooks/useAutoLockMinutes';
import { useAutoLockMinutesMutation } from '../../hooks/useAutoLockMinutesMutation';
import { useCreateAccountsMutation, type CreateType } from '../../hooks/useCreateAccountMutation';
import { Heading } from '../../shared/heading';

const ALLOWED_ACCOUNT_TYPES: CreateType[] = [
    'new-mnemonic',
    'import-mnemonic',
    'mnemonic-derived',
    'import-seed',
    'imported',
    'ledger',
];

const REDIRECT_TO_ACCOUNT_FINDER: CreateType[] = ['import-mnemonic', 'import-seed', 'imported'];

type AllowedAccountTypes = (typeof ALLOWED_ACCOUNT_TYPES)[number];

function isAllowedAccountType(accountType: string): accountType is AllowedAccountTypes {
    return ALLOWED_ACCOUNT_TYPES.includes(accountType as CreateType);
}

export function ProtectAccountPage() {
    const [searchParams] = useSearchParams();
    const accountType = searchParams.get('accountType') || '';
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
        async (password: string, type: CreateType) => {
            try {
                const createdAccounts = await createMutation.mutateAsync({
                    type,
                    password,
                });
                if (type === 'new-mnemonic' && isMnemonicSerializedUiAccount(createdAccounts[0])) {
                    navigate(`/accounts/backup/${createdAccounts[0].sourceID}`, {
                        replace: true,
                        state: {
                            onboarding: true,
                        },
                    });
                } else if (REDIRECT_TO_ACCOUNT_FINDER.includes(type)) {
                    // const path = '/accounts/accounts-finder/';
                    const path = '/accounts/manage/accounts-finder/';
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
    if (!isAllowedAccountType(accountType)) {
        return <Navigate to="/" replace />;
    }

    return (
        <div className="flex h-screen max-h-popup-height min-h-popup-minimum w-popup-width flex-col items-center overflow-auto rounded-20 bg-iota-lightest px-6 py-10 shadow-wallet-content">
            <Loading loading={showVerifyPasswordView === null}>
                {showVerifyPasswordView ? (
                    <VerifyPasswordModal
                        open
                        onClose={() => navigate(-1)}
                        onVerify={(password) => createAccountCallback(password, accountType)}
                    />
                ) : (
                    <>
                        <Text variant="caption" color="steel-dark" weight="semibold">
                            Wallet Setup
                        </Text>
                        <div className="mt-2.5 text-center">
                            <Heading variant="heading1" color="gray-90" as="h1" weight="bold">
                                Protect Account with a Password Lock
                            </Heading>
                        </div>
                        <div className="mt-6 w-full grow">
                            <ProtectAccountForm
                                cancelButtonText="Back"
                                submitButtonText="Create Wallet"
                                onSubmit={async ({ password, autoLock }) => {
                                    await autoLockMutation.mutateAsync({
                                        minutes: autoLockDataToMinutes(autoLock),
                                    });
                                    await createAccountCallback(password.input, accountType);
                                }}
                            />
                        </div>
                    </>
                )}
            </Loading>
        </div>
    );
}
