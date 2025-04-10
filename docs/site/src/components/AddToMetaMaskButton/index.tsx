import React, { useState } from 'react';
import type { MetaMaskInpageProvider } from '@metamask/providers';
import { NetworkProps } from '../constant';

declare global {
  interface Window {
    ethereum?: MetaMaskInpageProvider;
  }
}

export function AddToMetaMaskButton(props: NetworkProps) {
  const [networkAdded, setNetworkAdded] = useState(false);

  async function addNetwork() {
    if (!window.ethereum) {
      alert(
        'MetaMask is not installed. Please install MetaMask and try again.',
      );
      return;
    }

    try {
      await window.ethereum.request({
        method: 'wallet_addEthereumChain',
        params: [props.evm],
      });
      setNetworkAdded(true);
      setTimeout(() => setNetworkAdded(false), 2500);
    } catch (error) {
      console.error(error);
      console.log('Error adding network: ' + error.message);
    }
  }

  return (
    <div className='flex flex-row gap-2'>
      <button
        className={`button button--primary button--md margin-bottom--md`}
        onClick={() => addNetwork()}
      >
        Add to MetaMask
      </button>
      {networkAdded && <span className='text-green-500 mt-1'>Network added *</span>}
    </div>
  );
}
