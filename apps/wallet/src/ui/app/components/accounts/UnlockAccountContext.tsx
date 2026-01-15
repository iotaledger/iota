// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useCallback, useContext, useState, type ReactNode } from 'react';
import { toast } from '@iota/core';
import { useUnlockMutation, useBackgroundClient } from '_hooks';
import { UnlockAccountModal } from './UnlockAccountModal';

interface UnlockAccountsContextType {
    isUnlockModalOpen: boolean;
    unlockAccounts: () => void;
    lockAccounts: () => void;
    isPending: boolean;
    hideUnlockModal: () => void;
}

const UnlockAccountsContext = createContext<UnlockAccountsContextType | null>(null);

interface UnlockAccountsProviderProps {
    children: ReactNode;
}

export function UnlockAccountsProvider({ children }: UnlockAccountsProviderProps) {
    const [isUnlockModalOpen, setIsUnlockModalOpen] = useState(false);
    const unlockAccountMutation = useUnlockMutation();
    const backgroundClient = useBackgroundClient();
    const hideUnlockModal = useCallback(() => {
        setIsUnlockModalOpen(false);
    }, []);

    const unlockAccounts = useCallback(async () => {
        setIsUnlockModalOpen(true);
    }, []);

    const lockAccounts = useCallback(async () => {
        try {
            await backgroundClient.lockAllAccountsAndSources({});
            toast('Accounts locked');
        } catch (e) {
            toast.error((e as Error).message || 'Failed to lock account');
        }
    }, [backgroundClient]);

    return (
        <UnlockAccountsContext.Provider
            value={{
                isUnlockModalOpen,
                unlockAccounts,
                hideUnlockModal,
                lockAccounts,
                isPending: unlockAccountMutation.isPending,
            }}
        >
            {children}
            <UnlockAccountModal
                onClose={hideUnlockModal}
                onSuccess={hideUnlockModal}
                open={isUnlockModalOpen}
            />
        </UnlockAccountsContext.Provider>
    );
}

export function useUnlockAccounts(): UnlockAccountsContextType {
    const context = useContext(UnlockAccountsContext);
    if (!context) {
        throw new Error('useUnlockAccounts must be used within an UnlockAccountsProvider');
    }
    return context;
}
