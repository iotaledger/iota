// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ComponentProps, useState } from 'react';
import { UnstakeDialogView } from '../enums';
import type { UnstakeDialog } from '../UnstakeDialog';

export function useUnstakeDialog() {
    const [isOpen, setIsOpen] = useState(false);
    const [view, setView] = useState<UnstakeDialogView>(UnstakeDialogView.Unstake);
    const [txDigest, setTxDigest] = useState<string | null>(null);

    function openUnstakeDialog(view?: UnstakeDialogView) {
        setIsOpen(true);
        if (view) {
            setView(view);
        }
    }

    function handleClose() {
        setIsOpen(false);
    }

    const defaultDialogProps: Omit<ComponentProps<typeof UnstakeDialog>, 'onSuccess'> = {
        view,
        handleClose: () => setIsOpen(false),
        txDigest,
    };

    return {
        isOpen,
        setIsOpen,
        view,
        setView,
        openUnstakeDialog,
        txDigest,
        setTxDigest,
        defaultDialogProps,
        handleClose,
    };
}
