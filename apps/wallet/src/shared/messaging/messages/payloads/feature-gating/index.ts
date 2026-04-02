// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';
import type { FeatureDefinition } from '@iota/apps-backend-client';

export type LoadedFeatures = Record<string, FeatureDefinition>;
export type LoadedAttributes = Record<string, unknown>;

export interface LoadedFeaturesPayload extends BasePayload {
    type: 'features-response';
    features: LoadedFeatures;
    attributes: LoadedAttributes;
}

export function isLoadedFeaturesPayload(payload: Payload): payload is LoadedFeaturesPayload {
    return isBasePayload(payload) && payload.type === 'features-response';
}
