import { Transaction } from '@iota/iota-sdk/transactions';
import { bcs } from '@iota/iota-sdk/bcs';

const tx = new Transaction();

const [coin] = tx.splitCoins(tx.gas, [tx.pure.u64(100)]);
const [coin2] = tx.splitCoins(tx.gas, [tx.pure(bcs.U64.serialize(100))]);
tx.transferObjects([coin], tx.pure.address('0xSomeIotaAddress'));
tx.transferObjects([coin], tx.pure(bcs.Address.serialize('0xSomeIotaAddress')));
