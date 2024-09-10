// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    isIotaNSName,
    useGetObject,
    useResolveIotaNSAddress,
    useResolveIotaNSName,
} from '@iota/core';
import { ObjectDetailsHeader } from '@iota/icons';
import { useParams } from 'react-router-dom';
import { ErrorBoundary, PageLayout } from '~/components';
import { TotalStaked } from '~/pages/address-result/TotalStaked';
import { PageContent } from '~/pages/id-page/PageContent';
import { ObjectView } from '~/pages/object-result/views/ObjectView';
import { PageHeader } from '~/components/ui';

interface HeaderProps {
    address: string;
    loading?: boolean;
    error?: Error | null;
}

function Header({ address, loading, error }: HeaderProps): JSX.Element {
    const {
        data: domainName,
        isLoading,
        error: resolveIotansError,
    } = useResolveIotaNSName(address);
    const { data, isPending, error: getObjectError } = useGetObject(address!);
    const isObject = !!data?.data;
    const errorText = getObjectError?.message ?? resolveIotansError?.message ?? error?.message;

    return (
        <div>
            <PageHeader
                error={errorText}
                loading={loading || isLoading || isPending}
                type={isObject ? 'Object' : 'Address'}
                title={address}
                subtitle={domainName}
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
    const { id } = useParams();
    const isIotaNSAddress = isIotaNSName(id!);
    const {
        data,
        isLoading,
        error: iotansAddressError,
    } = useResolveIotaNSAddress(address, isIotaNSAddress);

    return (
        <PageLayout
            loading={isLoading}
            content={
                <>
                    <Header address={address} />
                    <PageContent address={data || address} error={iotansAddressError} />
                </>
            }
        />
    );
}

export function IdPage(): JSX.Element {
    const { id } = useParams();

    return <PageLayoutContainer address={id!} />;
}
