#!/usr/bin/env python3
import os
import re
from pathlib import Path

CONTENT_DIR = Path(__file__).resolve().parent.parent.parent / 'content'

# Regex to find [text](target) where target contains '::' and doesn't start with http/https
link_rx = re.compile(r'\[([^\]]*)\]\(((?!https?://)[^)]*::[^)]*)\)')

# Regex to find <super::...> or specific invalid JSX tags that cause port URL parsing errors
autolink_rx = re.compile(r'<(super::[^>]*)>')

def sanitize_file(file_path):
    try:
        content = file_path.read_text(encoding='utf-8')
    except Exception as e:
        print(f"Skipping {file_path}: {e}")
        return

    original = content

    # 1. Fix the specific NotarizationClient.mdupdatemetadata typo
    content = content.replace('NotarizationClient.mdupdatemetadata', 'NotarizationClient.md#updatemetadata')

    # 2. Fix general markdown links [text](some::target) -> `text`
    # (or we can just keep the text without link format)
    content = link_rx.sub(r'\1', content)

    # 3. Fix autolinks <some::target> -> `some::target`
    content = autolink_rx.sub(r'`\1`', content)

    if content != original:
        print(f"Sanitized: {file_path.relative_to(CONTENT_DIR.parent)}")
        file_path.write_text(content, encoding='utf-8')

def main():
    print(f"Sanitizing docs in {CONTENT_DIR}...")
    for root, _, files in os.walk(CONTENT_DIR):
        for file in files:
            if file.endswith('.md') or file.endswith('.mdx'):
                sanitize_file(Path(root) / file)
    print("Sanitization complete.")

if __name__ == '__main__':
    main()
