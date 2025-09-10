import {
    Account,
    AccountType,
    type SerializedAccount,
    type SerializedUIAccount,
    type SigningAccount,
} from './account';
import { toBase64 } from '@iota/iota-sdk/utils';

// No need to store any encryption data, just the public key
type SessionStorageData = { publicKey: string };

export interface PasskeyAccountSerialized extends SerializedAccount {
    type: AccountType.PasskeyDerived;
    publicKey: string;
    rpId: string;
    rpName: string;
}

export interface PasskeyAccountSerializedUI extends SerializedUIAccount {
    type: AccountType.PasskeyDerived;
    publicKey: string;
}

export function isPasskeyAccountSerializedUI(
    account: SerializedUIAccount,
): account is PasskeyAccountSerializedUI {
    return account.type === AccountType.PasskeyDerived;
}

export class PasskeyAccount
    extends Account<PasskeyAccountSerialized, SessionStorageData>
    implements SigningAccount
{
    readonly canSign = true;

    static async createNew(inputs: {
        address: string;
        publicKey: string;
        rpId: string;
        rpName: string;
    }): Promise<Omit<PasskeyAccountSerialized, 'id'>> {
        // Create a new passkey
        // console.log('Creating passkey provider with:', inputs);
        // const provider = new BrowserPasskeyProvider(inputs.rpName, {
        //     rp: {
        //         id: inputs.rpId,
        //         name: inputs.rpName,
        //     },
        //     // authenticatorSelection: {
        //     //     authenticatorAttachment: 'cross-platform',
        //     //     residentKey: 'required',
        //     //     userVerification: 'required',
        //     // },
        // });
        // console.log('12 Passkey provider created:', inputs);
        // // This will prompt the user to create a new passkey
        // const passkey = inputs.passkeyKeyPair;
        // // const passkey = await PasskeyKeypair.getPasskeyInstance(inputs.provider);
        // console.log('Passkey instance created:', passkey);
        // const publicKey = passkey.getPublicKey();
        // console.log('PublicKEy instance created:', passkey);

        return {
            type: AccountType.PasskeyDerived,
            // address: publicKey.toIotaAddress(),
            // publicKey: publicKey.toBase64(),
            address: inputs.address,
            publicKey: inputs.publicKey,
            rpId: inputs.rpId,
            rpName: inputs.rpName,
            lastUnlockedOn: null,
            selected: false,
            nickname: null,
            createdAt: Date.now(),
        };
    }

    static isOfType(serialized: SerializedAccount): serialized is PasskeyAccountSerialized {
        return serialized.type === AccountType.PasskeyDerived;
    }

    constructor({ id, cachedData }: { id: string; cachedData?: PasskeyAccountSerialized }) {
        super({ type: AccountType.PasskeyDerived, id, cachedData });
    }

    async lock(allowRead = false): Promise<void> {
        // With passkeys, we don't need to clear any sensitive data
        // But we'll clear the session cache as a best practice
        await this.clearEphemeralValue();
        await this.onLocked(allowRead);
    }

    async isLocked(): Promise<boolean> {
        // Passkeys are always "unlocked" when available on the device
        // but we'll check if we have the publicKey in session
        const ephemeralData = await this.getEphemeralValue();
        return !ephemeralData?.publicKey;
    }

    async toUISerialized(): Promise<PasskeyAccountSerializedUI> {
        const { address, publicKey, type, selected, nickname } = await this.getStoredData();
        return {
            id: this.id,
            type,
            address,
            publicKey,
            isLocked: await this.isLocked(),
            lastUnlockedOn: await this.lastUnlockedOn,
            selected,
            nickname,
            isPasswordUnlockable: false, // Passkeys don't use passwords
            isKeyPairExportable: false, // Cannot export private keys from passkeys
        };
    }

    async unlock(): Promise<void> {
        const { publicKey } = await this.getStoredData();
        await this.setEphemeralValue({ publicKey });
        await this.onUnlocked();
    }

    async signData(data: Uint8Array): Promise<string> {
        const { rpId, rpName, publicKey } = await this.getStoredData();

        // Create the passkey provider
        // const provider = new BrowserPasskeyProvider(rpName, {
        //     rp: {
        //         id: rpId,
        //         name: rpName,
        //     },
        //     // authenticatorSelection: {
        //     //     authenticatorAttachment: 'cross-platform',
        //     //     residentKey: 'required',
        //     //     userVerification: 'required',
        //     // },
        // });

        // We need to recover the keypair using the signAndRecover method
        // This uses two signatures to find the correct public key
        // const testMessage1 = new TextEncoder().encode('IOTA Auth Message 1');
        // const possiblePks1 = await passkey.signAndRecover(provider, testMessage1);

        // const testMessage2 = new TextEncoder().encode('IOTA Auth Message 2');
        // const possiblePks2 = await PasskeyKeypair.signAndRecover(provider, testMessage2);

        // Find the common public key
        // const commonPk = findCommonPublicKey(possiblePks1, possiblePks2);

        // Create the keypair with the identified public key
        // const keyPair = new PasskeyKeypair(commonPk.toRawBytes(), provider);
        // const signature = await passkeyKeyPair.sign(data);

        // Now sign the actual data
        // return toBase64(signature);
        return toBase64(new Uint8Array());
    }
}
