import toast, { Toaster as ToasterLib, type ToastType, resolveValue } from 'react-hot-toast';
import { Snackbar, SnackbarType } from '@iota/apps-ui-kit';

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
            default:
                return SnackbarType.Default;
        }
    }

    return (
        <ToasterLib position="bottom-right" containerClassName="!z-[999999] !right-8">
            {(t) => (
                <div style={{ opacity: t.visible ? 1 : 0 }} className="w-96">
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
