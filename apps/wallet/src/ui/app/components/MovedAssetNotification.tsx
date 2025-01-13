// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Toast, toast } from 'react-hot-toast';
import { ButtonUnstyled } from '@iota/apps-ui-kit';

interface MovedAssetNotificationProps {
    t: Toast;
    destination: string;
    onUndo: () => void;
}

export function MovedAssetNotification({ t, destination, onUndo }: MovedAssetNotificationProps) {
    return (
        <div
            className="flex w-full flex-row items-baseline gap-x-xxs"
            onClick={() => toast.dismiss(t.id)}
        >
            <ButtonUnstyled className="text-body-sm text-neutral-12 dark:text-neutral-92">
                Moved to {destination}
            </ButtonUnstyled>
            <ButtonUnstyled
                onClick={() => {
                    onUndo();
                    toast.dismiss(t.id);
                }}
                className="ml-auto mr-sm text-body-sm text-neutral-12 dark:text-neutral-92"
            >
                UNDO
            </ButtonUnstyled>
        </div>
    );
}
