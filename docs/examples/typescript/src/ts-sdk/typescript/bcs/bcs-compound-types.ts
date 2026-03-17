import { bcs } from '@iota/bcs';

// Vectors
const intList = bcs.vector(bcs.u8()).serialize([1, 2, 3, 4, 5]).toBytes();
const stringList = bcs.vector(bcs.string()).serialize(['a', 'b', 'c']).toBytes();

// Fixed arrays
const intArray = bcs.fixedArray(4, bcs.u8()).serialize([1, 2, 3, 4]).toBytes();
const stringArray = bcs.fixedArray(3, bcs.string()).serialize(['a', 'b', 'c']).toBytes();

// Option
const option = bcs.option(bcs.string()).serialize('some value').toBytes();
const nullOption = bcs.option(bcs.string()).serialize(null).toBytes();

// Enum
const MyEnum = bcs.enum('MyEnum', {
    NoType: null,
    Int: bcs.u8(),
    String: bcs.string(),
    Array: bcs.fixedArray(3, bcs.u8()),
});

const noTypeEnum = MyEnum.serialize({ NoType: null }).toBytes();
const intEnum = MyEnum.serialize({ Int: 100 }).toBytes();
const stringEnum = MyEnum.serialize({ String: 'string' }).toBytes();
const arrayEnum = MyEnum.serialize({ Array: [1, 2, 3] }).toBytes();

// Struct
const MyStruct = bcs.struct('MyStruct', {
    id: bcs.u8(),
    name: bcs.string(),
});

const struct = MyStruct.serialize({ id: 1, name: 'name' }).toBytes();

// Tuple
const tuple = bcs.tuple([bcs.u8(), bcs.string()]).serialize([1, 'name']).toBytes();

// Map
const map = bcs
    .map(bcs.u8(), bcs.string())
    .serialize(
        new Map([
            [1, 'one'],
            [2, 'two'],
        ]),
    )
    .toBytes();

// Parsing data back into original types

// Vectors
const parsedIntList = bcs.vector(bcs.u8()).parse(intList);
const parsedStringList = bcs.vector(bcs.string()).parse(stringList);

// Fixed arrays
const parsedIntArray = bcs.fixedArray(4, bcs.u8()).parse(intArray);

// Option
const parsedOption = bcs.option(bcs.string()).parse(option);
const parsedNullOption = bcs.option(bcs.string()).parse(nullOption);

// Enum
const parsedNoTypeEnum = MyEnum.parse(noTypeEnum);
const parsedIntEnum = MyEnum.parse(intEnum);
const parsedStringEnum = MyEnum.parse(stringEnum);
const parsedArrayEnum = MyEnum.parse(arrayEnum);

// Struct
const parsedStruct = MyStruct.parse(struct);

// Tuple
const parsedTuple = bcs.tuple([bcs.u8(), bcs.string()]).parse(tuple);

// Map
const parsedMap = bcs.map(bcs.u8(), bcs.string()).parse(map);

console.log({
    parsedIntList, parsedStringList, parsedIntArray,
    parsedOption, parsedNullOption,
    parsedNoTypeEnum, parsedIntEnum, parsedStringEnum, parsedArrayEnum,
    parsedStruct, parsedTuple, parsedMap,
    stringArray, intArray, intEnum, stringEnum, arrayEnum, struct, tuple, map,
});
