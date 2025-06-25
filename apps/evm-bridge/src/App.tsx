// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Header, Bridge, Footer } from './components';
import { DirectionalArrowsSvg } from './components/svgs/DirectionalArrows';
import { FaucetButton } from './components/FaucetButton';

const TITLE = 'Seamlessly transfer funds between IOTA & IOTA EVM';

export default function App() {
    return (
        <>
            <div className="relative overflow-x-hidden" id="app">
                <Header />

                <main className="flex flex-col min-h-screen container justify-center pt-24 flex-1">
                    <div className="flex flex-col items-center md:flex-row gap-lg h-full py-20">
                        <div className="flex-1 pb-2xl flex flex-col items-center md:items-start justify-center">
                            <div className="flex flex-col max-md:items-center max-md:text-center md:flex-col gap-10 pb-[72px] ">
                                <DirectionalArrowsSvg />
                                <h1 className="text-display-md text-balance text-neutral-10 dark:text-neutral-92 max-w-lg">
                                    {TITLE}
                                </h1>
                            </div>
                            <div>
                                <FaucetButton />
                            </div>
                        </div>

                        <div className="w-full max-w-[500px] md:w-2/5">
                            <Bridge />
                        </div>
                    </div>
                </main>
            </div>

            <Footer />
        </>
    );
}
