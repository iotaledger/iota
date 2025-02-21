// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading, Overlay } from '_components';
import {
    useGetDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { useNavigate } from 'react-router-dom';
import { useActiveAddress } from '_hooks';
import { SelectValidatorCard } from './SelectValidatorCard';
import { ValidatorsCard } from './ValidatorsCard';
import { InfoBox, InfoBoxType, InfoBoxStyle } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';

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
            <Loading loading={isPending}>
                <div className="flex min-h-full flex-1 flex-col">
                    {isError ? (
                        <div className="mb-2">
                            <InfoBox
                                type={InfoBoxType.Error}
                                title={error?.message}
                                icon={<Warning />}
                                style={InfoBoxStyle.Elevated}
                            />
                        </div>
                    ) : null}

                    {stakedValidators?.length ? <ValidatorsCard /> : <SelectValidatorCard />}
                </div>
            </Loading>
        </Overlay>
    );
}
