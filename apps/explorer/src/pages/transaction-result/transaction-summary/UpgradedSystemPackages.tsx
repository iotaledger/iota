// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '@iota/ui';

import { CollapsibleCard, CollapsibleSection, ObjectLink } from '~/components/ui';

import type { OwnedObjectRef } from '@iota/iota-sdk/client';

export function UpgradedSystemPackages({ data }: { data: OwnedObjectRef[] }): JSX.Element | null {
    if (!data?.length) return null;

    return (
        <CollapsibleCard title="Changes">
            <CollapsibleSection
                title={
                    <Text variant="body/semibold" color="success-dark">
                        Updated
                    </Text>
                }
            >
                <div className="flex flex-col gap-2">
                    {data.map((object) => {
                        const { objectId } = object.reference;
                        return (
                            <div
                                className="flex flex-wrap items-center justify-between"
                                key={objectId}
                            >
                                <div className="flex items-center gap-0.5">
                                    <Text variant="pBody/medium" color="steel-dark">
                                        Package
                                    </Text>
                                </div>

                                <div className="flex items-center">
                                    <ObjectLink objectId={objectId} />
                                </div>
                            </div>
                        );
                    })}
                </div>
            </CollapsibleSection>
        </CollapsibleCard>
    );
}
