// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useInitializedGuard } from '_src/ui/app/hooks';
import { useNavigate, useParams } from 'react-router-dom';
import Overlay from '../../../../components/overlay';
import { AccountsFinderView } from './AccountsFinderView';

export function AccountsFinderPage() {
    const { accountSourceId } = useParams();
    const navigate = useNavigate();
    const MOCKED_ACCOUNTS = [
        {
            id: '0',
            address: '0x0000000000000000000000000000',
            balance: 100,
            accountSourceId: accountSourceId,
        },
        {
            id: '1',
            address: '0x0000000000000000000000000000',
            balance: 200,
            accountSourceId: accountSourceId,
        },
        {
            id: '2',
            address: '0x0000000000000000000000000000',
            balance: 0,
            accountSourceId: accountSourceId,
        },
        {
            id: '3',
            address: '0x0000000000000000000000000000',
            balance: 50,
            accountSourceId: accountSourceId,
        },
    ];
    useInitializedGuard(true);
    return (
        <Overlay
            showModal
            title="Accounts Finder"
            closeOverlay={() => navigate('/accounts/manage')}
        >
            <AccountsFinderView accounts={MOCKED_ACCOUNTS} />
        </Overlay>
    );
}
