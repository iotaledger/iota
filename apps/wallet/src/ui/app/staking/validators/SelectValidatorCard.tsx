// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { calculateStakeShare, useGetValidatorsApy } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import cl from 'clsx';
import { useMemo, useState } from 'react';
import { Button, InfoBox, InfoBoxStyle, InfoBoxType, LoadingIndicator } from '@iota/apps-ui-kit';
import { useNavigate } from 'react-router-dom';
import { ValidatorLogo } from './ValidatorLogo';
import { Warning } from '@iota/apps-ui-icons';

type Validator = {
    name: string;
    address: string;
    apy: number | null;
    isApyApproxZero?: boolean;
    stakeShare: number;
};

export function SelectValidatorCard() {
    const [selectedValidator, setSelectedValidator] = useState<Validator | null>(null);

    const navigate = useNavigate();

    const { data, isPending, isError, error } = useIotaClientQuery('getLatestIotaSystemState');
    const { data: rollingAverageApys } = useGetValidatorsApy();

    const selectValidator = (validator: Validator) => {
        setSelectedValidator((state) => (state?.address !== validator.address ? validator : null));
    };

    const totalStake = useMemo(() => {
        if (!data) return 0;
        return data.activeValidators.reduce(
            (acc, curr) => (acc += BigInt(curr.stakingPoolIotaBalance)),
            0n,
        );
    }, [data]);

    const validatorsRandomOrder = useMemo(
        () => [...(data?.activeValidators || [])].sort(() => 0.5 - Math.random()),
        [data?.activeValidators],
    );
    const validatorList: Validator[] = useMemo(() => {
        const sortedAsc = validatorsRandomOrder.map((validator) => {
            const { apy, isApyApproxZero } = rollingAverageApys?.[validator.iotaAddress] ?? {
                apy: null,
            };
            return {
                name: validator.name,
                address: validator.iotaAddress,
                apy,
                isApyApproxZero,
                stakeShare: calculateStakeShare(
                    BigInt(validator.stakingPoolIotaBalance),
                    BigInt(totalStake),
                ),
            };
        });
        return sortedAsc;
    }, [validatorsRandomOrder, rollingAverageApys, totalStake]);

    if (isPending) {
        return (
            <div className="flex h-full w-full items-center justify-center p-2">
                <LoadingIndicator />
            </div>
        );
    }

    if (isError) {
        return (
            <div className="mb-2 flex h-full w-full items-center justify-center p-2">
                <InfoBox
                    type={InfoBoxType.Error}
                    title="Something went wrong"
                    supportingText={error?.message ?? 'An error occurred'}
                    icon={<Warning />}
                    style={InfoBoxStyle.Default}
                />
            </div>
        );
    }
    return (
        <div className="flex h-full w-full flex-col justify-between overflow-hidden">
            <div className="flex max-h-[530px] w-full flex-1 flex-col items-start overflow-auto">
                {data &&
                    validatorList.map((validator) => (
                        <div
                            className={cl('group relative w-full cursor-pointer', {
                                'rounded-xl bg-shader-neutral-light-8':
                                    selectedValidator?.address === validator.address,
                            })}
                            key={validator.address}
                        >
                            <ValidatorLogo
                                validatorAddress={validator.address}
                                showApy
                                onClick={() => {
                                    selectValidator(validator);
                                }}
                            />
                        </div>
                    ))}
            </div>
            {selectedValidator && (
                <Button
                    fullWidth
                    data-testid="select-validator-cta"
                    onClick={() => {
                        ampli.selectedValidator({
                            validatorName: selectedValidator.name,
                            validatorAddress: selectedValidator.address,
                            validatorAPY: selectedValidator.apy || 0,
                        });
                        navigate(
                            `/stake/new?address=${encodeURIComponent(selectedValidator.address)}`,
                        );
                    }}
                    text="Next"
                />
            )}
        </div>
    );
}
