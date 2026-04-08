// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType } from '@iota/apps-ui-kit';
import { parseStructTag } from '@iota/iota-sdk/utils';
import { ObjectLink } from '~/components/ui';
import { DetailPanel } from './DetailPanel';
import { StructItem, StructItemLabel } from './StructItem';

interface ControllerDetailProps {
    objectType: string;
    objectId: string;
}

export function ControllerDetail({
    objectType,
    objectId,
}: ControllerDetailProps): JSX.Element | null {
    const separator = '::';
    const objectTypeSplit = objectType?.split(separator) || [];
    const typeName = objectTypeSplit.slice(2).join(separator);
    const { address, module, name } = parseStructTag(objectType);
    const objectDetailLabels = [
        StructItemLabel.Package,
        StructItemLabel.Module,
        StructItemLabel.Type,
    ];

    return (
        <DetailPanel
            headerContent={
                <div className="flex shrink-0 items-center gap-sm">
                    <Badge type={BadgeType.Neutral} label={name} />
                    {objectId && (
                        <div className="flex flex-col items-end gap-xxxs">
                            <ObjectLink objectId={objectId} />
                        </div>
                    )}
                </div>
            }
            panelContent={
                <div className="flex flex-col gap-xs px-md--rs py-sm--rs pr-16 capitalize">
                    {objectDetailLabels.map((label) => (
                        <StructItem
                            key={label}
                            label={label}
                            packageId={address}
                            moduleName={module}
                            typeName={typeName}
                        />
                    ))}
                </div>
            }
        />
    );
}
