// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaObjectResponse } from '@iota/iota-sdk/client';
import { Placeholder } from '@iota/ui';
import { useMemo } from 'react';
import cx from 'clsx';
import { TableCard } from '~/components/ui';
import { generateObjectListColumns } from '~/lib/ui/utils/generateObjectListColumns';

function ListViewItemPlaceholder(): JSX.Element {
    return (
        <div
            className={cx(
                'flex items-center justify-around overflow-hidden',
                '[&_td]:flex [&_td]:items-center',
                'group mb-2 justify-between rounded-lg p-1 hover:bg-hero/5',
            )}
        >
            <div
                className={cx(
                    'w-3/12 basis-3/12',
                    '[&_td]:flex [&_td]:items-center',
                    'flex max-w-[66%] basis-8/12 items-center gap-3 md:max-w-[25%] md:basis-3/12 md:pr-5',
                )}
            >
                <Placeholder rounded="lg" width="540px" height="20px" />
            </div>

            <div
                className={cx(
                    'w-6/12 basis-6/12 overflow-hidden [&_td]:flex',
                    'hidden max-w-[50%] pr-5 md:flex',
                )}
            >
                <Placeholder rounded="lg" width="540px" height="20px" />
            </div>

            <div
                className={cx(
                    'w-3/12 basis-3/12',
                    '[&_td]:flex [&_td]:items-center',
                    'flex max-w-[34%]',
                )}
            >
                <Placeholder rounded="lg" width="540px" height="20px" />
            </div>
        </div>
    );
}

interface ListViewProps {
    data?: IotaObjectResponse[];
    loading?: boolean;
}

export function ListView({ data, loading }: ListViewProps): JSX.Element {
    const tableColumns = useMemo(() => {
        if (!data) return null;
        return generateObjectListColumns();
    }, [data]);

    return (
        <div className="flex flex-col overflow-auto">
            {tableColumns && data && <TableCard data={data ?? []} columns={tableColumns} />}
            {loading &&
                new Array(10).fill(0).map((_, index) => <ListViewItemPlaceholder key={index} />)}
        </div>
    );
}
