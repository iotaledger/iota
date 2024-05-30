// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import { format } from 'prettier';
import ts from 'typescript';

/** @ts-ignore */
import prettierConfig from '../../../prettier.config.js';
import type {
	OpenRpcMethod,
	OpenRpcParam,
	OpenRpcSpec,
	OpenRpcType,
	OpenRpcTypeRef,
} from './open-rpc.js';

const packageRoot = path.resolve(import.meta.url.slice(5), '../..');
const openRpcSpec: OpenRpcSpec = JSON.parse(
	await fs.readFile(
		path.resolve(packageRoot, '../../crates/iota-open-rpc/spec/openrpc.json'),
		'utf-8',
	),
);
export const LICENSE_HEADER = `
// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 *  ######################################
 *  ### DO NOT EDIT THIS FILE DIRECTLY ###
 *  ######################################
 *
 * This file is generated from:
 * /crates/iota-open-rpc/spec/openrpc.json
 */
`.trim();

const options: {
	types: Record<
		string,
		{
			typeAlias?: string;
			alias?: string;
			flatten?: boolean;
		}
	>;
	methods: Record<
		string,
		{
			flattenParams?: string[];
			params: Record<
				string,
				{
					alias?: string;
					typeAlias?: string;
				}
			>;
		}
	>;
} = {
	types: {
		Coin: { alias: 'CoinStruct' },
		Data: { alias: 'IotaParsedData' },
		Event: { alias: 'IotaEvent' },
		EventFilter: { alias: 'IotaEventFilter' },
		EventID: { alias: 'EventId' },
		GasData: { alias: 'IotaGasData' },
		MoveFunctionArgType: { alias: 'IotaMoveFunctionArgType' },
		ObjectChange: { alias: 'IotaObjectChange' },
		ObjectData: { alias: 'IotaObjectData' },
		ObjectDataOptions: { alias: 'IotaObjectDataOptions' },
		ObjectRef: { alias: 'IotaObjectRef' },
		ObjectResponseQuery: { alias: 'IotaObjectResponseQuery' },
		Owner: { alias: 'ObjectOwner' },
		PaginatedIotaObjectResponse: { alias: 'PaginatedObjectsResponse' },
		PaginatedTransactionBlockResponse: { alias: 'PaginatedTransactionResponse' },
		Stake: { alias: 'StakeObject' },
		IotaCoinMetadata: { alias: 'CoinMetadata' },
		IotaProgrammableMoveCall: { alias: 'MoveCallIotaTransaction' },
		Supply: { alias: 'CoinSupply' },
		TransactionBlock: { alias: 'IotaTransactionBlock' },
		TransactionBlockEffects: { alias: 'TransactionEffects' },
		TransactionBlockKind: { alias: 'IotaTransactionBlockKind' },
		TransactionBlockResponse: { alias: 'IotaTransactionBlockResponse' },
		TransactionBlockResponseOptions: { alias: 'IotaTransactionBlockResponseOptions' },
		TransactionBlockResponseQuery: { alias: 'IotaTransactionBlockResponseQuery' },
		ValidatorApys: { alias: 'ValidatorsApy' },
		GenericSignature: {
			typeAlias: 'string',
		},
		ExecutionStatus: {
			flatten: true,
		},
	},
	methods: {
		iota_getNormalizedMoveModule: {
			params: {
				module_name: {
					alias: 'module',
				},
			},
		},
		iota_getNormalizedMoveFunction: {
			params: {
				module_name: {
					alias: 'module',
				},
				function_name: {
					alias: 'function',
				},
			},
		},
		iota_getNormalizedMoveStruct: {
			params: {
				module_name: {
					alias: 'module',
				},
				struct_name: {
					alias: 'struct',
				},
			},
		},
		iotax_getOwnedObjects: {
			flattenParams: ['query'],
			params: {
				address: {
					alias: 'owner',
				},
			},
		},
		iota_getObject: {
			params: {
				object_id: {
					alias: 'id',
				},
			},
		},
		iota_tryGetPastObject: {
			params: {
				object_id: {
					alias: 'id',
				},
				version: {
					typeAlias: 'number',
				},
			},
		},
		iota_multiGetObjects: {
			params: {
				object_ids: {
					alias: 'ids',
				},
			},
		},
		iotax_queryTransactionBlocks: {
			flattenParams: ['query'],
			params: {
				descending_order: {
					alias: 'order',
					typeAlias: `'ascending' | 'descending'`,
				},
			},
		},
		iota_executeTransactionBlock: {
			params: {
				tx_bytes: {
					alias: 'transactionBlock',
					typeAlias: 'Uint8Array | string',
				},
				signatures: {
					alias: 'signature',
					typeAlias: 'string | string[]',
				},
			},
		},
		iotax_queryEvents: {
			params: {
				descending_order: {
					alias: 'order',
					typeAlias: `'ascending' | 'descending'`,
				},
			},
		},
		iota_devInspectTransactionBlock: {
			params: {
				sender_address: {
					alias: 'sender',
				},
				tx_bytes: {
					alias: 'transactionBlock',
					typeAlias: 'TransactionBlock | Uint8Array | string',
				},
				gas_price: {
					typeAlias: 'bigint | number',
				},
			},
		},
		iota_dryRunTransactionBlock: {
			params: {
				tx_bytes: {
					alias: 'transactionBlock',
					typeAlias: 'Uint8Array | string',
				},
			},
		},
		iotax_getDynamicFields: {
			params: {
				parent_object_id: {
					alias: 'parentId',
				},
			},
		},
		iotax_getDynamicFieldObject: {
			params: {
				parent_object_id: {
					alias: 'parentId',
				},
			},
		},
	},
};

