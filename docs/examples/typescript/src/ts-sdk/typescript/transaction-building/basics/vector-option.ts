import { Transaction } from '@iota/iota-sdk/transactions';
import { bcs } from '@iota/iota-sdk/bcs';

const tx = new Transaction();

tx.moveCall({
    target: '0x2::foo::bar',
    arguments: [
        tx.pure.vector('u8', [1, 2, 3]),
        tx.pure.option('u8', 1),
        tx.pure.option('u8', null),

        tx.pure('vector<u8>', [1, 2, 3]),
        tx.pure('option<u8>', 1),
        tx.pure('option<u8>', null),
        tx.pure('vector<option<u8>>', [1, null, 2]),

        tx.pure(bcs.vector(bcs.U8).serialize([1, 2, 3])),
        tx.pure(bcs.option(bcs.U8).serialize(1)),
        tx.pure(bcs.option(bcs.U8).serialize(null)),
        tx.pure(bcs.vector(bcs.option(bcs.U8)).serialize([1, null, 2])),
    ],
});
