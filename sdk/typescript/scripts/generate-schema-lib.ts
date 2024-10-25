// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { execSync } from 'child_process';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

// creates the schema tsconfig file in the respective target folder
export async function createSchemaTsConfigFile(targetFolder: string, minorVersion: string) {
    // create tsconfig.tada.json
    await writeFile(
        resolve(targetFolder, 'tsconfig.tada.json'),
        `
{
    "compilerOptions": {
        "plugins": [
            {
                "name": "gql.tada/ts-plugin",
                "schema": "./schema.graphql",
                "tadaOutputLocation": "src/graphql/generated/${minorVersion}/tada-env.d.ts"
            }
        ]
    }
}`.trimStart(),
    );
}

// generates the schema in the respective target folder by running `pnpm run generate-schema`
export async function generateSchema(targetFolder: string) {
    const filePath = resolve(targetFolder, 'tsconfig.tada.json');

    execSync(`pnpm run generate-schema -c ${filePath}`);
}

// creates the schema index file in the respective target folder
export async function createSchemaIndexFile(targetFolder: string, minorVersion: string) {
    // create index.ts
    await writeFile(
        resolve(targetFolder, 'index.ts'),
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

// adds the exports to the package.json file
export async function addExportsToPackageJson(packageRoot: string, versions: string[]) {
    const packageJsonPath = resolve(packageRoot, 'package.json');
    const packageJson = JSON.parse(await readFile(packageJsonPath, 'utf-8'));

    for (const version of versions) {
        packageJson.exports[`./graphql/schemas/${version}`] = {
            import: `./dist/esm/graphql/schemas/${version}/index.js`,
            require: `./dist/cjs/graphql/schemas/${version}/index.js`,
        };
    }

    await writeFile(packageJsonPath, `${JSON.stringify(packageJson, null, '    ')}\n`);
}
