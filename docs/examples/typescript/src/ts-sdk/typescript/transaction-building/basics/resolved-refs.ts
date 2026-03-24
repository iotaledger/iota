import { Transaction, Inputs } from '@iota/iota-sdk/transactions';

const digest = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=';
const objectId = '0x0000000000000000000000000000000000000000000000000000000000000000';
const version = '1';
const initialSharedVersion = '1';
const mutable = true;

const tx = new Transaction();

tx.object(Inputs.ObjectRef({ digest, objectId, version }));

tx.object(Inputs.SharedObjectRef({ objectId, initialSharedVersion, mutable }));

tx.object(Inputs.ReceivingRef({ digest, objectId, version }));
