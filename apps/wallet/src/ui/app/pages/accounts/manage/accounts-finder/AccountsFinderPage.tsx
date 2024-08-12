// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useInitializedGuard } from '_src/ui/app/hooks';
import { useNavigate } from 'react-router-dom';
import Overlay from '_components/overlay';
import { AccountsFinderView } from './AccountsFinderView';

export function AccountsFinderPage() {
    const navigate = useNavigate();
    useInitializedGuard(true);

    return (
        <Overlay
            showModal
            title="Accounts Finder"
            closeOverlay={() => navigate('/accounts/manage')}
        >
            <AccountsFinderView />
        </Overlay>
    );
}
