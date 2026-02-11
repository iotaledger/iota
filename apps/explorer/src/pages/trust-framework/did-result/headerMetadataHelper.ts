// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaObjectData } from '@iota/iota-sdk/src/client';
import { type MetaItem } from '~/components/ui/PageHeaderMeta';

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

/**
 * Determines the identity type of an IOTA DID object based on its type.
 *
 * @param didObject - The IOTA object data to analyze.
 * @param pkgId - The package ID to compare against for official identity package.
 * @returns A MetaItem object containing identity type information, or null if
 *          the objectData is null or has no type.
 */
export function getIdentityType(didObject: IotaObjectData | null, pkgId: string): MetaItem | null {
    if (didObject == null || didObject.type == null) {
        return null;
    }

    const [_package, _module, _method] = didObject.type.split('::');
    if (_method === IDENTITY_METHOD && _module === IDENTITY_MODULE && _package === pkgId) {
        // Official Identity package for the current network
        return {
            label: metadata.type.label,
            value: metadata.type.badge,
            visible: true,
        } as MetaItem;
    }

    return {
        label: metadata.type.label,
        value: didObject.type,
        visible: true,
    } as MetaItem;
}

/**
 * Extracts legacy metadata from an IOTA DID object if available.
 *
 * @param didObject - The IOTA DID object data containing potential legacy ID information.
 * @returns A MetaItem containing the legacy ID if found, otherwise null.
 */
export function getLegacyMetadata(didObject: IotaObjectData | null): MetaItem | null {
    if (didObject == null) {
        return null;
    }

    if (didObject.content?.dataType !== 'moveObject') {
        return null;
    }

    if (!('legacy_id' in didObject.content.fields)) {
        return null;
    }

    const legacyId = didObject.content.fields.legacy_id;
    if (legacyId == null) {
        return null;
    }

    return {
        label: metadata.legacyId.label,
        value: legacyId,
        visible: true,
    } as MetaItem;
}
