#!/bin/bash
TARGET_FOLDER="../.."
GRAPHQL_VERSION="2024.9"

# Parse command line arguments
# Usage:
# --target-folder <path>        - the target folder of the repository
# --graphql-version <version>   - the version (folder name) of the generated graphql files
while [ $# -gt 0 ]; do
    if [[ $1 == *"--graphql-version"* ]]; then
        GRAPHQL_VERSION=$2
    fi

    if [[ $1 == *"--target-folder"* ]]; then
        TARGET_FOLDER=$2
    fi
    shift
done

# Resolve the target folder
TARGET_FOLDER=$(realpath ${TARGET_FOLDER})

function print_step {
    echo -e "\e[32m$1\e[0m"
}

print_step "Building pnpm docker image..."
docker build -t pnpm-image -f ./Dockerfile .

print_step "Changing directory to ${TARGET_FOLDER}"
pushd ${TARGET_FOLDER}

# add cleanup hook to return to original folder
function cleanup {
    popd
}

trap cleanup EXIT

print_step "Generating open rpc schema..."
cargo run --package iota-open-rpc --example generate-json-rpc-spec -- record
if [ $? -ne 0 ]; then
    echo -e "\e[31mFailed to generate open rpc schema"
    exit 1
fi

echo -e "\e[32mGenerating graphql schema..."
cargo run --package iota-graphql-rpc generate-schema --file ./crates/iota-graphql-rpc/schema.graphql
if [ $? -ne 0 ]; then
    echo -e "\e[31mFailed to generate graphql schema"
    exit 1
fi

echo -e "\e[32mCopying generated graphlql schema to sdk folder..."
mkdir -p ./sdk/typescript/src/graphql/generated/${GRAPHQL_VERSION}
cp ./crates/iota-graphql-rpc/schema.graphql ./sdk/typescript/src/graphql/generated/${GRAPHQL_VERSION}/schema.graphql

print_step "Installing typescript sdk dependencies..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/typescript && pnpm i"

print_step "Installing graphql-transport dependencies..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/graphql-transport && pnpm i"

print_step "Updating open rpc schema..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/typescript && pnpm update-open-rpc-schema"

print_step "Updating graphql schema in 'client/types/generated.ts'..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/typescript && pnpm run generate-schema -c src/graphql/generated/${GRAPHQL_VERSION}/tsconfig.tada.json"

print_step "Updating graphql schema using gql.tada..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/typescript && pnpm generate-schema"

print_step "Generating graphql-transport typescript types..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/graphql-transport && pnpm codegen"
