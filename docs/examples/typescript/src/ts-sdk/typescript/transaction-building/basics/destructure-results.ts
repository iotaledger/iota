import { Transaction } from '@iota/iota-sdk/transactions';

const address = '0x0';

const tx = new Transaction();

const [nft1, nft2] = tx.moveCall({ target: '0x2::nft::mint_many' });
tx.transferObjects([nft1, nft2], address);

const mintMany = tx.moveCall({ target: '0x2::nft::mint_many' });
tx.transferObjects([mintMany[0], mintMany[1]], address);
