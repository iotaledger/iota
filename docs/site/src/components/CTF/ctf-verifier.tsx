import React, { useState, useMemo } from 'react';
import {
  IotaClientProvider,
  useSignAndExecuteTransaction,
  WalletProvider,
} from '@iota/dapp-kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { getFullnodeUrl } from '@iota/iota-sdk/client';
import clsx from 'clsx';
import { useConnectWallet, useWallets } from '@iota/dapp-kit';
import Popup from './popup';
import { handleChallengeSubmit } from "../../utils/ctf-utils"

interface ChallengeVerifierProps {
  expectedObjectType: string;
  nftName: string;
  challengeNumber: string
}

const NETWORKS = {
  testnet: { url: getFullnodeUrl('testnet') },
};

const ChallengeVerifier: React.FC<ChallengeVerifierProps> = ({
  expectedObjectType,
  nftName,
  challengeNumber,
}) => {
  const [inputText, setInputText] = useState('');
  const [coins, setCoins] = useState<string | null>(null);
  const [showPopup, setShowPopup] = useState<boolean>(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<{
    status: 'success' | 'error';
    description: string;
    title: string;
  }>({
    status: 'success',
    description: '',
    title: '',
  });

  const wallets = useWallets();
  const { mutate } = useConnectWallet();
  const { mutate: signAndExecuteTransaction} = useSignAndExecuteTransaction();
  const [digest,setDigest] = useState<string>('');
  const handleSubmit = async () => {
   await handleChallengeSubmit({
      inputText,
      expectedObjectType,
      nftName,
      challengeNumber,
      wallets,
      mutate,
      signAndExecuteTransaction,
      setLoading,
      setCoins,
      setError,
      setShowPopup,
      setDigest
    });
  };

  return (
    <div className='bg-[#0000001a] dark:bg-[#1e1e1e] p-5 rounded-lg'>
      <h3>Claim your NFT reward</h3>
      <label >Flag Id <span className="red">*</span></label>
      <div className='flex flex-col flex-wrap items-start mt-1'>
        <input
          type="text"
          value={inputText}
          onChange={(e) => setInputText(e.target.value)}
          placeholder="Enter Flag Object Id"
          className="input-field"
        />
        {<p className={`text-red-500 mb-0 mt-1 text-sm ${error.description!=='' ? 'visible' : 'invisible'}`}>{error.description}</p>}
        <button 
          onClick={handleSubmit} 
          className={`${clsx("button", { "button-disabled": inputText=='' || loading })} min-w-28 mt-4`}
          disabled={inputText=='' || loading}
        >
          {loading ? 'Loading...' : 'Submit Your Challenge'}
        </button>
        {coins && <p className='mb-0 py-3 px-2 bg-[#353535] rounded-md'>{coins}</p>}
      </div>
    </div>
  );
};

const withProviders = (Component: React.FC<ChallengeVerifierProps>) => {
  return ({ expectedObjectType }: ChallengeVerifierProps) => {
    if (typeof window === 'undefined') {
      return null;
    }

    const queryClient = useMemo(() => new QueryClient(), []);

    return (
      <QueryClientProvider client={queryClient}>
        <IotaClientProvider networks={NETWORKS}>
          <WalletProvider>
            <Component
              expectedObjectType={expectedObjectType}
              challengeNumber="1"
              nftName="Checkin"
            />
          </WalletProvider>
        </IotaClientProvider>
      </QueryClientProvider>
    );
  };
};

export default withProviders(ChallengeVerifier);