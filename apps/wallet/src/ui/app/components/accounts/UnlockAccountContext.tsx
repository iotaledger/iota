// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/account';
import { createContext, useCallback, useContext, useState, type ReactNode, useRef } from 'react';
import { toast } from '@iota/core';
import { useUnlockMutation, useBackgroundClient } from '_hooks';
import { UnlockAccountModal } from './UnlockAccountModal';

type OnSuccessCallback = () => void | Promise<void>;

interface UnlockAccountContextType {
    isUnlockModalOpen: boolean;
    unlockAccount: (account: SerializedUIAccount, onSuccessCallback?: OnSuccessCallback) => void;
    lockAccount: (account: SerializedUIAccount) => void;
    isPending: boolean;
    hideUnlockModal: () => void;
}

const UnlockAccountContext = createContext<UnlockAccountContextType | null>(null);

interface UnlockAccountProviderProps {
    children: ReactNode;
}

export function UnlockAccountProvider({ children }: UnlockAccountProviderProps) {
    const [isUnlockModalOpen, setIsUnlockModalOpen] = useState(false);
    const onSuccessCallbackRef = useRef<OnSuccessCallback | undefined>();
    const unlockAccountMutation = useUnlockMutation();
    const backgroundClient = useBackgroundClient();
    const hideUnlockModal = useCallback(() => {
        setIsUnlockModalOpen(false);
        onSuccessCallbackRef.current && onSuccessCallbackRef.current();
    }, []);

    const unlockAccount = useCallback(
        async (account: SerializedUIAccount, onSuccessCallback?: OnSuccessCallback) => {
            if (account) {
                if (account.isPasswordUnlockable) {
                    // for password-unlockable accounts, show the unlock modal
                    setIsUnlockModalOpen(true);

                    if (onSuccessCallback) {
                        onSuccessCallbackRef.current = onSuccessCallback;
                    }
                }
            }
        },
        [unlockAccountMutation],
    );

    const lockAccount = useCallback(
        async (account: SerializedUIAccount) => {
            try {
                await backgroundClient.lockAllAccounts({ id: account.id });
                toast('Account locked');
            } catch (e) {
                toast.error((e as Error).message || 'Failed to lock account');
            }
        },
        [backgroundClient],
    );

    return (
        <UnlockAccountContext.Provider
            value={{
                isUnlockModalOpen,
                unlockAccount,
                hideUnlockModal,
                lockAccount,
                isPending: unlockAccountMutation.isPending,
            }}
        >
            {children}
            <UnlockAccountModal
                onClose={hideUnlockModal}
                onSuccess={hideUnlockModal}
                open={isUnlockModalOpen}
            />
        </UnlockAccountContext.Provider>
    );
}

export function useUnlockAccount(): UnlockAccountContextType {
    const context = useContext(UnlockAccountContext);
    if (!context) {
        throw new Error('useUnlockAccount must be used within an UnlockAccountProvider');
    }
    return context;
}
