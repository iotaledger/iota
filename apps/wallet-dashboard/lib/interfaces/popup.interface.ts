// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ReactNode } from 'react';

export interface PopupManager {
    popups: ReactNode[];
    openPopup: (content: ReactNode) => void;
    closePopup: () => void;
}
