// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { create } from 'zustand';

interface BridgeState {
    isDepositAddressManualInput: boolean;
    toggleIsDepositAddressManualInput: () => void;
}

export const useBridgeStore = create<BridgeState>((set, get) => {
    return {
        isDepositAddressManualInput: false,
        toggleIsDepositAddressManualInput: () => {
            set({ isDepositAddressManualInput: !get().isDepositAddressManualInput });
        },
    };
});
