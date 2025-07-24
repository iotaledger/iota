# Kv Store Rest Api

This docker-compose configuration allows running the `iota-rest-kv` service. The service requires AWS credentials for proper execution.

## Configuration

The application requires a `yaml` file for its configuration.

A default configuration file is provided at `config/config.yaml`. This configuration can be customized based on your needs and is mounted into the container via Docker Compose.

```yaml
# Remote KV Store REST API config
#
# The Rest Api address
server-address: "0.0.0.0:3555"

instance-id: "iota"
column-family: "iota"
timeout-secs: 60
```

> [!NOTE]
> Following the application default credentials [guidelines](https://cloud.google.com/docs/authentication/application-default-credentials) the docker compose file uses the `GOOGLE_APPLICATION_CREDENTIALS` environment variable to authenticate with the Google Cloud API.

## Usage

#### 1. Build the required image

```shell
pushd <iota project directory>/docker/iota-rest-kv && ./build.sh && popd
```

#### 2. CD into the iota-rest-kv directory

```shell
cd <iota project directory>/dev-tools/iota-rest-kv
```

### 3. Start the Service

Run the container in detached mode:

```shell
docker compose up -d
```

> [!NOTE]
> Double check the rest api server port in the `config.yaml` and the exposed port on the `docker-compose.yaml` file, they should match.

### 4. Stop the Service

Stop and remove the container and associated resources:

```shell
docker compose down
```

## Local development

### Prerequisites

Follow the `README.md` file in the `iota-kvstore` directory.
