import { ethers, HDNodeWallet, Wallet } from 'ethers';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

export type TestWalletData = {
    addressL1: string;
    addressL2: string;
    mnemonicL1: string;
    mnemonicL2: string;
};

export function generate24WordMnemonic() {
    const entropy = ethers.randomBytes(32);
    return ethers.Mnemonic.fromEntropy(entropy).phrase;
}

export function deriveAddressFromMnemonic(mnemonic: string) {
    const keypair = Ed25519Keypair.deriveKeypair(mnemonic);
    const address = keypair.getPublicKey().toIotaAddress();
    return { address, keypair };
}

export async function checkBalanceWithRetries(
    fetchBalance: () => Promise<string | null>,
    layer: 'L1' | 'L2',
    maxRetries = 10,
    delay = 2500,
): Promise<string | null> {
    let balance: string | null = null;

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
        try {
            balance = await fetchBalance();
        } catch (error) {
            console.error('Error checking balance:', error);
        } finally {
            if ((!balance || balance?.startsWith('0')) && attempt < maxRetries) {
                console.log(
                    `Fetching ${layer} balance attempt ${attempt + 1} out of ${maxRetries} in ${delay} ms`,
                );
                await new Promise((resolve) => setTimeout(resolve, delay));
            }
        }
    }

    return balance;
}

export function getRandomL2MnemonicAndAddress(): { mnemonic: string; address: string } {
    const mnemonic = Wallet.createRandom().mnemonic;

    if (!mnemonic) {
        throw new Error('Failed to generate mnemonic');
    }

    return {
        mnemonic: mnemonic.phrase,
        address: HDNodeWallet.fromMnemonic(mnemonic, `m/44'/60'/0'/0/0`).address,
    };
}

export function generateTestWallets(): TestWalletData {
    const mnemonicL1 = generate24WordMnemonic();
    const { address: addressL1 } = deriveAddressFromMnemonic(mnemonicL1);

    const { mnemonic: mnemonicL2, address: addressL2 } = getRandomL2MnemonicAndAddress();

    return {
        mnemonicL1,
        addressL1,
        mnemonicL2,
        addressL2,
    };
}
