import React from 'react'

interface PropTypes {
    status: 'success' | 'error',
    description: string,
    title: string,
    setShowPopup: React.Dispatch<React.SetStateAction<boolean>>,
    showPopup: boolean,
    digest:string
}

const Popup = ({ status, description, title, setShowPopup, showPopup,digest }: PropTypes) => {
    return (
        <div className="relative z-10" aria-labelledby="modal-title" role="dialog" aria-modal="true">
            <div className="fixed inset-0 dark:bg-gray-500/75 bg-gray-200/75 transition-opacity" aria-hidden="true"></div>
            <div className="fixed inset-0 z-10 w-screen overflow-y-auto">
                <div className="flex min-h-full items-center justify-center p-4 text-center">
                    <div className="relative transform overflow-hidden rounded-lg bg-white text-center shadow-xl transition-all sm:my-8 sm:w-full sm:max-w-md">
                        <div className="bg-[#ffffff] px-6 py-10">
                            <div className={`${status !== 'success' && 'hidden'} mx-auto mb-6 flex size-16 items-center justify-center rounded-full bg-green-100`}>
                                <svg className="size-8 text-green-500" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" aria-hidden="true">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                                </svg>
                            </div>
                            <div className={`${status === 'success' && 'hidden'} mx-auto mb-6 flex size-16 items-center justify-center rounded-full bg-red-100`}>
                                <svg className="size-6 text-red-600" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" aria-hidden="true" data-slot="icon">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z" />
                                </svg>
                            </div>
                            <h3 className="text-lg font-semibold text-gray-900 mb-2" id="modal-title">
                                {title}
                            </h3>
                            <p className="text-sm text-gray-500 mb-6">
                                {description}
                            </p>
                            <div>
                                <button 
                                    type='button'
                                    className="inline-flex cursor-pointer w-full border-none justify-center rounded-md bg-[#017195] px-4 py-2 text-sm my-2">
                                        <a 
                                            target='blank' 
                                            href={`https://explorer.rebased.iota.org/txblock/${digest}?network=testnet`}
                                            className='font-semibold text-white hover:text-black'
                                        >Check transaction on explorer</a>
                                </button>
                                <button
                                    type="button"
                                    className="inline-flex cursor-pointer w-full border-none justify-center rounded-md bg-[#017195] px-4 py-2 text-sm font-semibold text-white hover:text-black"
                                    onClick={() => setShowPopup(!showPopup)}
                                >
                                    Close
                                </button>
                            </div>
                            <div className='text-black'>
                                <p className='text-xs text-gray-700 mt-4'>Note: Transactions may take some time to appear on the explorer.</p>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>

    )
}

export default Popup