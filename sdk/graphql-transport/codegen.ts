// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CodegenConfig } from '@graphql-codegen/cli';

const header = `
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
/* eslint-disable */
`.trimStart();

const config: CodegenConfig = {
    overwrite: true,
    schema: '../typescript/src/graphql/generated/2024.11/schema.graphql',
    documents: ['src/queries/*.graphql'],
    ignoreNoDocuments: true,
    generates: {
        'src/generated/queries.ts': {
            plugins: [
                {
                    add: {
                        content: header,
                    },
                },
                'typescript',
                'typescript-operations',
                {
                    'typed-document-node': {
                        documentMode: 'string',
                    },
                },
            ],
        },
    },
};

export default config;