export class FileGenerator {
	imports: ts.ImportDeclaration[] = [];
	statements: ts.Statement[] = [];

	async printFile() {
		const printer = ts.createPrinter({});
		const sourcefile = ts.createSourceFile(
			'temp.ts',
			'',
			ts.ScriptTarget.ESNext,
			false,
			ts.ScriptKind.TS,
		);

		const nodes = ts.factory.createNodeArray([...this.imports, ...this.statements]);

		const result = printer.printList(ts.ListFormat.SourceFileStatements, nodes, sourcefile);

		return `${LICENSE_HEADER}\n\n${await format(result, {
			...prettierConfig,
			parser: 'typescript',
		})}`;
	}
}

const fileGenerator = new FileGenerator();

fileGenerator.statements.push(
	...(await Promise.all(
		Object.entries(openRpcSpec.components.schemas)
			.filter(([name, schema]) => isNamedType(schema) && !options.types[name]?.typeAlias)
			.map(async ([name, schema]) => {
				const typeOptions = options.types[name] ?? {};
				if ('anyOf' in schema) {
					return withDescription(
						schema,
						ts.factory.createTypeAliasDeclaration(
							[ts.factory.createModifier(ts.SyntaxKind.ExportKeyword)],
							normalizeName(name),
							undefined,
							await generateUnionType(schema.anyOf, {
								name,
								merge: typeOptions.flatten,
								base: schema,
							}),
						),
					);
				}

				if ('oneOf' in schema) {
					return withDescription(
						schema,
						ts.factory.createTypeAliasDeclaration(
							[ts.factory.createModifier(ts.SyntaxKind.ExportKeyword)],
							normalizeName(name),
							undefined,
							await generateUnionType(schema.oneOf, {
								name,
								merge: typeOptions.flatten,
								base: schema,
							}),
						),
					);
				}

				if ('type' in schema && schema.type === 'string') {
					return withDescription(
						schema,
						ts.factory.createTypeAliasDeclaration(
							[ts.factory.createModifier(ts.SyntaxKind.ExportKeyword)],
							normalizeName(name),
							undefined,
							await generateTypeReference(schema),
						),
					);
				}

				return await withDescription(
					schema,
					ts.factory.createInterfaceDeclaration(
						[ts.factory.createModifier(ts.SyntaxKind.ExportKeyword)],
						normalizeName(name),
						undefined,
						undefined,
						await createObjectMembers(schema as Extract<OpenRpcType, { type: 'object' }>),
					),
				);
			}),
	)),
);

await fs.writeFile(
	path.resolve(packageRoot, './src/client/types/generated.ts'),
	await fileGenerator.printFile(),
);

const methodGenerator = new FileGenerator();

