import { Transaction, Inputs } from '@iota/iota-sdk/transactions';

declare const digest: string;
declare const objectId: string;
declare const version: string;
declare const initialSharedVersion: string;
declare const mutable: boolean;

const tx = new Transaction();

tx.object(Inputs.ObjectRef({ digest, objectId, version }));

tx.object(Inputs.SharedObjectRef({ objectId, initialSharedVersion, mutable }));

tx.object(Inputs.ReceivingRef({ digest, objectId, version }));
