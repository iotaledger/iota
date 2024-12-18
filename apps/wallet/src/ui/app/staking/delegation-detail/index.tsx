// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import LoadingIndicator from '_components/loading/LoadingIndicator';
import Overlay from '_components/overlay';
import { useGetDelegatedStake } from '@iota/core';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';

import { useActiveAddress } from '../../hooks/useActiveAddress';
import { getDelegationDataByStakeId } from '../getDelegationByStakeId';
import { ValidatorLogo } from '../validators/ValidatorLogo';
import { DelegationDetailCard } from './DelegationDetailCard';

export function DelegationDetail() {
    const [searchParams] = useSearchParams();
    const validatorAddressParams = searchParams.get('validator');
    const stakeIdParams = searchParams.get('staked');
    const navigate = useNavigate();
    const accountAddress = useActiveAddress();
    const { data, isPending } = useGetDelegatedStake({
        address: accountAddress || '',
    });

    if (!validatorAddressParams || !stakeIdParams) {
        return <Navigate to={'/stake'} replace={true} />;
    }

    if (isPending) {
        return (
            <div className="flex h-full w-full items-center justify-center p-2">
                <LoadingIndicator />
            </div>
        );
    }

    const delegationData = data ? getDelegationDataByStakeId(data, stakeIdParams) : null;
    return (
        <Overlay
            showModal
            title={
                <div className="flex max-w-full items-center px-4">
                    <ValidatorLogo
                        validatorAddress={validatorAddressParams}
                        isTitle
                        iconSize="sm"
                        size="body"
                        activeEpoch={delegationData?.stakeRequestEpoch}
                    />
                </div>
            }
            closeOverlay={() => navigate('/')}
        >
            <DelegationDetailCard
                validatorAddress={validatorAddressParams}
                stakedId={stakeIdParams}
            />
        </Overlay>
    );
}
