// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { UnstakeDialogView } from '../enums';

export function useUnstakeDialog() {
    const [isOpen, setIsOpen] = useState(false);
    const [view, setView] = useState<UnstakeDialogView>(UnstakeDialogView.Unstake);

    function openUnstakeDialog() {
        setIsOpen(true);
    }

    return {
        isOpen,
        setIsOpen,
        view,
        setView,
        openUnstakeDialog,
    };
}
