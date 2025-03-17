#!/bin/bash

ROOT=$(git rev-parse --show-toplevel || realpath "$(dirname "$0")/../..")

tag=$1
org=${2:-"iotaledger"}
repository=${3:-"iota"}

if [ -z "$GH_TOKEN" ]; then
    echo "Environment variable GH_TOKEN must be set!"
    exit 1
fi

# Remove leading `v` from tag
version=$(echo "${tag}" | sed -En "s|v?(.+)|\1|p")
server_url="https://github.com/${org}"
auth_url="https://${GH_TOKEN}@github.com/${org}"

checksums=${ROOT}/checksum.txt

macos_arm64_checksum=$(sed -En 's/^.*macos-arm64.*([0-9a-f]{64})$/\1/p' "${checksums}")
linux_x86_64_checksum=$(sed -En 's/^.*linux-x86_64.*([0-9a-f]{64})$/\1/p' "${checksums}")

git clone "${auth_url}"/homebrew-tap homebrew-tap
cd homebrew-tap || exit
git checkout -b "${repository}"-"${tag}"

formula="Formula/${repository}.rb"
pr_template=$(cat "${ROOT}"/scripts/homebrew/pr_template.md)

pr_description=$(echo "${pr_template}" | \
    sed "s|{{server_url}}|${server_url}|g" | \
    sed "s|{{repository}}|${repository}|g" | \
    sed "s|{{tag}}|${tag}|g")

cp -rf "${ROOT}"/scripts/homebrew/template "${formula}"

sed -i -e "s|{{version}}|${version}|g" "${formula}"
sed -i -e "s|{{macos-arm64-checksum}}|${macos_arm64_checksum}|g" "${formula}"
sed -i -e "s|{{linux-x86_64-checksum}}|${linux_x86_64_checksum}|g" "${formula}"

title="Update brew formula for ${repository} ${tag}"

git config user.name "IOTA Foundation"
git config user.email "info@iota.org"

git add "${formula}"
git commit -m "${title}"
git push --set-upstream origin "${repository}"-"${tag}"

gh pr create --base main --title "${title}" --body "${pr_description}"
