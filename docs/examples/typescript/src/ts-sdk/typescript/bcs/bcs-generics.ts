import { bcs, type BcsType } from '@iota/bcs';

// The T typescript generic is a placeholder for the typescript type of the generic value.
// The T argument will be the bcs type passed in when creating a concrete instance of the Container type.
function Container<T extends BcsType<any>>(T: T) {
    return bcs.struct('Container<T>', {
        contents: T,
    });
}

// When serializing, we have to pass the type to use for `T`.
const bytes = Container(bcs.u8()).serialize({ contents: 100 }).toBytes();

// Alternatively we can save the concrete type as a variable.
const U8Container = Container(bcs.u8());
const bytes2 = U8Container.serialize({ contents: 100 }).toBytes();

// Using multiple generics
function VecMap<K extends BcsType<any>, V extends BcsType<any>>(K: K, V: V) {
    return bcs.struct(
        // You can use the names of the generic params to give your type a more useful name
        `VecMap<${K.name}, ${V.name}>`,
        {
            keys: bcs.vector(K),
            values: bcs.vector(V),
        },
    );
}

// To serialize VecMap, we can use:
VecMap(bcs.string(), bcs.string())
    .serialize({
        keys: ['key1', 'key2', 'key3'],
        values: ['value1', 'value2', 'value3'],
    })
    .toBytes();

console.log({ bytes, bytes2 });
