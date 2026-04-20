// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { KeyValueInfo } from '@iota/apps-ui-kit';
import { ObjectLink } from '~/components/ui';

export enum StructItemLabel {
    Package = 'package',
    Module = 'module',
    Type = 'type',
}

interface StructItemProps {
    label: string;
    packageId?: string;
    moduleName?: string;
    typeName?: string;
}

export function StructItem({
    label,
    packageId,
    moduleName,
    typeName,
}: StructItemProps): JSX.Element | null {
    switch (label) {
        case StructItemLabel.Package:
            return (
                <KeyValueInfo
                    keyText={label}
                    value={<ObjectLink objectId={packageId || ''} copyText={packageId} />}
                />
            );
        case StructItemLabel.Module:
            return (
                <KeyValueInfo
                    keyText={label}
                    value={
                        <ObjectLink
                            objectId={packageId ? `${packageId}?module=${moduleName}` : ''}
                            label={moduleName || ''}
                        />
                    }
                />
            );
        case StructItemLabel.Type:
            return <KeyValueInfo keyText={label} value={typeName || ''} />;
        default:
            return <KeyValueInfo keyText={label} value="" />;
    }
}
