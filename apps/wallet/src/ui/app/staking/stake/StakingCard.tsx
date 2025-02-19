// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Navigate, useSearchParams } from 'react-router-dom';
import { StakeForm } from './StakeForm';
import { UnStakeForm } from './UnstakeForm';

export function StakingCard() {
    const [searchParams] = useSearchParams();
    const validatorAddress = searchParams.get('address');
    const stakeIotaIdParams = searchParams.get('staked');
    const unstake = searchParams.get('unstake') === 'true';

    const { data: system, isPending: validatorsIsPending } = useIotaClientQuery(
        'getLatestIotaSystemState',
    );

    if (!validatorAddress || (!validatorsIsPending && !system)) {
        return <Navigate to="/" replace={true} />;
    }
    return (
        <div className="flex h-full w-full flex-grow flex-col flex-nowrap">
            <Loading loading={validatorsIsPending}>
                {unstake ? (
                    <UnStakeForm
                        stakedIotaId={stakeIotaIdParams!}
                        validatorAddress={validatorAddress}
                        epoch={Number(system?.epoch || 0)}
                    />
                ) : (
                    <StakeForm validatorAddress={validatorAddress} epoch={system?.epoch} />
                )}
            </Loading>
        </div>
    );
}
