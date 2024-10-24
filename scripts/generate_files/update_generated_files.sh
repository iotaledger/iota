#!/bin/bash
TARGET_FOLDER="../.."
SKIP_SPEC_GENERATION=false

# Parse command line arguments
# Usage:
# --target-folder <path>        - the target folder of the repository
# --skip-spec-generation        - skip the generation of the open rpc and graphql schema
while [ $# -gt 0 ]; do
    if [[ $1 == *"--target-folder"* ]]; then
        TARGET_FOLDER=$2
    fi

    if [[ $1 == *"--skip-spec-generation"* ]]; then
        SKIP_SPEC_GENERATION=true
    fi

    shift
done

# Resolve the target folder
TARGET_FOLDER=$(realpath ${TARGET_FOLDER})

function print_step {
    echo -e "\e[32m$1\e[0m"
}

print_step "Building pnpm docker image..."
docker build --build-arg USER_ID=$(id -u) --build-arg GROUP_ID=$(id -g) -t pnpm-image -f ./Dockerfile .

# Check if the docker image was built successfully
if [ $? -ne 0 ]; then
    echo -e "\e[31mFailed to build pnpm docker image"
    exit 1
fi

print_step "Changing directory to ${TARGET_FOLDER}"
pushd ${TARGET_FOLDER}

# add cleanup hook to return to original folder
function cleanup {
    popd
}

trap cleanup EXIT

# if the spec generation is not skipped, generate the spec
if [ "$SKIP_SPEC_GENERATION" = false ]; then
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
fi

print_step "Installing typescript sdk dependencies..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/typescript && pnpm i"

print_step "Installing graphql-transport dependencies..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/graphql-transport && pnpm i"

print_step "Updating open rpc schema..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/typescript && pnpm update-open-rpc-schema"

print_step "Create graphql schema in 'client/types/generated.ts'..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/typescript && pnpm update-graphql-schemas"

print_step "Updating graphql schema using gql.tada..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/typescript && pnpm generate-schema"

print_step "Generating graphql-transport typescript types..."
docker run --rm --name pnpm-image -v ${TARGET_FOLDER}:/home/node/app:rw --user $(id -u):$(id -g) pnpm-image sh -c "cd sdk/graphql-transport && pnpm codegen"
