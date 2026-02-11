// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaObjectData } from '@iota/iota-sdk/src/client';
import { type MetaItem } from '~/components/ui/PageHeaderMeta';
import { type IotaDocument } from '@iota/identity-wasm/web';

const IDENTITY_MODULE = 'identity';
const IDENTITY_METHOD = 'Identity';

const metadata = {
    legacyId: {
        label: 'Legacy ID',
    },
    type: {
        label: 'Type',
        badge: 'IOTA Identity',
    },
};

export class MetadataBuilder {
    items: MetaItem[];

    public constructor() {
        this.items = [];
    }

    static create(): MetadataBuilder {
        return new MetadataBuilder();
    }

    addItem(item: MetaItem | null): MetadataBuilder {
        if (item != null) {
            this.items.push(item);
        }
        return this;
    }

    build(): MetaItem[] {
        return this.items;
    }
}

export function getIdentityType(objectData: IotaObjectData | null, pkgId: string): MetaItem | null {
    if (objectData == null || objectData.type == null) {
        return null;
    }

    const [_package, _module, _method] = objectData.type.split('::');
    if (_method === IDENTITY_METHOD && _module === IDENTITY_MODULE && _package === pkgId) {
        return {
            label: metadata.type.label,
            value: metadata.type.badge,
            visible: true,
        } as MetaItem;
    }

    return {
        label: metadata.type.label,
        value: objectData.type,
        visible: true,
    } as MetaItem;
}

export function getLegacyMetadata(didDocument: IotaDocument | null): MetaItem | null {
    const legacyId = didDocument?.toCoreDocument().properties().get('alsoKnownAs');
    if (legacyId == null) {
        return null;
    }

    return {
        label: metadata.legacyId.label,
        value: legacyId,
        visible: true,
    } as MetaItem;
}
