import { IotaClient, getFullnodeUrl } from '@iota/iota-sdk/client';
import {
    decodeIotaPrivateKey,
    parseSerializedSignature,
} from '@iota/iota-sdk/cryptography';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';

// ================= CONFIG =================

const RPC = getFullnodeUrl('testnet'); // devnet | localnet | mainnet
const ONLINE = true; // set to false to skip online submission

const IOTA_BECH32_PRIV = 'iotaprivkey1....';

const AMOUNT = 100; // nanos
const RECIPIENT_OVERRIDE: string | null = null;

// ==========================================

async function main() {
    // ---------- OFFLINE: Keypair & address ----------
    const decoded = decodeIotaPrivateKey(IOTA_BECH32_PRIV);
    const keypair = Ed25519Keypair.fromSecretKey(decoded.secretKey);

    const senderAddress = keypair.getPublicKey().toIotaAddress();
    const recipient = RECIPIENT_OVERRIDE ?? senderAddress;

    console.log('Sender:', senderAddress);
    console.log('Recipient:', recipient);

    // ---------- ONLINE: client ----------
    const client = new IotaClient({ url: RPC });

    // ---------- ONLINE : fetch ALL owned objects ----------
    const ownedRes = await client.getOwnedObjects({
        owner: senderAddress,
        options: { showType: true },
    });

    const owned = ownedRes.data ?? [];

    if (owned.length === 0) {
        console.warn('No owned objects found. Fund address via faucet.');
        return;
    }

    // ---------- Try extracting gas coins from owned objects ----------
    let gasObjects = owned
        .filter(
            (o) =>
                o.data &&
                o.data.type === '0x2::coin::Coin<0x2::iota::IOTA>',
        )
        .map((o) => ({
            objectId: o.data!.objectId,
            version: Number(o.data!.version),
            digest: o.data!.digest,
        }));

    // ---------- Fallback: canonical gas coin API ----------
    if (gasObjects.length === 0) {
        console.warn(
            'No gas coins found via getOwnedObjects(). Falling back to getCoins().',
        );

        const coinsRes = await client.getCoins({
            owner: senderAddress,
        });

        if (coinsRes.data.length === 0) {
            throw new Error('No gas coins available. Fund address via faucet.');
        }

        gasObjects = coinsRes.data.map((coin) => ({
            objectId: coin.coinObjectId,
            version: Number(coin.version),
            digest: coin.digest,
        }));
    }

    console.log('Gas coins (sample):', gasObjects.slice(0, 3));

    // ---------- OFFLINE: build transaction ----------
    const tx = new Transaction();

    // Split gas coin to create transfer amount
    const [coin] = tx.splitCoins(tx.gas, [AMOUNT]);
    tx.transferObjects([coin], recipient);

    // Fully define gas & sender for offline build
    tx.setGasPayment(gasObjects);
    tx.setGasBudget(3_000_000);
    tx.setGasPrice(1_000);
    tx.setSender(senderAddress);

    // Build unsigned BCS transaction bytes
    const unsignedTxBytes = await tx.build();
    console.log('Unsigned tx bytes length:', unsignedTxBytes.length);

    // ---------- decode unsigned tx ----------
    const decodedTx = Transaction.from(unsignedTxBytes);
    console.log(
        'Decoded unsigned tx:',
        JSON.stringify(decodedTx.getData(), null, 2),
    );

    // ---------- OFFLINE: sign transaction ----------
    const { signature } = await keypair.signTransaction(unsignedTxBytes);

    console.log('Serialized signature (base64):', signature);

    // ---------- inspect signature ----------
    const parsed = parseSerializedSignature(signature);
    console.log('Parsed signature:', {
        scheme: parsed.signatureScheme,
        signatureBytes: parsed.signature?.length,
        publicKeyBytes: parsed.publicKey?.length,
    });

    // ---------- ONLINE: submit signed transaction ----------
    if (ONLINE) {
        const execResult = await client.executeTransactionBlock({
            transactionBlock: unsignedTxBytes,
            signature,
            requestType: 'WaitForLocalExecution',
            options: {
                showEffects: true,
                showInput: true,
            },
        });

        console.log('Transaction digest:', execResult.digest);

        await client.waitForTransaction({
            digest: execResult.digest,
            options: { showEffects: true },
        });
    }

    console.log('Done.');
}

main().catch((e) => {
    console.error('Error:', e);
    process.exit(1);
});
