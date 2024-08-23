// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNextMenuUrl, NetworkSelector, Overlay } from '_components';
import { useNavigate } from 'react-router-dom';

export function NetworkSettings() {
    const navigate = useNavigate();
    const mainMenuUrl = useNextMenuUrl(true, '/');
    return (
        <Overlay showModal title="Network" closeOverlay={() => navigate(mainMenuUrl)}>
            <NetworkSelector />
        </Overlay>
    );
}
