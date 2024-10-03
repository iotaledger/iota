// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Alert, Loading, Overlay } from '_components';
import {
    useGetDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { useNavigate } from 'react-router-dom';

import { useActiveAddress } from '../../hooks/useActiveAddress';
import { SelectValidatorCard } from './SelectValidatorCard';
import { ValidatorsCard } from './ValidatorsCard';

export function Validators() {
    const accountAddress = useActiveAddress();
    const {
        data: stakedValidators,
        isPending,
        isError,
        error,
    } = useGetDelegatedStake({
        address: accountAddress || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const navigate = useNavigate();

    const pageTitle = stakedValidators?.length ? 'Stake & Earn IOTA' : 'Select a Validator';

    return (
        <Overlay
            showModal
            showBackButton
            title={isPending ? 'Loading' : pageTitle}
            closeOverlay={() => navigate('/')}
        >
            <div className="flex min-h-full flex-1 flex-col">
                <Loading loading={isPending}>
                    {isError ? (
                        <div className="mb-2">
                            <Alert title={error?.message} />
                        </div>
                    ) : null}

                    {stakedValidators?.length ? <ValidatorsCard /> : <SelectValidatorCard />}
                </Loading>
            </div>
        </Overlay>
    );
}
