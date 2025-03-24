// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import { StakeForm } from './StakeForm';
import { UnStakeForm } from './UnstakeForm';
import { useCallback } from 'react';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { queryClient } from '../../helpers';

export function StakingCard() {
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const validatorAddress = searchParams.get('address');
    const stakeIotaIdParams = searchParams.get('staked');
    const unstake = searchParams.get('unstake') === 'true';

    const { data: system, isPending: validatorsIsPending } = useIotaClientQuery(
        'getLatestIotaSystemState',
    );

    const handleOnSuccess = useCallback(
        (response: IotaTransactionBlockResponse) => {
            // Invalidate the react query for system state and validator
            Promise.all([
                queryClient.invalidateQueries({
                    queryKey: ['system', 'state'],
                }),
                queryClient.invalidateQueries({
                    queryKey: ['delegated-stakes'],
                }),
            ]);
            navigate(
                `/receipt?${new URLSearchParams({
                    txdigest: response.digest,
                    from: 'tokens',
                }).toString()}`,
                response?.transaction
                    ? {
                          state: {
                              response,
                          },
                      }
                    : undefined,
            );
        },
        [queryClient, navigate],
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
                        onSuccess={handleOnSuccess}
                    />
                ) : (
                    <StakeForm
                        validatorAddress={validatorAddress}
                        epoch={system?.epoch}
                        onSuccess={handleOnSuccess}
                    />
                )}
            </Loading>
        </div>
    );
}
