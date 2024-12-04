// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMenuIsOpen } from '_components';
import { useAppSelector } from '_hooks';
import { getNavIsVisible } from '_redux/slices/app';
import cl from 'clsx';
import toast, {
    Toaster as ToasterLib,
    type ToastType,
    resolveValue,
    useToasterStore,
} from 'react-hot-toast';
import { useLocation } from 'react-router-dom';
import { Portal } from '../Portal';
import { Snackbar, SnackbarType } from '@iota/apps-ui-kit';
import { useEffect } from 'react';

export type ToasterProps = {
    bottomNavEnabled?: boolean;
};

const LIMIT_MAX_TOASTS = 5;

function getBottomSpace(pathname: string, isMenuVisible: boolean, isBottomNavSpace: boolean) {
    if (isMenuVisible) {
        return '!bottom-28';
    }

    const overlayWithActionButton = [
        '/auto-lock',
        '/manage/accounts-finder',
        '/accounts/import-ledger-accounts',
        '/send',
        '/accounts/forgot-password/recover-many',
        '/accounts/manage',
    ].includes(pathname);

    const matchDynamicPaths = ['/dapp/connect'].some((path) => pathname.startsWith(path));

    if (overlayWithActionButton || isBottomNavSpace || matchDynamicPaths) {
        return '!bottom-20';
    }

    return '';
}

export function Toaster({ bottomNavEnabled = false }: ToasterProps) {
    const { pathname } = useLocation();

    const menuVisible = useMenuIsOpen();
    const isBottomNavVisible = useAppSelector(getNavIsVisible);
    const bottomSpace = getBottomSpace(
        pathname,
        menuVisible,
        isBottomNavVisible && bottomNavEnabled,
    );

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
    const { toasts } = useToasterStore();

    useEffect(() => {
        toasts
            .filter((t) => t.visible)
            .filter((_, i) => i >= LIMIT_MAX_TOASTS)
            .forEach((t) => toast.dismiss(t.id));
    }, [toasts]);

    return (
        <Portal containerId="toaster-portal-container">
            <ToasterLib
                containerClassName={cl('!absolute !z-[99999] transition-all', bottomSpace)}
                position="bottom-right"
            >
                {(t) => (
                    <div style={{ opacity: t.visible ? 1 : 0 }} className="w-full">
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
