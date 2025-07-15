// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/* eslint-disable @typescript-eslint/no-explicit-any */

import { PublicKey } from '@iota/iota-sdk/cryptography';
import { MultiSigPublicKey } from '@iota/iota-sdk/multisig';
import { publicKeyFromIotaBytes } from '@iota/iota-sdk/verify';
import { useEffect, useState } from 'react';
import { FieldValues, useFieldArray, useForm } from 'react-hook-form';
import toast from 'react-hot-toast';
import { useSearchParams } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';

export default function MultiSigCombineSignatureGenerator() {
    const [msAddress, setMSAddress] = useState('');
    const [msSignature, setMSSignature] = useState('');
    const [generatedUrl, setGeneratedUrl] = useState('');

    const [query] = useSearchParams();

    useEffect(() => {
        const pks = query.get('pks');
        const weights =
            query
                .get('weights')
                ?.split(',')
                .map((x) => x.trim()) || [];

        const threshold = parseInt(query.get('threshold') ?? '') || 1;

        if (pks) {
            const pubKeys = JSON.parse(pks);
            replacePubKeysArray(
                pubKeys.map((key: string, index: number) => ({
                    pubKey: key,
                    weight: weights[index] || 1,
                    signature: '',
                })),
            );
        }
        if (threshold) {
            setValue('threshold', threshold);
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [query]);

    const generateThresholdsUrl = () => {
        const values = getValues();
        const url = new URL(window.location.href);
        url.searchParams.set('pks', JSON.stringify(values.pubKeys.map((item: any) => item.pubKey)));
        url.searchParams.set('weights', values.pubKeys.map((item: any) => item.weight).join(','));
        url.searchParams.set('threshold', values.threshold.toString());

        setGeneratedUrl(url.href);
        window.navigator.clipboard.writeText(url.href);
        toast.success('Copied to clipboard');
    };

    const { register, control, handleSubmit, setValue, getValues } = useForm({
        defaultValues: {
            pubKeys: [{ pubKey: '', weight: '', signature: '' }],
            threshold: 1,
        },
    });
    const {
        fields,
        append,
        remove,
        replace: replacePubKeysArray,
    } = useFieldArray({
        control,
        name: 'pubKeys',
    });

    // Perform generation of multisig address
    const onSubmit = (data: FieldValues) => {
        try {
            setGeneratedUrl(''); // clear for better visibility.
            // handle MultiSig Pubkeys, Weights, and Threshold
            const pks: { publicKey: PublicKey; weight: number }[] = [];
            const sigs: string[] = [];
            data.pubKeys.forEach((item: any) => {
                const pk = publicKeyFromIotaBytes(item.pubKey);
                pks.push({ publicKey: pk, weight: Number(item.weight) });
                if (item.signature) {
                    sigs.push(item.signature);
                }
            });
            const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
                threshold: data.threshold,
                publicKeys: pks,
            });
            const multisigIotaAddress = multiSigPublicKey.toIotaAddress();
            setMSAddress(multisigIotaAddress);
            const multisigCombinedSig = multiSigPublicKey.combinePartialSignatures(sigs);
            setMSSignature(multisigCombinedSig);
        } catch (e: any) {
            toast.error(e?.message ?? 'An error occurred');
        }
    };

    return (
        <div className="flex flex-col gap-4">
            <h2 className="scroll-m-20 text-4xl font-extrabold tracking-tight lg:text-5xl">
                MultiSig Combined Signature Creator
            </h2>

            <form className="flex flex-col gap-4" onSubmit={handleSubmit(onSubmit)}>
                <p>The following demo allow you to create IOTA MultiSig Combined Signatures.</p>
                <p>IOTA Pubkeys, weights, signatures for testing/playing with:</p>
                <div className="flex flex-col gap-2 bg-gray-600 p-4 rounded-md">
                    <div className="flex gap-0 border-b">
                        <div className="flex-1 font-bold border-r p-2">IOTA Pubkeys</div>
                        <div className="flex-1 font-bold border-r p-2">Weights</div>
                        <div className="flex-1 font-bold p-2">Signatures</div>
                    </div>
                    <div className="flex gap-0 border-b">
                        <div className="flex-1 break-all border-r p-2">
                            AIKM0+W7wvP6pitTgJQVB7Yfn2oMO3aZd3votkb6x87l
                        </div>
                        <div className="flex-1 border-r p-2">1</div>
                        <div className="flex-1 break-all p-2">
                            AP58oYBpNZRsR8ReDL05R/37o8l5t89e+RdBDId7yA0+Oxt/F/jlfCw8bnFR596zhVi9CN19bb0aWpn8U0cENQqCjNPlu8Lz+qYrU4CUFQe2H59qDDt2mXd76LZG+sfO5Q==
                        </div>
                    </div>
                    <div className="flex gap-0 border-b">
                        <div className="flex-1 break-all border-r p-2">
                            AIA4z3cY/7bzUz/Kj1mPe5I9k82gpL3J/WppWjnB53SI
                        </div>
                        <div className="flex-1 border-r p-2">1</div>
                        <div className="flex-1 break-all p-2">
                            AIG+CPPEfpfJC/1AMSXrfPGmJ4hK7n2nGRp7ZTrYW3mPgM6zGJ+vepGk+CL0F9ihnzdA++CM2DUUCYOv4rHrQAqAOM93GP+281M/yo9Zj3uSPZPNoKS9yf1qaVo5wed0iA==
                        </div>
                    </div>
                    <div className="flex gap-0">
                        <div className="flex-1 break-all border-r p-2">
                            APBL9QuKI1MjSNn5Jt0w0zOUWdCQxbn84UlKmJtGbuU4
                        </div>
                        <div className="flex-1 border-r p-2">1</div>
                        <div className="flex-1 p-2"></div>
                    </div>
                </div>
                <div className="grid w-full gap-1.5">
                    {fields.map((item, index) => {
                        return (
                            <div
                                key={item.id}
                                className="grid grid-cols-2 max-md:border-b max-md:pb-3 max-md:mb-3 lg:grid-cols-6 gap-3 "
                            >
                                <input
                                    className="min-h-[80px] lg:col-span-2 rounded-md border border-input bg-transparent px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                    {...register(`pubKeys.${index}.pubKey`, { required: true })}
                                    placeholder="Public Key"
                                />

                                <input
                                    className="min-h-[80px] rounded-md border border-input bg-transparent px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                    type="number"
                                    {...register(`pubKeys.${index}.weight`, { required: true })}
                                    placeholder="Weight"
                                />
                                <input
                                    className="min-h-[80px] lg:col-span-2 rounded-md border border-input bg-transparent px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                    {...register(`pubKeys.${index}.signature`, { required: false })}
                                    placeholder="Signature (paste here or leave empty)"
                                />
                                <div>
                                    <Button
                                        className="min-h-[80px] w-auto rounded-md border border-input px-3 py-2 text-sm padding-2"
                                        type="button"
                                        onClick={() => remove(index)}
                                    >
                                        Delete
                                    </Button>
                                </div>
                            </div>
                        );
                    })}
                </div>
                <section className="flex flex-wrap gap-5">
                    <Button
                        type="button"
                        onClick={() => {
                            append({ pubKey: '', weight: '', signature: '' });
                        }}
                    >
                        New PubKey
                    </Button>
                    <Button
                        type="button"
                        variant="outline"
                        onClick={() => {
                            replacePubKeysArray([
                                {
                                    pubKey: 'AIKM0+W7wvP6pitTgJQVB7Yfn2oMO3aZd3votkb6x87l',
                                    weight: '1',
                                    signature:
                                        'AP58oYBpNZRsR8ReDL05R/37o8l5t89e+RdBDId7yA0+Oxt/F/jlfCw8bnFR596zhVi9CN19bb0aWpn8U0cENQqCjNPlu8Lz+qYrU4CUFQe2H59qDDt2mXd76LZG+sfO5Q==',
                                },
                                {
                                    pubKey: 'AIA4z3cY/7bzUz/Kj1mPe5I9k82gpL3J/WppWjnB53SI',
                                    weight: '1',
                                    signature:
                                        'AIG+CPPEfpfJC/1AMSXrfPGmJ4hK7n2nGRp7ZTrYW3mPgM6zGJ+vepGk+CL0F9ihnzdA++CM2DUUCYOv4rHrQAqAOM93GP+281M/yo9Zj3uSPZPNoKS9yf1qaVo5wed0iA==',
                                },
                                {
                                    pubKey: 'APBL9QuKI1MjSNn5Jt0w0zOUWdCQxbn84UlKmJtGbuU4',
                                    weight: '1',
                                    signature: '',
                                },
                            ]);
                            setValue('threshold', 2);
                        }}
                    >
                        Set to example values
                    </Button>
                </section>
                <section>
                    <label className="form-label min-h-[80px] rounded-md text-sm px-3 py-2 ring-offset-background">
                        MultiSig Threshold Value:
                    </label>
                    <input
                        className="min-h-[80px] rounded-md border border-input bg-transparent px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                        type="number"
                        {...register(`threshold`, { valueAsNumber: true, required: true })}
                    />
                </section>

                <section className="grid grid-cols-1 md:grid-cols-4 gap-5">
                    <Button type="submit" className="md:col-span-3">
                        Combine signatures
                    </Button>
                    <Button variant={'outline'} type="button" onClick={generateThresholdsUrl}>
                        Generate reusable URL
                    </Button>
                </section>
            </form>
            {generatedUrl && (
                <Card key={generatedUrl}>
                    <CardHeader>
                        <CardTitle>Reusable URL</CardTitle>
                        <CardDescription>
                            Using this URL you can skip adding the pubkeys, weights and threshold
                            every time you use this tool for a particular multi-sig address.
                        </CardDescription>
                    </CardHeader>
                    <CardContent>
                        <div className="flex flex-col gap-2">
                            <div className="bg-muted rounded text-sm font-mono p-2 break-all">
                                {generatedUrl}
                            </div>
                        </div>
                    </CardContent>
                </Card>
            )}
            {msAddress && (
                <Card key={msAddress}>
                    <CardHeader>
                        <CardTitle>IOTA MultiSig Address</CardTitle>
                        <CardDescription>
                            https://docs.iota.org/developer/cryptography/transaction-auth/multisig
                        </CardDescription>
                    </CardHeader>
                    <CardContent>
                        <div className="flex flex-col gap-2">
                            <div className="bg-muted rounded text-sm font-mono p-2 break-all">
                                {msAddress}
                            </div>
                        </div>
                    </CardContent>
                </Card>
            )}
            {msSignature && (
                <Card key={msSignature}>
                    <CardHeader>
                        <CardTitle>IOTA MultiSig Combined Signature</CardTitle>
                        <CardDescription>
                            https://docs.iota.org/developer/cryptography/transaction-auth/multisig
                        </CardDescription>
                    </CardHeader>
                    <CardContent>
                        <div className="flex flex-col gap-2">
                            <div className="bg-muted rounded text-sm font-mono p-2 break-all">
                                {msSignature}
                            </div>
                        </div>
                    </CardContent>
                </Card>
            )}
        </div>
    );
}

