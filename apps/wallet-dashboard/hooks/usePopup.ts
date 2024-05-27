// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PopupContext } from '@/contexts';
import { PopupManager } from '@/lib/interfaces';
import { useContext } from 'react';

export const usePopup = (): PopupManager | null => {
    const context = useContext(PopupContext);
    if (!context) {
        return null;
    }
    return context;
};
