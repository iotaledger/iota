// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { ArrowRight12 } from '@iota/icons';
import { type IotaValidatorSummary } from '@iota/iota-sdk/client';
import { Text } from '@iota/ui';
import { type ReactNode, useMemo } from 'react';

import { HighlightedTableCol } from '~/components';
import {
    AddressLink,
    Banner,
    ImageIcon,
    Link,
    PlaceholderTable,
    TableCard,
    ValidatorLink,
} from '~/components/ui';
import { ampli } from '~/lib/utils';
import { StakeColumn } from './StakeColumn';

const NUMBER_OF_VALIDATORS = 10;

export function processValidators(set: IotaValidatorSummary[]) {
    return set.map((av) => ({
        name: av.name,
        address: av.iotaAddress,
        stake: av.stakingPoolIotaBalance,
        logo: av.imageUrl,
    }));
}
interface ValidatorData {
    name: ReactNode;
    stake: ReactNode;
    delegation: ReactNode;
    address: ReactNode;
}

interface TableColumn {
    header: string;
    accessorKey: keyof ValidatorData;
}

interface ValidatorsTableData {
    data: ValidatorData[];
    columns: TableColumn[];
}

function validatorsTable(
    validatorsData: IotaValidatorSummary[],
    limit?: number,
    showIcon?: boolean,
): ValidatorsTableData {
    const validators = processValidators(validatorsData).sort(() => (Math.random() > 0.5 ? -1 : 1));

    const validatorsItems = limit ? validators.splice(0, limit) : validators;

    return {
        data: validatorsItems.map(({ name, stake, address, logo }) => ({
            name: (
                <HighlightedTableCol first>
                    <div className="flex items-center gap-2.5">
                        {showIcon && (
                            <ImageIcon src={logo} size="sm" fallback={name} label={name} circle />
                        )}
                        <ValidatorLink
                            address={address}
                            label={name}
                            onClick={() =>
                                ampli.clickedValidatorRow({
                                    sourceFlow: 'Top validators - validator name',
                                    validatorAddress: address,
                                    validatorName: name,
                                })
                            }
                        />
                    </div>
                </HighlightedTableCol>
            ),
            stake: <StakeColumn stake={stake} />,
            delegation: (
                <Text variant="bodySmall/medium" color="steel-darker">
                    {stake.toString()}
                </Text>
            ),
            address: (
                <HighlightedTableCol>
                    <AddressLink
                        address={address}
                        noTruncate={!limit}
                        onClick={() =>
                            ampli.clickedValidatorRow({
                                sourceFlow: 'Top validators - validator address',
                                validatorAddress: address,
                                validatorName: name,
                            })
                        }
                    />
                </HighlightedTableCol>
            ),
        })),
        columns: [
            {
                header: 'Name',
                accessorKey: 'name',
            },
            {
                header: 'Address',
                accessorKey: 'address',
            },
            {
                header: 'Stake',
                accessorKey: 'stake',
            },
        ],
    };
}

type TopValidatorsCardProps = {
    limit?: number;
    showIcon?: boolean;
};

export function TopValidatorsCard({ limit, showIcon }: TopValidatorsCardProps): JSX.Element {
    const { data, isPending, isSuccess, isError } = useIotaClientQuery('getLatestIotaSystemState');

    const tableData = useMemo(
        () => (data ? validatorsTable(data.activeValidators, limit, showIcon) : null),
        [data, limit, showIcon],
    );

    if (isError || (!isPending && !tableData?.data.length)) {
        return (
            <Banner variant="error" fullWidth>
                Validator data could not be loaded
            </Banner>
        );
    }

    return (
        <>
            {isPending && (
                <PlaceholderTable
                    rowCount={limit || NUMBER_OF_VALIDATORS}
                    rowHeight="13px"
                    colHeadings={['Name', 'Address', 'Stake']}
                    colWidths={['220px', '220px', '220px']}
                />
            )}

            {isSuccess && tableData && (
                <>
                    <TableCard data={tableData.data} columns={tableData.columns} />
                    <div className="mt-3 flex justify-between">
                        <Link to="/validators">
                            <div className="flex items-center gap-2">
                                View all
                                <ArrowRight12 fill="currentColor" className="h-3 w-3 -rotate-45" />
                            </div>
                        </Link>
                        <Text variant="body/medium" color="steel-dark">
                            {data ? data.activeValidators.length : '-'}
                            {` Total`}
                        </Text>
                    </div>
                </>
            )}
        </>
    );
}
