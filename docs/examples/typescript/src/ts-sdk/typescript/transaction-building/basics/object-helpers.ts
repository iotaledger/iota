import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();

tx.object.system();
tx.object.clock();
tx.object.random();
tx.object.denyList();

tx.object.option({
	type: '0x123::example::Thing',
	value: '0x456',
});
