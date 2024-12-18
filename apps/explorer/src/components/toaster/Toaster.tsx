// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import toast, { Toaster as ToasterLib, type ToastType, resolveValue } from 'react-hot-toast';
import { Snackbar, SnackbarType } from '@iota/apps-ui-kit';

export type ToasterProps = {
    bottomNavEnabled?: boolean;
};

export function Toaster() {
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
        <ToasterLib position="bottom-right">
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
