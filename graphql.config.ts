// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IGraphQLConfig } from 'graphql-config';

const config: IGraphQLConfig = {
    projects: {
        tsSDK: {
            schema: './crates/iota-graphql-rpc/schema/current_progress_schema.graphql',
            documents: [
                './sdk/graphql-transport/src/**/*.ts',
                './sdk/graphql-transport/src/**/*.graphql',
            ],
            include: [
                './sdk/graphql-transport/src/**/*.ts',
                './sdk/graphql-transport/src/**/*.graphql',
            ],
        },
    },
};

export default config;
