import { describe, it, expect } from 'vitest';
import { getHname } from '../src/utils/getHname';
import { AccountsContractMethod, CoreContract } from '../src/enums/contracts.enums';

describe('getHname function', () => {
    it('should return a number for a string input', () => {
        const result = getHname('test');
        expect(typeof result).toBe('number');
    });

    it('should return consistent results for the same input', () => {
        const input = 'transferAllowanceTo';
        const result1 = getHname(input);
        const result2 = getHname(input);
        expect(result1).toBe(result2);
    });

    it('should handle empty strings', () => {
        const result = getHname('');
        expect(typeof result).toBe('number');
    });

    it('should generate different hashes for different inputs', () => {
        const result1 = getHname('deposit');
        const result2 = getHname('withdraw');
        expect(result1).not.toBe(result2);
    });

    it('should generate valid hnames for AccountsContractMethod', () => {
        const hnames = Object.values(AccountsContractMethod).map((method) => ({
            method,
            hname: getHname(method),
        }));

        // Check all hnames are unique
        const uniqueHnames = new Set(hnames.map((item) => item.hname));
        expect(uniqueHnames.size).toBe(hnames.length);

        const accountsHname = hnames.find((h) => h.method === 'transferAllowanceTo')?.hname;
        expect(accountsHname).toBe(603251617);

        console.table(hnames);
    });

    it('should generate valid hnames for CoreContract', () => {
        const hnames = Object.values(CoreContract).map((contract) => ({
            contract,
            hname: getHname(contract),
        }));

        const accountsHname = hnames.find((h) => h.contract === 'accounts')?.hname;
        expect(accountsHname).toBe(1011572226);

        console.table(hnames);
    });

    it('should generate hnames in the correct format', () => {
        // The function returns a number, but it's derived from a hex string
        // with 8 digits (32 bits)
        const result = getHname('test');
        const hexString = result.toString(16);
        expect(hexString.length).toBeLessThanOrEqual(8);
    });

    it('should match known test vectors', () => {
        const testVectors = [
            { input: 'accounts', expected: 1011572226 },
            { input: 'transferAllowanceTo', expected: 603251617 },
            { input: 'deposit', expected: 3184070701 },
            { input: 'withdraw', expected: 2647396161 },
            { input: 'balanceBaseToken', expected: 1284295951 },
        ];

        for (const vector of testVectors) {
            expect(getHname(vector.input)).toBe(vector.expected);
        }
    });
});
