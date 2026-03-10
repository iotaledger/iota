// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Snackbar, SnackbarType } from '@iota/apps-ui-kit';
import toast, { resolveValue, Toaster as ToasterLib, type ToastType } from 'react-hot-toast';

export type ToasterProps = {
    bottomNavEnabled?: boolean;
};

export function Toaster() {
    function getSnackbarType(type: ToastType): SnackbarType {
        switch (type) {
            case 'success':
                return SnackbarType.Success;
            case 'error':
                return SnackbarType.Error;
            case 'loading':
                return SnackbarType.Default;
            default:
                return SnackbarType.Default;
        }
    }

    return (
        <ToasterLib position="bottom-right" containerClassName="!z-[999999] toast-layer !right-8">
            {(t) => (
                <div style={{ opacity: t.visible ? 1 : 0 }}>
                    <Snackbar
                        onClose={() => toast.dismiss(t.id)}
                        text={resolveValue(t.message, t)}
                        type={getSnackbarType(t.type)}
                        showClose
                        duration={t.duration}
                    />
                </div>
            )}
        </ToasterLib>
    );
}
