// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createPortal } from 'react-dom';

interface PortalProps {
    children: React.ReactNode;
    containerId: string;
}

export function Portal({ children, containerId }: PortalProps) {
    return createPortal(children, document.getElementById(containerId)!);
}
