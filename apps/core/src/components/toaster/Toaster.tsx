// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import toast, { Toaster as ToasterLib, type ToastType, resolveValue, Toast } from 'react-hot-toast';
import { Snackbar, SnackbarType } from '@iota/apps-ui-kit';
import cl from 'clsx';

export type ToasterProps = {
    bottomNavEnabled?: boolean;
    containerClassName?: string;
    snackbarWrapClassName?: string;
};

export function Toaster(props: ToasterProps) {
    function getSnackbarType(type: ToastType | 'warning'): SnackbarType {
        switch (type) {
            case 'success':
                return SnackbarType.Success;
            case 'error':
                return SnackbarType.Error;
            case 'warning':
                return SnackbarType.Warning;
            case 'loading':
                return SnackbarType.Default;
            default:
                return SnackbarType.Default;
        }
    }

    return (
        <ToasterLib
            position="bottom-right"
            containerClassName={cl('!z-[9999999]', props.containerClassName)}
        >
            {(t) => (
                <div style={{ opacity: t.visible ? 1 : 0 }} className={props.snackbarWrapClassName}>
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
    // By default for custom toasts duration is undefined, but we use 2000ms everywhere else
    const duration = options?.duration ?? 2000;

    // @ts-expect-error Type 'warning' is not assignable to type 'ToastType'.
    return toast.custom(message, { ...options, type: 'warning', duration });
};

export { enhancedToast as toast };
