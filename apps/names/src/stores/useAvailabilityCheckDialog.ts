// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { create } from 'zustand';

import type { AvailabilityCheck } from '@/components/availability-check/AvailabilityCheck';

type DialogProps = React.ComponentProps<typeof AvailabilityCheck>;
type UseAvailabilityCheckDialog = {
    isOpen: boolean;
    open: (props?: DialogProps) => void;
    close: () => void;
    props: DialogProps;
};

export const useAvailabilityCheckDialog = create<UseAvailabilityCheckDialog>((set) => ({
    isOpen: false,
    open: (props) => set({ isOpen: true, props }),
    close: () => set({ isOpen: false, props: {} }),
    props: {},
}));
