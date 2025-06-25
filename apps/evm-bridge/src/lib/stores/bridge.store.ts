// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { create } from 'zustand';

interface BridgeState {
    isFromLayer1: boolean;
    toggleBridgeDirection: () => void;
    isDepositAddressManualInput: boolean;
    toggleIsDepositAddressManualInput: () => void;
}

export const useBridgeStore = create<BridgeState>((set, get) => {
    return {
        isFromLayer1: true,
        toggleBridgeDirection: () => {
            set({
                isFromLayer1: !get().isFromLayer1,
            });
        },

        isDepositAddressManualInput: false,
        toggleIsDepositAddressManualInput: () => {
            set({ isDepositAddressManualInput: !get().isDepositAddressManualInput });
        },
    };
});
