// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/* eslint-disable @typescript-eslint/no-explicit-any */

import { PublicKey } from '@iota/iota-sdk/cryptography';
import { MultiSigPublicKey } from '@iota/iota-sdk/multisig';
import { publicKeyFromIotaBytes } from '@iota/iota-sdk/verify';
import { useState } from 'react';
import { FieldValues, useFieldArray, useForm } from 'react-hook-form';
import { toast } from 'react-hot-toast';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';

export default function MultiSigAddressGenerator() {
    const [msAddress, setMSAddress] = useState('');
    const { register, control, handleSubmit, setValue } = useForm({
        defaultValues: {
            pubKeys: [{ pubKey: '', weight: '' }],
            threshold: 1,
        },
    });
    const { fields, append, remove } = useFieldArray({
        control,
        name: 'pubKeys',
    });

    // Perform generation of multisig address
    const onSubmit = (data: FieldValues) => {
        try {
            const pks: { publicKey: PublicKey; weight: number }[] = [];
            data.pubKeys.forEach((item: any) => {
                const pk = publicKeyFromIotaBytes(item.pubKey);
                pks.push({ publicKey: pk, weight: item.weight });
            });
            const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
                threshold: data.threshold,
                publicKeys: pks,
            });
            const multisigIotaAddress = multiSigPublicKey.toIotaAddress();
            setMSAddress(multisigIotaAddress);
        } catch (e: any) {
            toast.error(e?.message || 'Error generating MultiSig Address');
        }
    };

    // if you want to control your fields with watch
    // const watchResult = watch("pubKeys");
    // console.log(watchResult);

    // The following is useWatch example
    // console.log(useWatch({ name: "pubKeys", control }));

    return (
        <div className="flex flex-col gap-4">
            <h2 className="scroll-m-20 text-4xl font-extrabold tracking-tight lg:text-5xl">
                MultiSig Address Creator
            </h2>

            <form className="flex flex-col gap-4" onSubmit={handleSubmit(onSubmit)}>
                <p>The following demo allow you to create IOTA MultiSig addresses.</p>

                <div className="flex gap-2 items-center">
                    <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() => {
                            // Clear existing fields first
                            remove();
                            // Add the three example public keys
                            append({
                                pubKey: 'AIKM0+W7wvP6pitTgJQVB7Yfn2oMO3aZd3votkb6x87l',
                                weight: '1',
                            });
                            append({
                                pubKey: 'AIA4z3cY/7bzUz/Kj1mPe5I9k82gpL3J/WppWjnB53SI',
                                weight: '1',
                            });
                            append({
                                pubKey: 'APBL9QuKI1MjSNn5Jt0w0zOUWdCQxbn84UlKmJtGbuU4',
                                weight: '1',
                            });
                            setValue('threshold', 2);
                        }}
                    >
                        Example 2 out of 3
                    </Button>
                </div>

                <code className="overflow-x-auto">
                    <details>
                        <summary>Mnemonic for the example keys:</summary>
                        <p>
                            can escape fee use fabric ill brief park doll reflect bus skirt fury leg
                            brown toast diet two skull tornado name soda cave junk
                        </p>
                    </details>
                </code>
                <ul className="grid w-full gap-1.5">
                    {fields.map((item, index) => {
                        return (
                            <li key={item.id} className="grid grid-cols-3 gap-3">
                                <input
                                    className="min-h-[80px] rounded-md border border-input bg-transparent px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                    {...register(`pubKeys.${index}.pubKey`, { required: true })}
                                    placeholder="IOTA Public Key"
                                />

                                <input
                                    className="min-h-[80px] rounded-md border border-input bg-transparent px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                    type="number"
                                    {...register(`pubKeys.${index}.weight`, { required: true })}
                                    placeholder="Weight"
                                />

                                <div>
                                    <Button
                                        className="min-h-[80px] rounded-md border border-input px-3 py-2 text-sm padding-2"
                                        type="button"
                                        onClick={() => remove(index)}
                                    >
                                        Delete
                                    </Button>
                                </div>
                            </li>
                        );
                    })}
                </ul>
                <section>
                    <Button
                        type="button"
                        onClick={() => {
                            append({ pubKey: '', weight: '' });
                        }}
                    >
                        New PubKey
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

                {/* <input
					{...register('threshold', { valueAsNumber: true })}
					id="threshold"
					type="number"
					className="form-control"
				/> */}

                <Button type="submit">Create MultiSig Address</Button>
            </form>
            {msAddress && (
                <Card key={msAddress}>
                    <CardHeader>
                        <CardTitle>IOTA MultiSig Address</CardTitle>
                        <CardDescription>
                            https://docs.iota.org/developer/ts-sdk/typescript/cryptography/multisig
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
        </div>
    );
}

