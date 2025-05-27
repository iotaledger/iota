// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonSegment, ButtonSegmentType, KeyValueInfo } from '@iota/apps-ui-kit';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import {
    parseSerializedSignature,
    type PublicKey,
    type SignatureScheme,
} from '@iota/iota-sdk/cryptography';
import { parsePartialSignatures } from '@iota/iota-sdk/multisig';
import { normalizeIotaAddress, toB64 } from '@iota/iota-sdk/utils';
import { publicKeyFromRawBytes } from '@iota/iota-sdk/verify';

import { AddressLink } from '~/components/ui';
import { onCopySuccess } from '~/lib';

type SignaturePubkeyPair = {
    signatureScheme: SignatureScheme;
    signature: Uint8Array;
} & ({ address: string } | { publicKey: PublicKey });

interface SignaturePanelProps {
    title: string;
    signature: SignaturePubkeyPair;
}

function SignaturePanel({ title, signature: data }: SignaturePanelProps): JSX.Element {
    const { signature, signatureScheme } = data;
    return (
        <div className="flex w-full flex-col gap-md">
            <ButtonSegment selected label={title} type={ButtonSegmentType.Underlined} />
            <KeyValueInfo keyText="Scheme" value={signatureScheme} fullwidth />
            <KeyValueInfo
                keyText="Address"
                value={
                    <AddressLink
                        address={'address' in data ? data.address : data.publicKey.toIotaAddress()}
                        copyText={'address' in data ? data.address : data.publicKey.toIotaAddress()}
                        onCopySuccess={onCopySuccess}
                    />
                }
                fullwidth
            />
            {'publicKey' in data ? (
                <KeyValueInfo
                    keyText="IOTA Public Key"
                    value={data.publicKey.toIotaPublicKey()}
                    copyText={data.publicKey.toIotaPublicKey()}
                    onCopySuccess={onCopySuccess}
                    isTruncated
                    fullwidth
                />
            ) : null}
            <KeyValueInfo
                keyText="Signature"
                copyText={toB64(signature)}
                onCopySuccess={onCopySuccess}
                value={toB64(signature)}
                isTruncated
                fullwidth
            />
        </div>
    );
}

function getSignatureFromAddress(signatures: SignaturePubkeyPair[], iotaAddress: string) {
    return signatures.find(
        (signature) =>
            ('address' in signature ? signature.address : signature.publicKey.toIotaAddress()) ===
            normalizeIotaAddress(iotaAddress),
    );
}

function getSignaturesExcludingAddress(
    signatures: SignaturePubkeyPair[],
    iotaAddress: string,
): SignaturePubkeyPair[] {
    return signatures.filter(
        (signature) =>
            ('address' in signature ? signature.address : signature.publicKey.toIotaAddress()) !==
            normalizeIotaAddress(iotaAddress),
    );
}
interface SignaturesProps {
    transaction: IotaTransactionBlockResponse;
}

export function Signatures({ transaction }: SignaturesProps) {
    const sender = transaction.transaction?.data.sender;
    const gasData = transaction.transaction?.data.gasData;
    const transactionSignatures = transaction.transaction?.txSignatures;

    if (!transactionSignatures) return null;

    const isSponsoredTransaction = gasData?.owner !== sender;

    const deserializedTransactionSignatures = transactionSignatures
        .map((signature) => {
            const parsed = parseSerializedSignature(signature);
            if (parsed.signatureScheme === 'MultiSig') {
                return parsePartialSignatures(parsed.multisig);
            }

            return {
                ...parsed,
                publicKey: publicKeyFromRawBytes(parsed.signatureScheme, parsed.publicKey),
            };
        })
        .flat();

    const userSignatures = isSponsoredTransaction
        ? getSignaturesExcludingAddress(deserializedTransactionSignatures, gasData!.owner)
        : deserializedTransactionSignatures;

    const sponsorSignature = isSponsoredTransaction
        ? getSignatureFromAddress(deserializedTransactionSignatures, gasData!.owner)
        : null;

    return (
        <div className="flex flex-wrap gap-lg px-md--rs py-md md:py-md">
            {userSignatures.length > 0 && (
                <div className="flex w-full flex-col gap-lg">
                    {userSignatures.map((signature, index) => (
                        <SignaturePanel key={index} title="User Signature" signature={signature} />
                    ))}
                </div>
            )}

            {sponsorSignature && (
                <SignaturePanel title="Sponsor Signature" signature={sponsorSignature} />
            )}
        </div>
    );
}
