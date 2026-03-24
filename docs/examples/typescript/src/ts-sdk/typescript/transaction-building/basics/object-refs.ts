import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();

tx.transferObjects(['0xSomeObject'], '0xSomeAddress');
tx.transferObjects([tx.object('0xSomeObject')], '0xSomeAddress');

tx.moveCall({
    target: '0x2::nft::mint',
    arguments: [tx.object('0xSomeObject')],
});

tx.moveCall({
    target: '0xSomeAddress::example::receive_object',
    arguments: [tx.object('0xParentObjectID'), tx.object('0xReceivingObjectID')],
});
