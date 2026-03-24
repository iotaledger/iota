import { lightTheme, WalletProvider, type ThemeVars } from '@iota/dapp-kit';

declare function YourApp(): JSX.Element;

// Example custom themes -- in a real app these would be in their own file
const darkTheme: ThemeVars = { ...lightTheme };
const pinkTheme: ThemeVars = { ...lightTheme };

const App = () => {
    return (
        <WalletProvider
            theme={[
                {
                    // default to light theme
                    variables: lightTheme,
                },
                {
                    // Adds theme inside a media query
                    mediaQuery: '(prefers-color-scheme: dark)',
                    variables: darkTheme,
                },
                {
                    // prefixes the theme styles with the given selector
                    // this allows controlling the theme by adding a class to the body
                    selector: '.pink-theme',
                    variables: pinkTheme,
                },
            ]}
        >
            <YourApp />
        </WalletProvider>
    );
};

export default App;
