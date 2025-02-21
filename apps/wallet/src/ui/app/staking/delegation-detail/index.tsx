// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Overlay } from '_components';
import { useGetDelegatedStake } from '@iota/core';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import { useActiveAddress } from '_hooks';
import { DelegationDetailCard } from './DelegationDetailCard';
import { LoadingIndicator } from '@iota/apps-ui-kit';

export function DelegationDetail() {
    const [searchParams] = useSearchParams();
    const validatorAddressParams = searchParams.get('validator');
    const stakeIdParams = searchParams.get('staked');
    const navigate = useNavigate();
    const accountAddress = useActiveAddress();
    const { isPending } = useGetDelegatedStake({
        address: accountAddress || '',
    });

    if (!validatorAddressParams || !stakeIdParams) {
        return <Navigate to="/stake" replace={true} />;
    }

    if (isPending) {
        return (
            <div className="flex h-full w-full items-center justify-center p-2">
                <LoadingIndicator />
            </div>
        );
    }

    return (
        <Overlay showBackButton showModal title="Stake Details" closeOverlay={() => navigate('/')}>
            <DelegationDetailCard
                validatorAddress={validatorAddressParams}
                stakedId={stakeIdParams}
            />
        </Overlay>
    );
}
