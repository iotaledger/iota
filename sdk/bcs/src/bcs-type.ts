// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { fromBase58, toBase58 } from './b58.js';
import { fromBase64, toBase64 } from './b64.js';
import { fromHex, toHex } from './hex.js';
import { BcsReader } from './reader.js';
import { ulebEncode } from './uleb.js';
import type { BcsWriterOptions } from './writer.js';
import { BcsWriter } from './writer.js';
import type { EnumInputShape, EnumOutputShape, JoinString } from './types.js';

export interface BcsTypeOptions<T, Input = T, Name extends string = string> {
    name?: Name;
    validate?: (value: Input) => void;
}

export class BcsType<T, Input = T, const Name extends string = string> {
    $inferType!: T;
    $inferInput!: Input;
    name: Name;
    read: (reader: BcsReader) => T;
    serializedSize: (value: Input, options?: BcsWriterOptions) => number | null;
    validate: (value: Input) => void;
    #write: (value: Input, writer: BcsWriter) => void;
    #serialize: (value: Input, options?: BcsWriterOptions) => Uint8Array;

    constructor(
        options: {
            name: Name;
            read: (reader: BcsReader) => T;
            write: (value: Input, writer: BcsWriter) => void;
            serialize?: (value: Input, options?: BcsWriterOptions) => Uint8Array;
            serializedSize?: (value: Input) => number | null;
            validate?: (value: Input) => void;
        } & BcsTypeOptions<T, Input, Name>,
    ) {
        this.name = options.name;
        this.read = options.read;
        this.serializedSize = options.serializedSize ?? (() => null);
        this.#write = options.write;
        this.#serialize =
            options.serialize ??
            ((value, options) => {
                const writer = new BcsWriter({
                    initialSize: this.serializedSize(value) ?? undefined,
                    ...options,
                });
                this.#write(value, writer);
                return writer.toBytes();
            });

        this.validate = options.validate ?? (() => {});
    }

    write(value: Input, writer: BcsWriter) {
        this.validate(value);
        this.#write(value, writer);
    }

    serialize(value: Input, options?: BcsWriterOptions) {
        this.validate(value);
        return new SerializedBcs(this, this.#serialize(value, options));
    }

    parse(bytes: Uint8Array): T {
        const reader = new BcsReader(bytes);
        return this.read(reader);
    }

    fromHex(hex: string) {
        return this.parse(fromHex(hex));
    }

    fromBase58(b64: string) {
        return this.parse(fromBase58(b64));
    }

    fromBase64(b64: string) {
        return this.parse(fromBase64(b64));
    }

    transform<T2 = T, Input2 = Input, NewName extends string = Name>({
        name,
        input,
        output,
        validate,
    }: {
        input?: (val: Input2) => Input;
        output?: (value: T) => T2;
    } & BcsTypeOptions<T2, Input2, NewName>) {
        return new BcsType<T2, Input2, NewName>({
            name: (name ?? this.name) as NewName,
            read: (reader) => (output ? output(this.read(reader)) : (this.read(reader) as never)),
            write: (value, writer) => this.#write(input ? input(value) : (value as never), writer),
            serializedSize: (value) => this.serializedSize(input ? input(value) : (value as never)),
            serialize: (value, options) =>
                this.#serialize(input ? input(value) : (value as never), options),
            validate: (value) => {
                validate?.(value);
                this.validate(input ? input(value) : (value as never));
            },
        });
    }
}

const SERIALIZED_BCS_BRAND = Symbol.for('@iota/serialized-bcs') as never;
export function isSerializedBcs(obj: unknown): obj is SerializedBcs<unknown> {
    return !!obj && typeof obj === 'object' && (obj as any)[SERIALIZED_BCS_BRAND] === true;
}

export class SerializedBcs<T, Input = T> {
    #schema: BcsType<T, Input>;
    #bytes: Uint8Array;

    // Used to brand SerializedBcs so that they can be identified, even between multiple copies
    // of the @iota/bcs package are installed
    get [SERIALIZED_BCS_BRAND]() {
        return true;
    }

    constructor(type: BcsType<T, Input>, schema: Uint8Array) {
        this.#schema = type;
        this.#bytes = schema;
    }

    toBytes() {
        return this.#bytes;
    }

