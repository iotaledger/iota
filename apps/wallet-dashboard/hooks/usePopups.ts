// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PopupContext, PopupManager } from '@/contexts';
import { useContext } from 'react';

export const usePopups = (): PopupManager => {
    return useContext(PopupContext);
};