methodGenerator.imports.push(
	ts.factory.createImportDeclaration(
		undefined,
		ts.factory.createImportClause(
			true,
			undefined,
			ts.factory.createNamespaceImport(ts.factory.createIdentifier('RpcTypes')),
		),
		ts.factory.createStringLiteral('./generated.js'),
	),
	ts.factory.createImportDeclaration(
		undefined,
		ts.factory.createImportClause(
			true,
			undefined,
			ts.factory.createNamedImports([
				ts.factory.createImportSpecifier(
					false,
					undefined,
					ts.factory.createIdentifier('TransactionBlock'),
				),
			]),
		),
		ts.factory.createStringLiteral('../../transactions/index.js'),
	),
);

methodGenerator.statements.push(
	...(await Promise.all(openRpcSpec.methods.map((method) => createMethodParams(method)))),
);

await fs.writeFile(
	path.resolve(packageRoot, './src/client/types/params.ts'),
	await methodGenerator.printFile(),
);

async function createMethodParams(method: OpenRpcMethod) {
	const methodOptions = options.methods[method.name] ?? {};
	const params = await Promise.all(
		method.params
			.filter((param) => {
				return !methodOptions.flattenParams?.includes(param.name);
			})
			.map(async (param) => {
				return withDescription(
					param,
					ts.factory.createPropertySignature(
						undefined,
						normalizeParamName(method.name, param.name),
						param.required ? undefined : ts.factory.createToken(ts.SyntaxKind.QuestionToken),
						await createMethodParam(method, param),
					),
				);
			}),
	);

	if (methodOptions.flattenParams && methodOptions.flattenParams?.length > 0) {
		return withDescription(
			method,
			ts.factory.createTypeAliasDeclaration(
				[ts.factory.createModifier(ts.SyntaxKind.ExportKeyword)],
				`${normalizeMethodName(method.name)}Params`,
				undefined,
				ts.factory.createIntersectionTypeNode([
					ts.factory.createTypeLiteralNode(params),
					...(await Promise.all(
						method.params
							.filter((param) => methodOptions.flattenParams?.includes(param.name))
							.map((param) => createMethodParam(method, { ...param, required: true })),
					)),
				]),
			),
		);
	}

	return withDescription(
		method,
		ts.factory.createInterfaceDeclaration(
			[ts.factory.createModifier(ts.SyntaxKind.ExportKeyword)],
			`${normalizeMethodName(method.name)}Params`,
			undefined,
			undefined,
			params,
		),
	);
}

async function createMethodParam(method: OpenRpcMethod, param: OpenRpcParam) {
	const type = await generateTypeReference(param.schema, 'RpcTypes');
	const paramOptions = options.methods[method.name]?.params[param.name] ?? {};

	return param.required
		? paramOptions.typeAlias
			? ts.factory.createTypeReferenceNode(paramOptions.typeAlias)
			: type
		: ts.factory.createUnionTypeNode([
				paramOptions.typeAlias ? ts.factory.createTypeReferenceNode(paramOptions.typeAlias) : type,
				ts.factory.createLiteralTypeNode(ts.factory.createNull()),
				ts.factory.createToken(ts.SyntaxKind.UndefinedKeyword),
		  ]);
}

async function createObjectMembers(
	type: Extract<OpenRpcTypeRef, { type: 'object' }>,
	namespace?: string,
) {
	const members: ts.TypeElement[] = [];

	if (type.additionalProperties === true) {
		members.push(
			ts.factory.createIndexSignature(
				undefined,
				[
					ts.factory.createParameterDeclaration(
						undefined,
						undefined,
						'key',
						undefined,
						ts.factory.createKeywordTypeNode(ts.SyntaxKind.StringKeyword),
					),
				],
				ts.factory.createKeywordTypeNode(ts.SyntaxKind.UnknownKeyword),
			),
		);
	} else if (type.additionalProperties) {
		members.push(
			ts.factory.createIndexSignature(
				undefined,
				[
					ts.factory.createParameterDeclaration(
						undefined,
						undefined,
						'key',
						undefined,
						ts.factory.createKeywordTypeNode(ts.SyntaxKind.StringKeyword),
					),
				],
				await generateTypeReference(type.additionalProperties, namespace),
			),
		);
	}

	members.push(
		...(await Promise.all(
			Object.entries(type.properties ?? {}).map(async ([name, property]) => {
				return withDescription(
					property,
					ts.factory.createPropertySignature(
						undefined,
						name,
						type.required?.includes(name)
							? undefined
							: ts.factory.createToken(ts.SyntaxKind.QuestionToken),
						await generateTypeReference(property),
					),
				);
			}),
		)),
	);

	return members;
}