    toHex() {
        return toHex(this.#bytes);
    }

    toBase64() {
        return toBase64(this.#bytes);
    }

    toBase58() {
        return toBase58(this.#bytes);
    }

    parse() {
        return this.#schema.parse(this.#bytes);
    }
}

export function fixedSizeBcsType<T, Input = T, const Name extends string = string>({
    size,
    ...options
}: {
    name: Name;
    size: number;
    read: (reader: BcsReader) => T;
    write: (value: Input, writer: BcsWriter) => void;
} & BcsTypeOptions<T, Input, Name>) {
    return new BcsType<T, Input, Name>({
        ...options,
        serializedSize: () => size,
    });
}

export function uIntBcsType<const Name extends string = string>({
    readMethod,
    writeMethod,
    ...options
}: {
    name: Name;
    size: number;
    readMethod: `read${8 | 16 | 32}`;
    writeMethod: `write${8 | 16 | 32}`;
    maxValue: number;
} & BcsTypeOptions<number, number, Name>) {
    return fixedSizeBcsType<number, number, Name>({
        ...options,
        read: (reader) => reader[readMethod](),
        write: (value, writer) => writer[writeMethod](value),
        validate: (value) => {
            if (value < 0 || value > options.maxValue) {
                throw new TypeError(
                    `Invalid ${options.name} value: ${value}. Expected value in range 0-${options.maxValue}`,
                );
            }
            options.validate?.(value);
        },
    });
}

export function bigUIntBcsType<const Name extends string = string>({
    readMethod,
    writeMethod,
    ...options
}: {
    name: Name;
    size: number;
    readMethod: `read${64 | 128 | 256}`;
    writeMethod: `write${64 | 128 | 256}`;
    maxValue: bigint;
} & BcsTypeOptions<string, string | number | bigint>) {
    return fixedSizeBcsType<string, string | number | bigint, Name>({
        ...options,
        read: (reader) => reader[readMethod](),
        write: (value, writer) => writer[writeMethod](BigInt(value)),
        validate: (val) => {
            const value = BigInt(val);
            if (value < 0 || value > options.maxValue) {
                throw new TypeError(
                    `Invalid ${options.name} value: ${value}. Expected value in range 0-${options.maxValue}`,
                );
            }
            options.validate?.(value);
        },
    });
}

export function dynamicSizeBcsType<T, Input = T, const Name extends string = string>({
    serialize,
    ...options
}: {
    name: Name;
    read: (reader: BcsReader) => T;
    serialize: (value: Input, options?: BcsWriterOptions) => Uint8Array;
} & BcsTypeOptions<T, Input, Name>) {
    const type = new BcsType<T, Input, Name>({
        ...options,
        serialize,
        write: (value, writer) => {
            for (const byte of type.serialize(value).toBytes()) {
                writer.write8(byte);
            }
        },
    });

    return type;
}

export function stringLikeBcsType({
    toBytes,
    fromBytes,
    ...options
}: {
    name: string;
    toBytes: (value: string) => Uint8Array;
    fromBytes: (bytes: Uint8Array) => string;
    serializedSize?: (value: string) => number | null;
} & BcsTypeOptions<string>) {
    return new BcsType<string>({
        ...options,
        read: (reader) => {
            const length = reader.readULEB();
            const bytes = reader.readBytes(length);

            return fromBytes(bytes);
        },
        write: (hex, writer) => {
            const bytes = toBytes(hex);
            writer.writeULEB(bytes.length);
            for (let i = 0; i < bytes.length; i++) {
                writer.write8(bytes[i]);
            }
        },
        serialize: (value) => {
            const bytes = toBytes(value);
            const size = ulebEncode(bytes.length);
            const result = new Uint8Array(size.length + bytes.length);
            result.set(size, 0);
            result.set(bytes, size.length);

            return result;
        },
        validate: (value) => {
            if (typeof value !== 'string') {
                throw new TypeError(`Invalid ${options.name} value: ${value}. Expected string`);
            }
            options.validate?.(value);
        },
    });
}

export function lazyBcsType<T, Input>(cb: () => BcsType<T, Input>) {
    let lazyType: BcsType<T, Input> | null = null;
    function getType() {
        if (!lazyType) {
            lazyType = cb();
        }
        return lazyType;
    }

    return new BcsType<T, Input>({
        name: 'lazy' as never,
        read: (data) => getType().read(data),
        serializedSize: (value) => getType().serializedSize(value),
        write: (value, writer) => getType().write(value, writer),
        serialize: (value, options) => getType().serialize(value, options).toBytes(),
    });
}

export interface BcsStructOptions<
    T extends Record<string, BcsType<any>>,
    Name extends string = string,
> extends Omit<
        BcsTypeOptions<
            {
                [K in keyof T]: T[K] extends BcsType<infer U, any> ? U : never;
            },
            {
                [K in keyof T]: T[K] extends BcsType<any, infer U> ? U : never;
            },
            Name
        >,
        'name'
    > {
    name: Name;
    fields: T;
}

export class BcsStruct<
    T extends Record<string, BcsType<any>>,
    const Name extends string = string,
> extends BcsType<
    {
        [K in keyof T]: T[K] extends BcsType<infer U, any> ? U : never;
    },
    {
        [K in keyof T]: T[K] extends BcsType<any, infer U> ? U : never;
    },
    Name
> {
    constructor({ name, fields, ...options }: BcsStructOptions<T, Name>) {
        const canonicalOrder = Object.entries(fields);

        super({
            name,
            serializedSize: (values) => {
                let total = 0;
                for (const [field, type] of canonicalOrder) {
                    const size = type.serializedSize(values[field]);
                    if (size == null) {
                        return null;
                    }

                    total += size;
                }

                return total;
            },
            read: (reader) => {
                const result: Record<string, unknown> = {};
                for (const [field, type] of canonicalOrder) {
                    result[field] = type.read(reader);
                }

                return result as never;
            },
            write: (value, writer) => {
                for (const [field, type] of canonicalOrder) {
                    type.write(value[field], writer);
                }
            },
            ...options,
            validate: (value) => {
                options?.validate?.(value);
                if (typeof value !== 'object' || value == null) {
                    throw new TypeError(`Expected object, found ${typeof value}`);
                }
            },
        });
    }
}

export interface BcsEnumOptions<
    T extends Record<string, BcsType<any> | null>,
    Name extends string = string,
> extends Omit<
        BcsTypeOptions<
            EnumOutputShape<{
                [K in keyof T]: T[K] extends BcsType<infer U, any, any> ? U : true;
            }>,
            EnumInputShape<{
                [K in keyof T]: T[K] extends BcsType<any, infer U, any>
                    ? U
                    : boolean | object | null;
            }>,
            Name
        >,
        'name'
    > {
    name: Name;
    fields: T;
}

export class BcsEnum<
    T extends Record<string, BcsType<any> | null>,
    const Name extends string = string,
> extends BcsType<
    EnumOutputShape<{
        [K in keyof T]: T[K] extends BcsType<infer U, any> ? U : true;
    }>,
    EnumInputShape<{
        [K in keyof T]: T[K] extends BcsType<any, infer U, any> ? U : boolean | object | null;
    }>,
    Name
> {
    constructor({ fields, ...options }: BcsEnumOptions<T, Name>) {
        const canonicalOrder = Object.entries(fields as object);
        super({
            read: (reader) => {
                const index = reader.readULEB();

                const enumEntry = canonicalOrder[index];
                if (!enumEntry) {
                    throw new TypeError(`Unknown value ${index} for enum ${options.name}`);
                }

                const [kind, type] = enumEntry;

                return {
                    [kind]: type?.read(reader) ?? true,
                    $kind: kind,
                } as never;
            },
            write: (value, writer) => {
                const [name, val] = Object.entries(value).filter(([name]) =>
                    Object.hasOwn(fields, name),
                )[0];

                for (let i = 0; i < canonicalOrder.length; i++) {
                    const [optionName, optionType] = canonicalOrder[i];
                    if (optionName === name) {
                        writer.writeULEB(i);
                        optionType?.write(val, writer);
                        return;
                    }
                }
            },
            ...options,
            validate: (value) => {
                options?.validate?.(value);
                if (typeof value !== 'object' || value == null) {
                    throw new TypeError(`Expected object, found ${typeof value}`);
                }

                const keys = Object.keys(value).filter(
                    (k) => value[k] !== undefined && Object.hasOwn(fields, k),
                );

                if (keys.length !== 1) {
                    throw new TypeError(
                        `Expected object with one key, but found ${keys.length} for type ${options.name}}`,
                    );
                }

                const [variant] = keys;

                if (!Object.hasOwn(fields, variant)) {
                    throw new TypeError(`Invalid enum variant ${variant}`);
                }
            },
        });
    }
}

export interface BcsTupleOptions<T extends readonly BcsType<any>[], Name extends string>
    extends Omit<
        BcsTypeOptions<
            {
                -readonly [K in keyof T]: T[K] extends BcsType<infer T, any> ? T : never;
            },
            {
                [K in keyof T]: T[K] extends BcsType<any, infer T> ? T : never;
            },
            Name
        >,
        'name'
    > {
    name?: Name;
    fields: T;
}

export class BcsTuple<
    const T extends readonly BcsType<any>[],
    const Name extends
        string = `(${JoinString<{ [K in keyof T]: T[K] extends BcsType<any, any, infer T> ? T : never }, ', '>})`,
> extends BcsType<
    {
        -readonly [K in keyof T]: T[K] extends BcsType<infer T, any> ? T : never;
    },
    {
        [K in keyof T]: T[K] extends BcsType<any, infer T> ? T : never;
    },
    Name
> {
    constructor({ fields, name, ...options }: BcsTupleOptions<T, Name>) {
        super({
            name: name ?? (`(${fields.map((t) => t.name).join(', ')})` as never),
            serializedSize: (values) => {
                let total = 0;
                for (let i = 0; i < fields.length; i++) {
                    const size = fields[i].serializedSize(values[i]);
                    if (size == null) {
                        return null;
                    }

                    total += size;
                }

                return total;
            },
            read: (reader) => {
                const result: unknown[] = [];
                for (const field of fields) {
                    result.push(field.read(reader));
                }
                return result as never;
            },
            write: (value, writer) => {
                for (let i = 0; i < fields.length; i++) {
                    fields[i].write(value[i], writer);
                }
            },
            ...options,
            validate: (value) => {
                options?.validate?.(value);
                if (!Array.isArray(value)) {
                    throw new TypeError(`Expected array, found ${typeof value}`);
                }
                if (value.length !== fields.length) {
                    throw new TypeError(
                        `Expected array of length ${fields.length}, found ${value.length}`,
                    );
                }
            },
        });
    }
}
