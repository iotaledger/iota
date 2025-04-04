// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetObjectOrPastObject } from '@iota/core';
import { useParams } from 'react-router-dom';
import { ErrorBoundary, PageLayout } from '~/components';
import { PageHeader } from '~/components/ui';
import { ObjectView } from '~/pages/object-result/views/ObjectView';
import { translate, type DataType } from './ObjectResultType';
import { PkgView, TokenView } from './views';
import { InfoBox, InfoBoxStyle, InfoBoxType, LoadingIndicator } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';

const PACKAGE_TYPE_NAME = 'Move Package';

export function ObjectResult(): JSX.Element {
    const { id: objID } = useParams();
    const { data, isPending, isError, isFetched } = useGetObjectOrPastObject(objID);

    if (isPending) {
        return (
            <PageLayout
                content={
                    <div className="flex w-full items-center justify-center">
                        <LoadingIndicator text="Loading data" />
                    </div>
                }
            />
        );
    }

    const isPageError = isError || data?.error || (isFetched && !data);

    const resp = data && !isPageError ? translate(data) : null;
    const isPackage = resp ? resp.objType === PACKAGE_TYPE_NAME : false;

    return (
        <PageLayout
            content={
                <div className="flex flex-col gap-y-2xl">
                    {!isPackage && !isPageError && (
                        <div className="flex flex-col gap-y-2xl">
                            <PageHeader
                                type="Object"
                                title={resp?.id ?? ''}
                                error={
                                    data?.isViewingPastVersion
                                        ? 'This object was deleted. You are viewing a past version of this object.'
                                        : undefined
                                }
                            />
                            <ErrorBoundary>{data && <ObjectView data={data} />}</ErrorBoundary>
                        </div>
                    )}
                    {isPageError || !data || !resp ? (
                        <InfoBox
                            title="Error extracting data"
                            supportingText={`Data could not be extracted on the following specified object ID: ${objID}`}
                            icon={<Warning />}
                            type={InfoBoxType.Error}
                            style={InfoBoxStyle.Elevated}
                        />
                    ) : (
                        <>
                            {isPackage && <PageHeader type="Package" title={resp.id} />}
                            <ErrorBoundary>
                                {isPackage ? <PkgView data={resp} /> : <TokenView data={data} />}
                            </ErrorBoundary>
                        </>
                    )}
                </div>
            }
        />
    );
}

export type { DataType };