async function withDescription<T extends ts.Node>(
	options: { description?: string } | OpenRpcTypeRef,
	node: T,
) {
	if (typeof options === 'object' && 'description' in options && options.description)
		ts.addSyntheticLeadingComment(
			node,
			ts.SyntaxKind.MultiLineCommentTrivia,
			await formatComment(options.description),
			true,
		);

	return node;
}

async function generateTypeReference(
	type: OpenRpcTypeRef,
	namespace?: string,
): Promise<ts.TypeNode> {
	if (type === true) {
		return ts.factory.createKeywordTypeNode(ts.SyntaxKind.UnknownKeyword);
	}

	if (Array.isArray(type)) {
		return ts.factory.createUnionTypeNode(
			await Promise.all(type.map((type) => generateTypeReference(type, namespace))),
		);
	}

	if ('$ref' in type) {
		const name = type.$ref.split('/').pop()!;
		const ref = openRpcSpec.components.schemas[name];

		if (isNamedType(ref)) {
			return ts.factory.createTypeReferenceNode(
				namespace ? `${namespace}.${normalizeName(name)}` : normalizeName(name),
			);
		}

		return generateTypeReference(ref, namespace);
	}

	if ('anyOf' in type) {
		return generateUnionType([...(type.anyOf ?? [])], { base: type });
	}

	if ('oneOf' in type) {
		return generateUnionType([...(type.oneOf ?? [])], { base: type });
	}

	if ('allOf' in type) {
		const types = [...(type.allOf ?? [])];
		return ts.factory.createIntersectionTypeNode(
			await Promise.all(types.map((t) => generateTypeReference(t, namespace))),
		);
	}

	if (!('type' in type)) {
		return ts.factory.createKeywordTypeNode(ts.SyntaxKind.UnknownKeyword);
	}

	if (Array.isArray(type.type)) {
		return generateUnionType(
			type.type.map((item) => ({
				...type,
				type: item,
			})),
		);
	}

	switch (type.type) {
		case 'string':
			if (type.enum) {
				return ts.factory.createUnionTypeNode(
					type.enum.map((value) =>
						ts.factory.createLiteralTypeNode(ts.factory.createStringLiteral(value)),
					),
				);
			}
			return ts.factory.createKeywordTypeNode(ts.SyntaxKind.StringKeyword);
		case 'integer':
			if (type.format === 'uint64') {
				return ts.factory.createKeywordTypeNode(ts.SyntaxKind.StringKeyword);
			}
			return ts.factory.createKeywordTypeNode(ts.SyntaxKind.NumberKeyword);
		case 'number':
			return ts.factory.createKeywordTypeNode(ts.SyntaxKind.NumberKeyword);
		case 'boolean':
			return ts.factory.createKeywordTypeNode(ts.SyntaxKind.BooleanKeyword);
		case 'array':
			if (Array.isArray(type.items)) {
				return ts.factory.createTupleTypeNode(
					await Promise.all(type.items.map((t) => generateTypeReference(t, namespace))),
				);
			}
			return ts.factory.createArrayTypeNode(await generateTypeReference(type.items, namespace));
		case 'object':
			return withDescription(
				type,
				ts.factory.createTypeLiteralNode(await createObjectMembers(type, namespace)),
			);
		case 'null':
			return ts.factory.createLiteralTypeNode(ts.factory.createNull());
		default:
			throw new TypeError(`Unknown type: ${JSON.stringify(type)}`);
	}
}

