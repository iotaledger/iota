// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { mkdir } from 'node:fs/promises';
import { copyFile } from 'node:fs';
import path, { resolve } from 'node:path';
import {
    createSchemaTsConfigFile,
    generateSchema,
    createSchemaIndexFile,
    addExportsToPackageJson,
} from './generate-schema-lib.js';

const BRANCH = 'develop';
const MAJOR = 2025;
const MINOR = 2;
const PATCH = 0;

const VERSION = `${MAJOR}.${MINOR}.${PATCH}`;
const MINOR_VERSION = `${MAJOR}.${MINOR}`;

const releases = [
    {
        version: VERSION,
        minorVersion: MINOR_VERSION,
        major: MAJOR,
        minor: MINOR,
        patch: PATCH,
        branch: BRANCH,
    },
];

const packageRoot = path.resolve(import.meta.url.slice(5), '../..');
const workspaceRoot = path.resolve(packageRoot, '../..');
const schemaSourceFilePath = path.resolve(
    workspaceRoot,
    'crates/iota-graphql-rpc',
    'schema.graphql',
);

for (const { minorVersion } of releases.values()) {
    const targetFolderGenerated = path.resolve(
        packageRoot,
        `src/graphql/generated/${minorVersion}/`,
    );
    const targetFolderSchemas = resolve(packageRoot, `src/graphql/schemas/${minorVersion}/`);

    // create target folders
    await mkdir(targetFolderGenerated, { recursive: true });
    await mkdir(targetFolderSchemas, { recursive: true });

    // copy the schema file to the target folder
    copyFile(schemaSourceFilePath, resolve(targetFolderGenerated, 'schema.graphql'), (err) => {
        if (err) throw err;
    });

    // create additional files per version
    await createSchemaTsConfigFile(targetFolderGenerated, minorVersion);
    await generateSchema(targetFolderGenerated);
    await createSchemaIndexFile(targetFolderSchemas, minorVersion);
}

const releaseVersions = releases.map(({ minorVersion }) => minorVersion);

// add exports to package.json
await addExportsToPackageJson(packageRoot, releaseVersions);