/*
# Examples values generated with the iota keytool:

#!/bin/bash
EXAMPLE_MNEMONIC="can escape fee use fabric ill brief park doll reflect bus skirt fury leg brown toast diet two skull tornado name soda cave junk"
iota keytool import $EXAMPLE_MNEMONIC ed25519 "m/44'/4218'/0'/0'/0'" --alias address-0-for-multisig
iota keytool import $EXAMPLE_MNEMONIC ed25519 "m/44'/4218'/0'/0'/1'" --alias address-1-for-multisig
iota keytool import $EXAMPLE_MNEMONIC ed25519 "m/44'/4218'/0'/0'/2'" --alias address-2-for-multisig
json_output=$(iota keytool list --json)

PUB_KEY_0=$(echo "$json_output" | jq -r '.[] | select(.alias == "address-0-for-multisig") | .publicBase64KeyWithFlag')
PUB_KEY_1=$(echo "$json_output" | jq -r '.[] | select(.alias == "address-1-for-multisig") | .publicBase64KeyWithFlag')
PUB_KEY_2=$(echo "$json_output" | jq -r '.[] | select(.alias == "address-2-for-multisig") | .publicBase64KeyWithFlag')

echo "Public key 0 with flag: $PUB_KEY_0"
echo "Public key 1 with flag: $PUB_KEY_1"
echo "Public key 2 with flag: $PUB_KEY_2"

# Expected output:
# Public key 0 with flag: AIKM0+W7wvP6pitTgJQVB7Yfn2oMO3aZd3votkb6x87l
# Public key 1 with flag: AIA4z3cY/7bzUz/Kj1mPe5I9k82gpL3J/WppWjnB53SI
# Public key 2 with flag: APBL9QuKI1MjSNn5Jt0w0zOUWdCQxbn84UlKmJtGbuU4

# 2 out of 2
iota keytool multi-sig-address --pks $PUB_KEY_0 $PUB_KEY_1 --weights 1 1 --threshold 2
# Expected output:
# ╭─────────────────┬────────────────────────────────────────────────────────────────────────────────────────────────────────╮
# │ multisigAddress │  0x71e0a341931b7a2751cef71190b81e88784019f7e16162c795ffa4907e50192a                                    │
# │ multisig        │ ╭────────────────────────────────────────────────────────────────────────────────────────────────────╮ │
# │                 │ │ ╭─────────────────────────┬──────────────────────────────────────────────────────────────────────╮ │ │
# │                 │ │ │ address                 │  0x12149b7f1a386833615b3f8d07349020bc27517a02f5e0d242625d8bf2b8aa95  │ │ │
# │                 │ │ │ publicBase64KeyWithFlag │  AIKM0+W7wvP6pitTgJQVB7Yfn2oMO3aZd3votkb6x87l                        │ │ │
# │                 │ │ │ weight                  │  1                                                                   │ │ │
# │                 │ │ ╰─────────────────────────┴──────────────────────────────────────────────────────────────────────╯ │ │
# │                 │ │ ╭─────────────────────────┬──────────────────────────────────────────────────────────────────────╮ │ │
# │                 │ │ │ address                 │  0x3db2c2d98a492d42fccc19d232963ba8e675ab83173e7b5574268fe2676a0b73  │ │ │
# │                 │ │ │ publicBase64KeyWithFlag │  AIA4z3cY/7bzUz/Kj1mPe5I9k82gpL3J/WppWjnB53SI                        │ │ │
# │                 │ │ │ weight                  │  1                                                                   │ │ │
# │                 │ │ ╰─────────────────────────┴──────────────────────────────────────────────────────────────────────╯ │ │
# │                 │ ╰────────────────────────────────────────────────────────────────────────────────────────────────────╯ │
# │ threshold       │  2                                                                                                     │
# ╰─────────────────┴────────────────────────────────────────────────────────────────────────────────────────────────────────╯

# 2 out of 3
iota keytool multi-sig-address --pks $PUB_KEY_0 $PUB_KEY_1 $PUB_KEY_2 --weights 1 1 1 --threshold 2
# Expected output:
# ╭─────────────────┬────────────────────────────────────────────────────────────────────────────────────────────────────────╮
# │ multisigAddress │  0x9c3d1202a483f33cc340183df29ae9ffa55697947be431c963be78917e7fc538                                    │
# │ multisig        │ ╭────────────────────────────────────────────────────────────────────────────────────────────────────╮ │
# │                 │ │ ╭─────────────────────────┬──────────────────────────────────────────────────────────────────────╮ │ │
# │                 │ │ │ address                 │  0x12149b7f1a386833615b3f8d07349020bc27517a02f5e0d242625d8bf2b8aa95  │ │ │
# │                 │ │ │ publicBase64KeyWithFlag │  AIKM0+W7wvP6pitTgJQVB7Yfn2oMO3aZd3votkb6x87l                        │ │ │
# │                 │ │ │ weight                  │  1                                                                   │ │ │
# │                 │ │ ╰─────────────────────────┴──────────────────────────────────────────────────────────────────────╯ │ │
# │                 │ │ ╭─────────────────────────┬──────────────────────────────────────────────────────────────────────╮ │ │
# │                 │ │ │ address                 │  0x3db2c2d98a492d42fccc19d232963ba8e675ab83173e7b5574268fe2676a0b73  │ │ │
# │                 │ │ │ publicBase64KeyWithFlag │  AIA4z3cY/7bzUz/Kj1mPe5I9k82gpL3J/WppWjnB53SI                        │ │ │
# │                 │ │ │ weight                  │  1                                                                   │ │ │
# │                 │ │ ╰─────────────────────────┴──────────────────────────────────────────────────────────────────────╯ │ │
# │                 │ │ ╭─────────────────────────┬──────────────────────────────────────────────────────────────────────╮ │ │
# │                 │ │ │ address                 │  0x8203b574ae9291fbceb5dfd42b12bc2708e2f013a75db6e1958e79ac3de61a4b  │ │ │
# │                 │ │ │ publicBase64KeyWithFlag │  APBL9QuKI1MjSNn5Jt0w0zOUWdCQxbn84UlKmJtGbuU4                        │ │ │
# │                 │ │ │ weight                  │  1                                                                   │ │ │
# │                 │ │ ╰─────────────────────────┴──────────────────────────────────────────────────────────────────────╯ │ │
# │                 │ ╰────────────────────────────────────────────────────────────────────────────────────────────────────╯ │
# │ threshold       │  2                                                                                                     │
# ╰─────────────────┴────────────────────────────────────────────────────────────────────────────────────────────────────────╯
*/
