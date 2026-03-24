import { lightTheme, WalletProvider } from '@iota/dapp-kit';

declare function YourApp(): JSX.Element;

const App = () => {
    return (
        <WalletProvider theme={lightTheme}>
            <YourApp />
        </WalletProvider>
    );
};

export default App;
