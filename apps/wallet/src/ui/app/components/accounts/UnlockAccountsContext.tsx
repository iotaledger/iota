// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createContext,
    useCallback,
    useContext,
    useState,
    useEffect,
    type ReactNode,
    useRef,
} from 'react';
import { toast } from '@iota/core';
import { useBackgroundClient, useActiveAccount } from '_hooks';
import { UnlockAccountModal } from './UnlockAccountModal';

interface UnlockAccountsContextType {
    isUnlockModalOpen: boolean;
    unlockAccounts: () => void;
    lockAccounts: () => void;
    hideUnlockModal: () => void;
}

const UnlockAccountsContext = createContext<UnlockAccountsContextType | null>(null);

interface UnlockAccountsProviderProps {
    children: ReactNode;
}

export function UnlockAccountsProvider({ children }: UnlockAccountsProviderProps) {
    const [isUnlockModalOpen, setIsUnlockModalOpen] = useState(false);
    const backgroundClient = useBackgroundClient();
    const activeAccount = useActiveAccount();
    const isUnlockingRef = useRef(false);

    // Automatically show the modal when the active account is locked
    useEffect(() => {
        if (activeAccount?.isLocked && !isUnlockingRef.current) {
            setIsUnlockModalOpen(true);
        } else if (!activeAccount?.isLocked && isUnlockModalOpen && !isUnlockingRef.current) {
            setIsUnlockModalOpen(false);
        }
    }, [activeAccount?.isLocked, isUnlockModalOpen]);

    const hideUnlockModal = useCallback(() => {
        // Allow only hiding the modal if the account is not locked
        if (!activeAccount?.isLocked) {
            setIsUnlockModalOpen(false);
        }
    }, [activeAccount?.isLocked]);

    const unlockAccounts = useCallback(async () => {
        setIsUnlockModalOpen(true);
    }, []);

    const lockAccounts = useCallback(async () => {
        try {
            await backgroundClient.lockAllAccountsAndSources({});
            toast('Wallet locked');
        } catch (e) {
            toast.error((e as Error).message || 'Failed to lock account');
        }
    }, [backgroundClient]);

    const handleUnlockSuccess = useCallback(() => {
        isUnlockingRef.current = true;
        setIsUnlockModalOpen(false);
        setTimeout(() => {
            isUnlockingRef.current = false;
        }, 100);
    }, []);

    return (
        <UnlockAccountsContext.Provider
            value={{
                isUnlockModalOpen,
                unlockAccounts,
                hideUnlockModal,
                lockAccounts,
            }}
        >
            {children}
            <UnlockAccountModal
                onClose={hideUnlockModal}
                onSuccess={handleUnlockSuccess}
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