/*
# Keys from comment in multisig-address.tsx

# Prepared tx bytes to send 1000000000 from 0x9c3d1202a483f33cc340183df29ae9ffa55697947be431c963be78917e7fc538 to itself
TX_BYTES="AAACAAgAypo7AAAAAAAgnD0SAqSD8zzDQBg98prp/6VWl5R75DHJY754kX5/xTgCAgABAQAAAQEDAAAAAAEBAJw9EgKkg/M8w0AYPfKa6f+lVpeUe+QxyWO+eJF+f8U4AoMDJiR7kcSFDFoZ7UzCU6MoBn73fgTOMBOAOs+IILIrDTcAAAAAAAAgs5nazsoEocwG689hghNgpFaWiJOU/9DHN7Yq2maiwTnlODl+LFe++RM9vcUs9iAPRQHml+BB8ZCOGHgM+7ngnww3AAAAAAAAIAnVjkdkzUTZiGSLgaE9098I5CjfgJ2GcvBqXYpqsRkynD0SAqSD8zzDQBg98prp/6VWl5R75DHJY754kX5/xTjoAwAAAAAAAOBvPAAAAAAAAA=="

# Sign with first address
iota keytool sign --address address-0-for-multisig --data $TX_BYTES
# ╭───────────────┬──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
# │ iotaAddress   │ 0x12149b7f1a386833615b3f8d07349020bc27517a02f5e0d242625d8bf2b8aa95                                                                                               │
# │ rawTxData     │ AAACAAgAypo7AAAAAAAgnD0SAqSD8zzDQBg98prp/6VWl5R75DHJY754kX5/xTgCAgABAQAAAQEDAAAAAAEBAJw9EgKkg/M8w0AYPfKa6f+lVpeUe+QxyWO+eJF+f8U4AoMDJiR7kcSFDFoZ7UzCU6MoBn73fgTO │
# │               │ MBOAOs+IILIrDTcAAAAAAAAgs5nazsoEocwG689hghNgpFaWiJOU/9DHN7Yq2maiwTnlODl+LFe++RM9vcUs9iAPRQHml+BB8ZCOGHgM+7ngnww3AAAAAAAAIAnVjkdkzUTZiGSLgaE9098I5CjfgJ2GcvBqXYpq │
# │               │ sRkynD0SAqSD8zzDQBg98prp/6VWl5R75DHJY754kX5/xTjoAwAAAAAAAOBvPAAAAAAAAA==                                                                                         │
# │ intent        │ ╭─────────┬─────╮                                                                                                                                                │
# │               │ │ scope   │  0  │                                                                                                                                                │
# │               │ │ version │  0  │                                                                                                                                                │
# │               │ │ app_id  │  0  │                                                                                                                                                │
# │               │ ╰─────────┴─────╯                                                                                                                                                │
# │ rawIntentMsg  │ AAAAAAACAAgAypo7AAAAAAAgnD0SAqSD8zzDQBg98prp/6VWl5R75DHJY754kX5/xTgCAgABAQAAAQEDAAAAAAEBAJw9EgKkg/M8w0AYPfKa6f+lVpeUe+QxyWO+eJF+f8U4AoMDJiR7kcSFDFoZ7UzCU6MoBn73 │
# │               │ fgTOMBOAOs+IILIrDTcAAAAAAAAgs5nazsoEocwG689hghNgpFaWiJOU/9DHN7Yq2maiwTnlODl+LFe++RM9vcUs9iAPRQHml+BB8ZCOGHgM+7ngnww3AAAAAAAAIAnVjkdkzUTZiGSLgaE9098I5CjfgJ2GcvBq │
# │               │ XYpqsRkynD0SAqSD8zzDQBg98prp/6VWl5R75DHJY754kX5/xTjoAwAAAAAAAOBvPAAAAAAAAA==                                                                                     │
# │ digest        │ pV79fMCpxkNW5Ow8D7wr6XFK8i4D+xEjsmLp8S5HPqg=                                                                                                                     │
# │ iotaSignature │ AP58oYBpNZRsR8ReDL05R/37o8l5t89e+RdBDId7yA0+Oxt/F/jlfCw8bnFR596zhVi9CN19bb0aWpn8U0cENQqCjNPlu8Lz+qYrU4CUFQe2H59qDDt2mXd76LZG+sfO5Q==                             │
# ╰───────────────┴──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯

# Sign with second address
iota keytool sign --address address-1-for-multisig --data $TX_BYTES
# ╭───────────────┬──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
# │ iotaAddress   │ 0x3db2c2d98a492d42fccc19d232963ba8e675ab83173e7b5574268fe2676a0b73                                                                                               │
# │ rawTxData     │ AAACAAgAypo7AAAAAAAgnD0SAqSD8zzDQBg98prp/6VWl5R75DHJY754kX5/xTgCAgABAQAAAQEDAAAAAAEBAJw9EgKkg/M8w0AYPfKa6f+lVpeUe+QxyWO+eJF+f8U4AoMDJiR7kcSFDFoZ7UzCU6MoBn73fgTO │
# │               │ MBOAOs+IILIrDTcAAAAAAAAgs5nazsoEocwG689hghNgpFaWiJOU/9DHN7Yq2maiwTnlODl+LFe++RM9vcUs9iAPRQHml+BB8ZCOGHgM+7ngnww3AAAAAAAAIAnVjkdkzUTZiGSLgaE9098I5CjfgJ2GcvBqXYpq │
# │               │ sRkynD0SAqSD8zzDQBg98prp/6VWl5R75DHJY754kX5/xTjoAwAAAAAAAOBvPAAAAAAAAA==                                                                                         │
# │ intent        │ ╭─────────┬─────╮                                                                                                                                                │
# │               │ │ scope   │  0  │                                                                                                                                                │
# │               │ │ version │  0  │                                                                                                                                                │
# │               │ │ app_id  │  0  │                                                                                                                                                │
# │               │ ╰─────────┴─────╯                                                                                                                                                │
# │ rawIntentMsg  │ AAAAAAACAAgAypo7AAAAAAAgnD0SAqSD8zzDQBg98prp/6VWl5R75DHJY754kX5/xTgCAgABAQAAAQEDAAAAAAEBAJw9EgKkg/M8w0AYPfKa6f+lVpeUe+QxyWO+eJF+f8U4AoMDJiR7kcSFDFoZ7UzCU6MoBn73 │
# │               │ fgTOMBOAOs+IILIrDTcAAAAAAAAgs5nazsoEocwG689hghNgpFaWiJOU/9DHN7Yq2maiwTnlODl+LFe++RM9vcUs9iAPRQHml+BB8ZCOGHgM+7ngnww3AAAAAAAAIAnVjkdkzUTZiGSLgaE9098I5CjfgJ2GcvBq │
# │               │ XYpqsRkynD0SAqSD8zzDQBg98prp/6VWl5R75DHJY754kX5/xTjoAwAAAAAAAOBvPAAAAAAAAA==                                                                                     │
# │ digest        │ pV79fMCpxkNW5Ow8D7wr6XFK8i4D+xEjsmLp8S5HPqg=                                                                                                                     │
# │ iotaSignature │ AIG+CPPEfpfJC/1AMSXrfPGmJ4hK7n2nGRp7ZTrYW3mPgM6zGJ+vepGk+CL0F9ihnzdA++CM2DUUCYOv4rHrQAqAOM93GP+281M/yo9Zj3uSPZPNoKS9yf1qaVo5wed0iA==                             │
# ╰───────────────┴──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯

iota keytool multi-sig-combine-partial-sig \
--pks \
AIKM0+W7wvP6pitTgJQVB7Yfn2oMO3aZd3votkb6x87l \
AIA4z3cY/7bzUz/Kj1mPe5I9k82gpL3J/WppWjnB53SI \
APBL9QuKI1MjSNn5Jt0w0zOUWdCQxbn84UlKmJtGbuU4 \
--weights 1 1 1 \
--threshold 2 \
--sigs \
AP58oYBpNZRsR8ReDL05R/37o8l5t89e+RdBDId7yA0+Oxt/F/jlfCw8bnFR596zhVi9CN19bb0aWpn8U0cENQqCjNPlu8Lz+qYrU4CUFQe2H59qDDt2mXd76LZG+sfO5Q== \
AIG+CPPEfpfJC/1AMSXrfPGmJ4hK7n2nGRp7ZTrYW3mPgM6zGJ+vepGk+CL0F9ihnzdA++CM2DUUCYOv4rHrQAqAOM93GP+281M/yo9Zj3uSPZPNoKS9yf1qaVo5wed0iA==

# multisigParsed and multisigSerialized are the same: https://github.com/iotaledger/iota/issues/7668
# ╭────────────────────┬────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
# │ multisigAddress    │  0x9c3d1202a483f33cc340183df29ae9ffa55697947be431c963be78917e7fc538                                                                                                                                                                                                                                                                │
# │ multisigParsed     │  AwIA/nyhgGk1lGxHxF4MvTlH/fujyXm3z175F0EMh3vIDT47G38X+OV8LDxucVHn3rOFWL0I3X1tvRpamfxTRwQ1CgCBvgjzxH6XyQv9QDEl63zxpieISu59pxkae2U62Ft5j4DOsxifr3qRpPgi9BfYoZ83QPvgjNg1FAmDr+Kx60AKAwADAIKM0+W7wvP6pitTgJQVB7Yfn2oMO3aZd3votkb6x87lAQCAOM93GP+281M/yo9Zj3uSPZPNoKS9yf1qaVo5wed0iAEA8Ev1C4ojUyNI2fkm3TDTM5RZ0JDFufzhSUqYm0Zu5TgBAgA=  │
# │ multisigSerialized │  AwIA/nyhgGk1lGxHxF4MvTlH/fujyXm3z175F0EMh3vIDT47G38X+OV8LDxucVHn3rOFWL0I3X1tvRpamfxTRwQ1CgCBvgjzxH6XyQv9QDEl63zxpieISu59pxkae2U62Ft5j4DOsxifr3qRpPgi9BfYoZ83QPvgjNg1FAmDr+Kx60AKAwADAIKM0+W7wvP6pitTgJQVB7Yfn2oMO3aZd3votkb6x87lAQCAOM93GP+281M/yo9Zj3uSPZPNoKS9yf1qaVo5wed0iAEA8Ev1C4ojUyNI2fkm3TDTM5RZ0JDFufzhSUqYm0Zu5TgBAgA=  │
# ╰────────────────────┴────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
*/