async function generateUnionType(
	types: OpenRpcTypeRef[],
	{
		name,
		merge,
		base,
	}: {
		merge?: boolean;
		name?: string;
		base?: Extract<OpenRpcTypeRef, { oneOf: unknown } | { anyOf: unknown }>;
	} = {},
): Promise<ts.TypeNode> {
	if (merge) {
		const merged = {
			type: 'object' as const,
			properties: {} as Record<string, OpenRpcTypeRef>,
		};

		const required = new Set<string>((types[0] as { required?: string[] }).required ?? []);

		for (const type of types) {
			if (typeof type !== 'object' || !('type' in type) || type.type !== 'object') {
				throw new TypeError(`Cannot merge non-object type: ${JSON.stringify(type)}`);
			}

			required.forEach((key) => {
				if (!type.required?.includes(key)) {
					required.delete(key);
				}
			});

			Object.keys(type.properties ?? {}).forEach((key) => {
				const ref = type.properties![key];

				if (merged.properties![key] === undefined) {
					merged.properties![key] = ref;
					return;
				}

				const existing = merged.properties![key];
				if (!isTypeRef('string', existing) || !isTypeRef('string', ref)) {
					throw new TypeError(`Cannot merge non-string type`);
				}

				const enumValues = new Set([...(existing.enum ?? []), ...(ref.enum ?? [])]);
				merged.properties![key] = {
					...existing,
					...ref,
					enum: enumValues.size > 0 ? [...enumValues] : undefined,
				};
			});
		}

		return ts.factory.createTypeLiteralNode(
			await createObjectMembers({
				...merged,
				required: [...required],
			}),
		);
	}

	let refs;
	if (base && (base.properties || base.additionalProperties || base.required)) {
		refs = await Promise.all(
			types.map((item) => {
				return generateTypeReference(
					isTypeRef('object', item)
						? {
								...item,
								properties: {
									...base.properties,
									...item.properties,
								},
								required: [...(base.required ?? []), ...(item.required ?? [])],
								additionalProperties: base.additionalProperties,
						  }
						: item,
				);
			}),
		);
	} else {
		refs = await Promise.all(
			types.map((item) =>
				generateTypeReference(
					typeof item === 'object'
						? {
								description: undefined,
								...item,
						  }
						: item,
				),
			),
		);
	}

	const flattened = [];

	for (const ref of refs) {
		if (ts.isUnionTypeNode(ref)) {
			flattened.push(...ref.types);
		} else {
			flattened.push(ref);
		}
	}

	if (name) {
		return ts.factory.createUnionTypeNode(
			flattened.filter((ref) => {
				if (ts.isTypeReferenceNode(ref)) {
					return (ref.typeName as { escapedText: string }).escapedText !== name;
				}

				return true;
			}),
		);
	}

	return ts.factory.createUnionTypeNode(flattened);
}

async function formatComment(text: string) {
	const lines = (await format(text, { ...prettierConfig, parser: 'markdown', proseWrap: 'always' }))
		.trim()
		.split('\n');

	if (lines.length === 1) return `* ${lines[0]} `;

	return `*\n ${lines.map((line) => ` * ${line}`).join('\n')}\n `;
}

function isNamedType(
	type: OpenRpcType,
): type is Extract<
	OpenRpcType,
	{ anyOf: unknown } | { oneOf: unknown } | { type: 'object' } | { type: 'string'; enum: string[] }
> {
	return (
		'anyOf' in type ||
		'oneOf' in type ||
		('type' in type && type.type === 'object') ||
		('type' in type && type.type === 'string' && type.enum !== undefined)
	);
}

function isTypeRef<T extends string>(
	type: T,
	ref: OpenRpcTypeRef,
): ref is Extract<OpenRpcTypeRef, { type: T }> {
	return typeof ref === 'object' && 'type' in ref && ref.type === type;
}

function normalizeName(name: string) {
	const typeOptions = options.types[name] ?? {};

	if (typeOptions.alias) {
		return typeOptions.alias;
	}

	if (typeOptions.typeAlias) {
		return typeOptions.typeAlias;
	}

	if (name.startsWith('Page_for_')) {
		return normalizeName(
			name.replace(/^Page_for_(.*)_and.*/, `Paginated$1${name.includes('Response') ? '' : 's'}`),
		);
	}

	return name;
}

export function normalizeMethodName(name: string): string {
	if (name.startsWith('iota_')) {
		return normalizeMethodName(name.slice(4));
	}

	if (name.startsWith('iotax_')) {
		return normalizeMethodName(name.slice(5));
	}

	const parts = name.split('_');

	return parts.map((part) => part[0].toUpperCase() + part.slice(1)).join('');
}

export function normalizeParamName(method: string, name: string) {
	const alias = options.methods[method]?.params[name]?.alias;

	if (alias) {
		return alias;
	}

	const parts = name.split('_');

	return parts.map((part, i) => (i > 0 ? part[0].toUpperCase() + part.slice(1) : part)).join('');
}
