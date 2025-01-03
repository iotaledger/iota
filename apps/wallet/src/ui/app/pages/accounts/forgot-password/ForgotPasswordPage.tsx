// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PasswordRecoveryData } from '_src/shared/messaging/messages/payloads/methodPayload';
import { createContext, useCallback, useContext, useState } from 'react';
import { Outlet } from 'react-router-dom';

const forgotPasswordContext = createContext<{
    value: PasswordRecoveryData[];
    add: (data: PasswordRecoveryData) => void;
    clear: () => void;
} | null>(null);

export function useForgotPasswordContext() {
    const context = useContext(forgotPasswordContext);
    if (!context) {
        throw new Error('Missing forgot password context');
    }
    return context;
}

export function ForgotPasswordPage() {
    const [recoveryData, setRecoveryData] = useState<PasswordRecoveryData[]>([]);
    const add = useCallback((data: PasswordRecoveryData) => {
        setRecoveryData((existing) => [
            ...existing.filter(({ accountSourceID }) => accountSourceID !== data.accountSourceID),
            data,
        ]);
    }, []);
    const clear = useCallback(() => {
        setRecoveryData([]);
    }, []);
    return (
        <forgotPasswordContext.Provider value={{ value: recoveryData, add, clear }}>
            <Outlet />
        </forgotPasswordContext.Provider>
    );
}
