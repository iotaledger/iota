// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';

import { NameDialogId } from '@/components/dialogs/enums';

interface UseNameManageDialogReturn {
    openDialogId: NameDialogId | null;
    openDialog: (dialogId: NameDialogId) => void;
    closeDialog: () => void;
    isDialogOpen: (dialogId: NameDialogId) => boolean;
}

export function useNameManageDialog(): UseNameManageDialogReturn {
    const [openDialogId, setOpenDialogId] = useState<NameDialogId | null>(null);

    const isOpen = (dialog: NameDialogId) => openDialogId === dialog;
    const close = () => setOpenDialogId(null);

    return {
        openDialogId: openDialogId,
        openDialog: (dialog: NameDialogId) => setOpenDialogId(dialog),
        closeDialog: close,
        isDialogOpen: isOpen,
    };
}
