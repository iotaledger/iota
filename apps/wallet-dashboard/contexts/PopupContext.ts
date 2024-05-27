// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PopupManager } from '@/lib/interfaces';
import { createContext } from 'react';

export const PopupContext = createContext<PopupManager | undefined>(undefined);
