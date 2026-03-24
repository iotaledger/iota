import { bcs } from '@iota/iota-sdk/bcs';

bcs.U8.serialize(1);
bcs.Address.serialize('0x1');
bcs.TypeTag.serialize({
    vector: {
        u8: true,
    },
});
