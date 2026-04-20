// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NetworkSelector, Overlay } from '_components';
import { useAppSelector } from '_hooks';
import { ExtensionViewType } from '_src/ui/app/redux/slices/app/appType';
import { useNavigate } from 'react-router-dom';

export function NetworkSettings() {
    const navigate = useNavigate();
    const extensionViewType = useAppSelector((state) => state.app.extensionViewType);
    const useSidebar =
        extensionViewType === ExtensionViewType.FullScreen ||
        extensionViewType === ExtensionViewType.Popup;
    return (
        <Overlay
            showModal
            title="Network"
            closeOverlay={() => navigate('/tokens')}
            showBackButton={!useSidebar}
            useInlineLayout={useSidebar}
            hideCloseIcon={useSidebar}
        >
            <NetworkSelector />
        </Overlay>
    );
}
