#!/usr/bin/env python3

# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

import argparse
import glob, os
from gitignore_parser import parse_gitignore
from git import Repo

# Anything in ignored paths, including what is inside in case of a directory, will be ignored regardless of gitignore
IGNORED_PATHS = (
    'pnpm-lock.yaml', 
    'crates/sui-light-client/example_config/20873329.yaml', 
    'crates/sui-framework-snapshot/bytecode_snapshot/',
    'crates/iota-light-client/example_config/20873329.yaml', 
    'crates/iota-framework-snapshot/bytecode_snapshot/',
    'scripts/rename-to-iota/',
)

IGNORED_EXTENSIONS = ('svg', 'mv', 'png', 'jpg', 'jpeg', 'gif', 'wasm', 'errmap', 'bcs', 'chk', 'pdf', 'ai', 'mp3', 'wav', 'ico', 'ttf', 'otf', 'woff', 'woff2', 'mvsm')

# This mapping allows a simple replace mechanism for certain keywords, it's executed in order
REPLACE_MAP = (
    ('MystenLabs/sui', 'iotaledger/iota'),
    ('mystenlabs/sui', 'iotaledger/iota'),
    ('SUI', 'IOTA'),
    ('Sui', 'Iota'),
    ('sui', 'iota'),
    ('@mysten/', '@iota/'),
    ('MIST', 'NANOS'),
    ('Mist', 'Nanos'),
    ('mist', 'nanos')
)

REPLACE_DICT = dict(REPLACE_MAP)

# A list of things to not replace if they occur, especially words containing 'mist'
CASE_INSENSITIVE_IGNORE_REPLACE = ('mistakes', 'optimistic', 'optimist', 'optimistically', 'testsuite', 'test_suite', 'test-suite', 'lawsuit', 'suitable', 'mistic', 'minimist', 'headlessui')

# Make a mapping of to ignore file name replacements for use later on
skip_replace_mapping = []

for x in CASE_INSENSITIVE_IGNORE_REPLACE:
    for k, _ in REPLACE_MAP:
        if k in x:
            skip_replace_mapping.append((k, x))

WARN_ABOUT = (
    'suiprivkey',
)

