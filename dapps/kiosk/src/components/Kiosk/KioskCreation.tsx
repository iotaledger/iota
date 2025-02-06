// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toast } from 'react-hot-toast';

import { useCreateKioskMutation } from '../../mutations/kiosk';
import { Button } from '../Base/Button';

export function KioskCreation({ onCreate }: { onCreate: () => void }) {
    const createKiosk = useCreateKioskMutation({
        onSuccess: () => {
            onCreate();
            toast.success('Kiosk created successfully');
        },
    });

    return (
        <div className="min-h-[70vh] container py-24 gap-4 mt-6">
            <div className="lg:w-7/12 mx-auto">
                <h2 className="font-bold text-3xl mb-6">Create an IOTA Kiosk</h2>
                <p className="pb-3">
                    <strong>There’s no kiosk for your address yet.</strong> Create a kiosk to store
                    your digital assets and list them for sale on the IOTA network. Anyone can view
                    your kiosk and the assets you place in it.
                </p>
                <p className="pb-3">
                    The demo app works only on <strong>IOTA Testnet.</strong> Make sure that your
                    wallet connects to Testnet and that you have at least 1 IOTA to cover gas fees.
                    You can get test IOTA tokens using{' '}
                    <a
                        href="https://docs.iota.org/references/cli/cheatsheet#faucet--gas"
                        target="_blank"
                        rel="noreferrer"
                        className="underline"
                    >
                        the faucet
                    </a>
                    .
                </p>
                <p className="pb-3">
                    When you click <strong>Create Kiosk</strong>, your wallet opens. Click{' '}
                    <strong>Approve</strong> to allow the app to create a kiosk for the connected
                    wallet address.
                </p>
                <Button
                    loading={createKiosk.isPending}
                    onClick={() => createKiosk.mutate()}
                    className="mt-3 px-12 bg-primary text-white"
                >
                    Create Kiosk
                </Button>
            </div>
        </div>
    );
}
