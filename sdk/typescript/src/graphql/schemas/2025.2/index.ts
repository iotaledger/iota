// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { initGraphQLTada } from 'gql.tada';

import type { introspection } from '../../generated/2025.2/tada-env.js';
import type { CustomScalars } from '../../types.js';

export * from '../../types.js';

export type { FragmentOf, ResultOf, VariablesOf, TadaDocumentNode } from 'gql.tada';
export { readFragment, maskFragments } from 'gql.tada';

export const graphql = initGraphQLTada<{
    introspection: introspection;
    scalars: CustomScalars;
}>();
