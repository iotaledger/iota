// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ErrorBoundary,
    MainLocationContext,
    useMenuIsOpen,
    useMenuUrl,
    useNextMenuUrl,
} from '_components';
import { useOnKeyboardEvent } from '_hooks';
import { useCallback } from 'react';
import type { MouseEvent } from 'react';
import { Navigate, Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import { AutoLockAccounts } from './AutoLockAccounts';
import { NetworkSettings } from './NetworkSettings';
import { MenuList } from './WalletSettingsMenuList';
import { ThemeSettings } from './ThemeSettings';

const CLOSE_KEY_CODES: string[] = ['Escape'];

export function MenuContent() {
    const mainLocation = useLocation();
    const isOpen = useMenuIsOpen();
    const menuUrl = useMenuUrl();
    const menuHomeUrl = useNextMenuUrl(true, '/');
    const closeMenuUrl = useNextMenuUrl(false);
    const navigate = useNavigate();
    const handleOnCloseMenu = useCallback(
        (e: KeyboardEvent | MouseEvent<HTMLDivElement>) => {
            if (isOpen) {
                e.preventDefault();
                navigate(closeMenuUrl);
            }
        },
        [isOpen, navigate, closeMenuUrl],
    );

    useOnKeyboardEvent('keydown', CLOSE_KEY_CODES, handleOnCloseMenu, isOpen);
    if (!isOpen) {
        return null;
    }

    return (
        <div className="absolute inset-0 z-50 flex flex-col justify-items-stretch overflow-y-auto rounded-t-xl bg-neutral-100 px-2.5 pb-8 dark:bg-neutral-6">
            <ErrorBoundary>
                <MainLocationContext.Provider value={mainLocation}>
                    <Routes location={menuUrl || ''}>
                        <Route path="/" element={<MenuList />} />
                        <Route path="/network" element={<NetworkSettings />} />
                        <Route path="/auto-lock" element={<AutoLockAccounts />} />
                        <Route path="/theme" element={<ThemeSettings />} />
                        <Route path="*" element={<Navigate to={menuHomeUrl} replace={true} />} />
                    </Routes>
                </MainLocationContext.Provider>
            </ErrorBoundary>
        </div>
    );
}
