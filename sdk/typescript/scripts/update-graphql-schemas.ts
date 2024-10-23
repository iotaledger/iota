// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { execSync } from 'child_process';
import { readFile } from 'fs/promises';
import { mkdir, writeFile } from 'node:fs/promises';
import path, { resolve } from 'node:path';

const BRANCH = 'develop';
const MAJOR = 2024;
const MINOR = 10;
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

for (const { minorVersion } of releases.values()) {
    const packageRoot = path.resolve(import.meta.url.slice(5), '../..');

    const schema = await readFile(
        path.resolve(packageRoot, '../../crates/iota-graphql-rpc/schema.graphql'),
        'utf-8',
    );

    const filePath = resolve(
        import.meta.url.slice(5),
        `../../src/graphql/generated/${minorVersion}/schema.graphql`,
    );

    await mkdir(resolve(filePath, '..'), { recursive: true });
    await writeFile(filePath, schema);

    await writeFile(
        resolve(filePath, '..', 'tsconfig.tada.json'),
        `
{
    "compilerOptions": {
        "plugins": [
            {
                "name": "@0no-co/graphqlsp",
                "schema": "./schema.graphql",
                "tadaOutputLocation": "src/graphql/generated/${minorVersion}/tada-env.d.ts"
            }
        ]
    }
}
`.trimStart(),
    );

    execSync(`pnpm run generate-schema -c ${resolve(filePath, '..', 'tsconfig.tada.json')}`);

    await mkdir(resolve(filePath, '../../../schemas', minorVersion), { recursive: true });
    await writeFile(
        resolve(filePath, `../../../schemas/${minorVersion}/index.ts`),
        `
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { initGraphQLTada } from 'gql.tada';

import type { introspection } from '../../generated/${minorVersion}/tada-env.js';
import type { CustomScalars } from '../../types.js';

export * from '../../types.js';

export type { FragmentOf, ResultOf, VariablesOf, TadaDocumentNode } from 'gql.tada';
export { readFragment, maskFragments } from 'gql.tada';

export const graphql = initGraphQLTada<{
	introspection: introspection;
	scalars: CustomScalars;
}>();
`.trimStart(),
    );
}

const releaseVersions = releases.map(({ minorVersion }) => minorVersion);

await addExportsToPackageJson(releaseVersions);

async function addExportsToPackageJson(versions: string[]) {
    const packageJsonPath = resolve(import.meta.url.slice(5), '../../package.json');
    const packageJson = JSON.parse(await readFile(packageJsonPath, 'utf-8'));

    for (const version of versions) {
        packageJson.exports[`./graphql/schemas/${version}`] = {
            import: `./dist/esm/graphql/schemas/${version}/index.js`,
            require: `./dist/cjs/graphql/schemas/${version}/index.js`,
        };
    }

    await writeFile(packageJsonPath, `${JSON.stringify(packageJson, null, '    ')}\n`);
}