'''
A function that renames all references to SUI and its terms to the corresponding IOTA alternative.
It's highly recommended to run a dry run and manually check the results before actually executing.

The path defaults to the full repo, unless specified otherwise
'''
def rename(path=None, dry_run=True, respect_gitignore=True, skip_filemod=False, use_git_mv=False):
    if not path:
        path = os.path.join(os.path.dirname(__file__), '../..')

    path = os.path.abspath(path)
    
    print('\n')
    print(80*'#')

    if dry_run:
        print('# Renaming dry run for for `%s`' % path)
        print('# It is highly recommended to pipe the output of this command to a log file for manual checks')
    else:
        print('# Renaming EXECUTION run for for `%s`' % path)
        print('# It is highly recommended to pipe the output of this command to a log file for manual checks')

    print(80*'#')

    print('# Gathering files to process...\n')

    ignored_paths = [os.path.join(path, f) for f in IGNORED_PATHS]
    gitignore_path = os.path.join(path, '.gitignore')
    
    gitignore = None

    # If there's a gitignore and it's respected, use it, otherwise never ignore
    if respect_gitignore:
        if os.path.exists('%s.rename' % gitignore_path):
            gitignore = parse_gitignore('%s.rename' % gitignore_path)
        elif os.path.exists(gitignore_path):
            gitignore = parse_gitignore(gitignore_path)

    if not gitignore:
        gitignore = lambda x: False
    
    to_process = []
    ignored = []
    suspicious = []

    for filename in glob.iglob(os.path.join(path, '**'), recursive=True):
        
        if gitignore(filename) or any([filename.startswith(p) for p in ignored_paths]) or any([filename.lower().endswith('.%s' % ext) for ext in IGNORED_EXTENSIONS]):
            ignored.append(filename)
        else:
            to_process.append(filename)

    if not skip_filemod:
        print("# Processing renames for %d files, this might take a while...\n" % len(to_process))

        for fn in to_process:
        
            fh = None
            if os.path.isfile(fn):
                fh = open(fn, 'r+')

            # Skip folders
            if not fh:
                continue

            print(fn)
            
            file_changed = False
            new_content = []
            for line, content in enumerate(fh.readlines()):
                orig_content = content
                content = content
                
                has_changes = False
                for kw, replacement in REPLACE_MAP:
                    if kw not in content:
                        continue
                    
                    ignored_word = False
                    for word in content.split():
                        if '://' in word and kw in word:
                            suspicious.append((fn, line, 'PART_OF_URL', content.strip(), word, '://'))

                        for ignore_part in CASE_INSENSITIVE_IGNORE_REPLACE:
                            if ignore_part.lower() in word.lower():
                                suspicious.append((fn, line, 'IGNORE_LIST', content.strip(), word, ignore_part))
                                ignored_word = True
                        
                        for ignore_part in WARN_ABOUT:
                            if ignore_part in word:
                                suspicious.append((fn, line, 'REPLACED_BUT_POSSIBLE_BREAKING_TESTS', content.strip(), word, ignore_part))

                    if ignored_word:
                        continue
                    
                    has_changes = True

                    content = content.replace(kw, replacement)

                if has_changes:
                    file_changed = True
                    print('# - File: %s (line: %s): \n#   < %s\n#   > %s\n' % (fn, line, orig_content.strip(), content.strip()))

                new_content.append(content)
            
            # Check for copyrights and add our own

            fix_copyright = False
            last_copyright_line = 0
            copyright_prefix = None

            for i, line in enumerate(new_content):
                if 'SPDX-License-Identifier' in line:
                    fix_copyright = True
                    last_copyright_line = i
                    if line.strip().startswith('#'):
                        copyright_prefix = '#'
                    elif line.strip().startswith('//'):
                        copyright_prefix = '//'
                    elif line.strip().startswith('f.write("//'):
                        copyright_prefix = '     f.write("//'
                    else:
                        print('# Copyright addition edge case for %s: `%s` (line %s)' % (fn, line, i))
                        suspicious.append((fn, i, 'COPYRIGHT_EDGE_CASE', line, '', ''))
                        fix_copyright = False
                        break
            
                if fn.endswith('.patch') or fn.endswith('.md') or fn.endswith('lint.rs'):
                    fix_copyright = False
                    break

                if 'Modifications Copyright (c) 2024 IOTA Stiftung' in line:
                    fix_copyright = False
                    break

            if fix_copyright:
                file_changed = True
                to_add = ['\n', '%s Modifications Copyright (c) 2024 IOTA Stiftung' % copyright_prefix, '\n', '%s SPDX-License-Identifier: Apache-2.0' % copyright_prefix, '\n']
                new_content = new_content[:last_copyright_line+1] + to_add + new_content[last_copyright_line+1:]
                
                prefix_line = last_copyright_line - 3
                if prefix_line < 0:
                    prefix_line = 0

                print('# Copyright added to %s: \n%s\n' % (fn, ''.join(new_content[prefix_line:last_copyright_line+10])))
        
            if file_changed:
                fcontent = ''.join(new_content)
                if not dry_run:
                    fh.seek(0)
                    fh.write(fcontent)
                    fh.truncate()

            fh.close()
            

        print("# Renaming files and directories as the final step...\n")
        
        # Display ignored non replacements due to ignore list or being suspicious

        if len(suspicious) > 0:
            print('# IGNORED REPLACEMENTS (marked as suspicious), might need manual replacement!\n')

            for sus in suspicious:
                fn, line, reason, content, word, extra = sus
                print('# - %s (line: %s):  %s (%s: %s `%s`)' % (fn, line, content.strip(), reason, word, extra))

    
    print('\n\n# Renaming files...\n\n')

    # Dirty loop to try several times in a row instead of implementing logic to deal with partial renames and their exceptions
    cnt = 0
    while True:

        cnt += 1
        if cnt > 10:
            break

        print('\n# Round #%d\n' %cnt) 
    
        renames = []
        errors = []

        for filename in glob.iglob(os.path.join(path, '**'), recursive=True):

            for kw, replacement in REPLACE_MAP:
                if kw in os.path.basename(filename):
                    
                    newfn = filename.replace(kw, replacement)
                    # Exceptions to rule out, like `testsuite`
                    for orig, case in skip_replace_mapping:
                        if case in filename:
                            reversal = REPLACE_DICT[orig]
                            newfn = newfn.replace(case.replace(orig, replacement), case)
                            print('# Edge case: %s (o: %s, case: %s) -> %s -> %s' % (filename, orig, case, replacement, newfn))
            
                    if filename != newfn:
                        renames.append((filename, newfn))

        # Make sure we sort it by shortest path first so that we don't end up with non-existing not-yet-renamed paths
        for fn, to_fn in sorted(renames, key=lambda x: len(x[0]), reverse=False):
            print('# `%s` -> `%s`' % (fn, to_fn))
            if not dry_run:
                try:
                    if use_git_mv:
                        repo = Repo.init(path)
                        git = repo.git()
                        git.mv((fn, to_fn))
                    else:
                        os.rename(fn, to_fn)
                except Exception as e:
                    errors.append((fn, to_fn))
            
        if len(errors) == 0:
            break

    print("\n# Ignored files: \n# - %s\n" % '\n# - '.join(ignored))

    print("\n\n # DONE!!!\n\n")


def main():
    parser = argparse.ArgumentParser(description='Port a folder recursively to from SUI based naming to IOTA based naming.')

    parser.add_argument('--path', '-p',  help='The path of the folder to process, defaults to the main folder of the project if left out')
    parser.add_argument('--execute', action='store_true', default=False, help='Flag to execute the replacements, if omitted it will just be a dry-run')
    parser.add_argument('--respect-gitignore', action='store_true', default=True, help='Respect what is set in .gitignore to be ignored, defaults to True')
    parser.add_argument('--skip-filemod', action='store_true', default=False, help='Skip file modification and only do the rename part')
    parser.add_argument('--use-git-mv', action='store_true', default=False, help='Use git mv instead of a normal mv, for same repo modifications')

    args = parser.parse_args()


    rename(path=args.path, dry_run=(not args.execute), respect_gitignore=args.respect_gitignore, skip_filemod=args.skip_filemod, use_git_mv=args.use_git_mv)


if __name__ == '__main__':
    main()
