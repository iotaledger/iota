// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AddressAlias, useCopyToClipboard, useGetObjectOrPastObject } from '@iota/core';
import { useParams } from 'react-router-dom';
import { ErrorBoundary, PageLayout } from '~/components';
import { PageHeader } from '~/components/ui';
import { usePackageUpgradePolicy } from '~/hooks';
import { ObjectView } from '~/pages/object-result/views/ObjectView';
import { translate, type DataType } from './ObjectResultType';
import { PkgView, TokenView } from './views';
import {
    Badge,
    BadgeType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';

const PACKAGE_TYPE_NAME = 'Move Package';

export function ObjectResult(): JSX.Element {
    const { id: objID } = useParams();
    const { data, isPending, isError, isFetched } = useGetObjectOrPastObject(objID);
    const copyToClipboard = useCopyToClipboard();

    const isPageError = !isPending && (isError || data?.error || (isFetched && !data));
    const resp = data && !isPageError ? translate(data) : null;
    const isPackage = resp ? resp.objType === PACKAGE_TYPE_NAME : false;
    const txDigest = isPackage ? resp?.data.tx_digest : undefined;
    const { upgradePolicy } = usePackageUpgradePolicy(txDigest);

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

    return (
        <PageLayout
            content={
                <div className="flex flex-col gap-y-2xl">
                    {!isPackage && !isPageError && (
                        <div className="flex flex-col gap-y-2xl">
                            <PageHeader
                                type="Object"
                                title={
                                    <div className="flex flex-col gap-xs">
                                        <AddressAlias
                                            address={resp?.id || ''}
                                            onCopy={() => copyToClipboard(resp?.id || '')}
                                        />
                                    </div>
                                }
                                showCopyButton={false}
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
                            title="Invalid Object ID"
                            supportingText={`No object found matching ID: ${objID} on this network. Please verify the ID and try again.`}
                            icon={<Warning />}
                            type={InfoBoxType.Error}
                            style={InfoBoxStyle.Elevated}
                        />
                    ) : (
                        <>
                            {isPackage && (
                                <PageHeader
                                    type="Package"
                                    showCopyButton={false}
                                    title={
                                        <div className="flex items-center gap-xs">
                                            <AddressAlias
                                                address={resp.id}
                                                onCopy={() => copyToClipboard(resp.id)}
                                            />
                                            {upgradePolicy && (
                                                <span className="shrink-0">
                                                    <Badge
                                                        label={
                                                            upgradePolicy.isImmutable
                                                                ? 'Immutable'
                                                                : 'Upgradeable'
                                                        }
                                                        type={
                                                            upgradePolicy.isImmutable
                                                                ? BadgeType.Neutral
                                                                : BadgeType.PrimarySoft
                                                        }
                                                    />
                                                </span>
                                            )}
                                        </div>
                                    }
                                />
                            )}
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
