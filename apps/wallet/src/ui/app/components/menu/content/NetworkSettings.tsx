// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NetworkSelector, Overlay } from '_components';
import { useSidebar } from '_hooks';
import { useNavigate } from 'react-router-dom';

export function NetworkSettings() {
    const navigate = useNavigate();
    const sidebar = useSidebar();
    return (
        <Overlay
            showModal
            title="Network"
            closeOverlay={() => navigate('/tokens')}
            showBackButton={!sidebar}
            useInlineLayout={sidebar}
            hideCloseIcon={sidebar}
        >
            <NetworkSelector />
        </Overlay>
    );
}
