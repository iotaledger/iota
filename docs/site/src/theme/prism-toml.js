(function (Prism) {

	// Matches keys in TOML (e.g., `key`, `"quoted key"`, `key.subkey`)
	var key = /(?:[A-Za-z0-9_-]+|"[^"\r\n]*")(?=\s*=)/;

	// Matches section headers like [package]
	var sectionHeader = /\[[^\]\r\n]+\]/;

	// Matches dates in TOML (e.g., `1979-05-27T07:32:00Z`, `1979-05-27 07:32:00`, `1979-05-27`)
	var date = /\d{4}-\d{2}-\d{2}(?:[Tt ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[Zz]|[-+]\d{2}:\d{2})?)?/;

	// Matches strings (basic and literal)
	var string = /"(?:[^"\\\r\n]|\\.)*"|'[^'\r\n]*'/;

	// Matches booleans
	var boolean = /\b(?:true|false)\b/;

	// Matches numbers, including hex, binary, octal, and floats
	var number = /[+-]?(?:0x[\da-fA-F]+|0b[01]+|0o[0-7]+|\d+(\.\d+)?([eE][+-]?\d+)?)/;

	// Matches inline comments starting with `#`
	var comment = /#.*/;

	Prism.languages.toml = {
		'section-header': {
			pattern: sectionHeader,
			alias: 'keyword' // Apply a distinct color for section headers by using the "keyword" alias
		},
		'comment': {
			pattern: comment,
			greedy: true
		},
		'key': {
			pattern: key,
			alias: 'attr-name'
		},
		'date': {
			pattern: date,
			alias: 'number'
		},
		'string': {
			pattern: string,
			greedy: true
		},
		'boolean': {
			pattern: boolean,
			alias: 'important'
		},
		'number': {
			pattern: number,
			alias: 'number'
		},
		'punctuation': /[=[\]{}.,]/ // Matches punctuation characters
	};

}(Prism));
