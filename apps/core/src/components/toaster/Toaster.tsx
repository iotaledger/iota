// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import toast, { Toaster as ToasterLib, type ToastType, resolveValue, Toast } from 'react-hot-toast';
import { Snackbar, SnackbarType } from '@iota/apps-ui-kit';

export type ToasterProps = {
    bottomNavEnabled?: boolean;
    containerClassName?: string;
};

export function Toaster(props: ToasterProps) {
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
        <ToasterLib position="bottom-right" containerClassName={props.containerClassName}>
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

// Duplicate type because it's not exportable from the library
type ToastOptions = Partial<
    Pick<
        Toast,
        'id' | 'icon' | 'duration' | 'ariaProps' | 'className' | 'style' | 'position' | 'iconTheme'
    >
>;

const enhancedToast = toast as typeof toast & {
    warning: (message: JSX.Element | string | null, options?: ToastOptions) => string;
};

// Implement the warning function
enhancedToast.warning = (message, options) => {
    return toast.custom(
        (t) => (
            <div style={{ opacity: t.visible ? 1 : 0 }}>
                <Snackbar
                    onClose={() => toast.dismiss(t.id)}
                    text={message}
                    type={SnackbarType.Warning}
                    showClose
                    duration={t.duration}
                />
            </div>
        ),
        options,
    );
};

export { enhancedToast as toast };
