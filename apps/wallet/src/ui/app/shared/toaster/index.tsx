// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMenuIsOpen } from '_components';
import { useAppSelector } from '_hooks';
import { getNavIsVisible } from '_redux/slices/app';
import cl from 'clsx';
import toast, { Toaster as ToasterLib, type ToastType, resolveValue } from 'react-hot-toast';
import { useLocation } from 'react-router-dom';
import { Portal } from '../Portal';
import { Snackbar, SnackbarType } from '@iota/apps-ui-kit';

export type ToasterProps = {
    bottomNavEnabled?: boolean;
};

export function Toaster({ bottomNavEnabled = false }: ToasterProps) {
    const { pathname } = useLocation();
    const isExtraNavTabsVisible = ['/apps', '/nfts'].includes(pathname);
    const menuVisible = useMenuIsOpen();
    const isBottomNavVisible = useAppSelector(getNavIsVisible);
    const includeBottomNavSpace = !menuVisible && isBottomNavVisible && bottomNavEnabled;
    const includeExtraBottomNavSpace = includeBottomNavSpace && isExtraNavTabsVisible;

    function getSnackbarType(type: ToastType): SnackbarType {
        switch (type) {
            case 'success':
                return SnackbarType.Default;
            case 'error':
                return SnackbarType.Error;
            case 'loading':
                return SnackbarType.Default;
            default:
                return SnackbarType.Default;
        }
    }

    return (
        <Portal containerId="toaster-portal-container">
            <ToasterLib
                containerClassName={cl(
                    '!absolute !z-[99999] transition-all',
                    includeBottomNavSpace && 'mb-nav-height',
                    includeExtraBottomNavSpace && '!bottom-10',
                )}
                position="bottom-right"
            >
                {(t) => (
                    <div style={{ opacity: t.visible ? 1 : 0 }}>
                        <Snackbar
                            onClose={() => toast.dismiss(t.id)}
                            text={resolveValue(t.message, t)}
                            type={getSnackbarType(t.type)}
                            showClose={true}
                            duration={t.duration}
                        />
                    </div>
                )}
            </ToasterLib>
        </Portal>
    );
}
