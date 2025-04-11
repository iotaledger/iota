import React, { useState } from 'react';
import {
  useSignAndExecuteTransaction,
} from '@iota/dapp-kit';
import clsx from 'clsx';
import { useConnectWallet, useWallets } from '@iota/dapp-kit';
import { handleChallengeSubmit } from "../../utils/ctf-utils"
import PopIn from './pop-in';

interface ChallengeVerifierProps {
  expectedObjectType: string;
  nftName: string;
  challengeNumber: string
}

const ChallengeVerifier: React.FC<ChallengeVerifierProps> = ({
  expectedObjectType,
  nftName,
  challengeNumber,
}) => {
  const [inputText, setInputText] = useState('');
  const [coins, setCoins] = useState<string | null>(null);
  const [showPopIn, setShowPopIn] = useState<boolean>(false);
  const [loading, setLoading] = useState(false);
  const [response, setResponse] = useState<{
    status: 'success' | 'error';
    description: string;
    title: string;
    digest: string;
  }>({
    status: 'success',
    description: '',
    title: '',
    digest: ''
  });

  const wallets = useWallets();
  const { mutate } = useConnectWallet();
  const { mutate: signAndExecuteTransaction} = useSignAndExecuteTransaction();
  const handleSubmit = () => {
    handleChallengeSubmit({
      inputText,
      expectedObjectType,
      nftName,
      challengeNumber,
      wallets,
      mutate,
      signAndExecuteTransaction,
      setLoading,
      setCoins,
      setResponse,
      setShowPopIn,
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
        {<p className={`text-red-500 mb-0 mt-1 text-sm ${response.description!=='' ? 'visible' : 'invisible'}`}>{response.description}</p>}
        <button 
          onClick={handleSubmit} 
          className={`${clsx("button", { "button-disabled": inputText=='' || loading })} min-w-28 mt-4`}
          disabled={inputText=='' || loading}
        >
          {loading ? 'Loading...' : 'Submit Your Challenge'}
        </button>
        {coins && <p className='mb-0 mt-2 p-2 bg-[#353535] rounded-md'>{coins}</p>}
      </div>
      {showPopIn && (
        <PopIn
            status={response.status}
            description={response.description}
            title={response.title}
            setShowPopIn={setShowPopIn}
            digest={response.digest}
            showPopIn={showPopIn}
        />
      )}
    </div>
  );
};

export default ChallengeVerifier;
