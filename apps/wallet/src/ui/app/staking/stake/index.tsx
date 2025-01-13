// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Overlay } from '_components';
import { useNavigate, useSearchParams } from 'react-router-dom';

import { SelectValidatorCard } from '../validators/SelectValidatorCard';
import { StakingCard } from './StakingCard';

export function StakePage() {
    const [searchParams] = useSearchParams();
    const validatorAddress = searchParams.get('address');
    const unstake = searchParams.get('unstake') === 'true';

    const navigate = useNavigate();
    const stakingTitle = unstake ? 'Unstake' : 'Stake IOTA';

    return (
        <Overlay
            showModal={true}
            title={validatorAddress ? stakingTitle : 'Select a Validator'}
            closeOverlay={() => navigate('/')}
            showBackButton
        >
            {validatorAddress ? <StakingCard /> : <SelectValidatorCard />}
        </Overlay>
    );
}
