import React, { useState } from 'react';
import {
  useCurrentWallet,
  useSignAndExecuteTransaction,
} from '@iota/dapp-kit';
import clsx from 'clsx';
import { useConnectWallet, useWallets } from '@iota/dapp-kit';
import PopIn from './pop-in';
import { handleMintLeapFrogSubmit } from "../../utils/ctf-utils"

const MintLeapFrogNFT: React.FC = () => {
  const { currentWallet, connectionStatus } = useCurrentWallet();
  const [nft, setNFT] = useState({
    name:'',
    description:'',
    url:'',
    address: currentWallet?.address || '',
  });
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
  const { mutate: signAndExecuteTransaction } = useSignAndExecuteTransaction();
  const handleSubmit = () => {
    handleMintLeapFrogSubmit({
      nft,
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
    <div className='bg-[#e5e5e5] dark:bg-[#1e1e1e] p-4 rounded-lg'>
      <h3>Claim your Leap Frog NFT</h3>
      <div className="flex flex-col items-start">
      <label htmlFor="name">Name <span className="red">*</span></label>
      <input
        type="text"
        value={nft.name}
        onChange={(e) => setNFT((prevState) => ({
          ...prevState,
          name:e.target.value
        }))}
        placeholder="Enter name"
        className="input-field mb-4"
      />
      <label htmlFor="description">Description <span className="red">*</span></label>
      <input
        type="text"
        value={nft.description}
        onChange={(e) => setNFT((prevState) => ({
          ...prevState,
          description:e.target.value
        }))}
        placeholder="Enter description"
        className="input-field mb-4"
      />
      <label htmlFor="URL">URL <span className="red">*</span></label>
      <input
        type="text"
        value={nft.url}
        onChange={(e) => setNFT((prevState) => ({
          ...prevState,
          url:e.target.value
        }))}
        placeholder="Enter url"
        className="input-field mb-4"
      />
      <button
        onClick={handleSubmit}
        className={`${clsx('button', { 'button-disabled': loading })} p-3 min-w-[12.5rem]`}
        disabled={loading|| coins==="Congratulations! You have successfully completed this level!" ||  nft.name==='' || nft.description==='' || nft.url===''}
      >
        {loading ? 'Loading...' : 'Submit Challenge'}
      </button>
      </div>
      <div className="flex items-center">
      {coins && !loading && <pre className="mt-2 mb-0 p-3">{coins}</pre>}
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

export default MintLeapFrogNFT;
