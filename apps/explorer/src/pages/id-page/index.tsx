// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ObjectDetailsHeader } from '@iota/icons';
import { useParams } from 'react-router-dom';
import { ErrorBoundary, PageHeader, PageLayout } from '~/components';
import { PageContent } from '~/pages/id-page/PageContent';
import { ObjectView } from '../object-result/views/ObjectView';
import { TotalStaked } from '../address-result/TotalStaked';
import { useGetObject } from '@iota/core';

interface HeaderProps {
    address: string;
}

function Header({ address }: HeaderProps): JSX.Element {
    const { data, error: getObjectError } = useGetObject(address!);
    const isObject = !!data?.data;
    const errorText = getObjectError?.message;

    return (
        <div>
            <PageHeader
                error={errorText}
                type={isObject ? 'Object' : 'Address'}
                title={address}
                before={<ObjectDetailsHeader className="h-6 w-6" />}
                after={<TotalStaked address={address} />}
            />

            <ErrorBoundary>
                {data && (
                    <div className="mt-5">
                        <ObjectView data={data} />
                    </div>
                )}
            </ErrorBoundary>
        </div>
    );
}

interface PageLayoutContainerProps {
    address: string;
}

function PageLayoutContainer({ address }: PageLayoutContainerProps): JSX.Element {
    return (
        <PageLayout
            content={
                <>
                    <Header address={address} />
                    <PageContent address={address} />
                </>
            }
        />
    );
}

export function IdPage(): JSX.Element {
    const { id } = useParams();

    return <PageLayoutContainer address={id!} />;
}
