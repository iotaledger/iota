export function makeDerivationPath(index: number) {
	// currently returns only Ed25519 path
	return `m/44'/4218'/${index}'/0'/0'`;
}