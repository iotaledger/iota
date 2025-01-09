// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';

import { useAccountSources } from '../../../hooks/useAccountSources';
import { AccountSourceType } from '_src/background/account-sources/accountSource';

export function ForgotPasswordIndexPage() {
    const allAccountSources = useAccountSources();
    const navigate = useNavigate();
    const totalRecoverable =
        allAccountSources.data?.filter(
            ({ type }) => type === AccountSourceType.Mnemonic || type === AccountSourceType.Seed,
        ).length || 0;
    useEffect(() => {
        if (allAccountSources.isPending) {
            return;
        }
        const url =
            totalRecoverable === 0 ? '/' : totalRecoverable === 1 ? './recover' : './recover-many';
        navigate(url, { replace: true });
    }, [allAccountSources.isPending, totalRecoverable, navigate]);
    return null;
}
