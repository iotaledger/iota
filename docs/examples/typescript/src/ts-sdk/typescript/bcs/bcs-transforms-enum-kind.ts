import { bcs, type BcsType } from '@iota/bcs';

type Merge<T> = T extends infer U ? { [K in keyof U]: U[K] } : never;
type EnumKindTransform<T> = T extends infer U
    ? Merge<(U[keyof U] extends null | boolean ? object : U[keyof U]) & { kind: keyof U }>
    : never;

function enumKind<T extends object, Input extends object>(type: BcsType<T, Input>) {
    return type.transform({
        input: ({ kind, ...val }: EnumKindTransform<Input>) =>
            ({
                [kind]: val,
            }) as Input,
        output: (val) => {
            const key = Object.keys(val)[0] as keyof T;

            return { kind: key, ...val[key] } as EnumKindTransform<T>;
        },
    });
}

const MyEnum = enumKind(
    bcs.enum('MyEnum', {
        A: bcs.struct('A', {
            id: bcs.u8(),
        }),
        B: bcs.struct('B', {
            val: bcs.string(),
        }),
    }),
);

// Enums wrapped with enumKind flatten the enum variants and add a `kind` field to differentiate them
const A = MyEnum.serialize({ kind: 'A', id: 1 }).toBytes();
const B = MyEnum.serialize({ kind: 'B', val: 'xyz' }).toBytes();

const parsedA = MyEnum.parse(A); // returns { kind: 'A', id: 1 }

console.log({ A, B, parsedA });
