#!/bin/bash
ROOT=$(git rev-parse --show-toplevel || realpath "$(dirname "$0")/../..")

run_id=$1
version=$2
server_url=${3:-"https://github.com/iotaledger"}
repository=${4:-"iota"}

checksums=${ROOT}/checksum.txt

macos_arm64_checksum=$(sed -En 's/^macos-arm64.*([0-9a-f]{64})$/\1/p' ${checksums})
linux_x86_64_checksum=$(sed -En 's/^linux-x86_64.*([0-9a-f]{64})$/\1/p' ${checksums})

git clone -b ${repository}-${version} ${server_url}/homebrew-tap homebrew-tap
cd homebrew-tap

formula=Formula/${repository}.rb
pr_description=$(echo \
    $(cat ${ROOT}/scripts/homebrew/pr_template.md) | \
    sed 's/{{server_url}}/${server_url}/g' | \
    sed 's/{{repository}}/${repository}/g' | \
    sed 's/{{version}}/${version}/g' )

cp -rf ${ROOT}/scripts/homebrew/template ${formula}

sed -i 's/{{version}}/${version}/g' ${formula}
sed -i 's/{{macos-arm64-checksum}}}/${macos_arm64_checksum}/g' ${formula}
sed -i 's/{{linux-x86_64-checksum}}}/${linux_x86_64_checksum}/g' ${formula}

title="Update brew formula for ${repository} ${version}"

git add ${formula}
git commit -m ${title}

gh pr create --base main --title ${title} --body-file ${pr_description}
