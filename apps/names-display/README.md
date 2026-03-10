# Names Display API - Docker Setup

This service generates SVG images for IOTA names with timestamps.

## Quick Start

To run the service from scratch with a single command:

```bash
docker-compose up --build
```

The service will be available at `http://localhost:3000`.

## API Usage

The service provides an endpoint to generate SVG images for IOTA names:

```
GET /:name/:timestamp
```

### Parameters

- `name`: A valid IOTA name in one of these formats:
  - `name@iota` (e.g., `test@iota`)
  - `name.iota` (e.g., `hello.iota`)
- `timestamp`: Unix timestamp in milliseconds

### Examples

```bash
# Generate SVG for "test.iota" name
curl "http://localhost:3000/test.iota/1234567890"

# Generate SVG for "hello@iota" name  
curl "http://localhost:3000/hello@iota/1609459200000"
```

### Response

- **Success (200)**: Returns SVG image with `Content-Type: image/svg+xml`
- **Error (500)**: Returns "Error generating SVG" text for invalid names or processing errors

## Valid Name Formats

✅ **Valid:**

- `test@iota`
- `hello.iota`
- `example@iota`
- `myname.iota`

❌ **Invalid:**

- `test` (missing domain)
- `test@example` (wrong domain)
- `test.com` (wrong TLD)

## Docker Configuration

The service is containerized with complete isolation - no local Node.js/pnpm installation required.

### Files

- `Dockerfile`: Single-stage build optimized for monorepo
- `docker-compose.yml`: Service orchestration with health checks
- `.dockerignore`: Build context optimization

### Architecture

- **Node.js 20** (Alpine Linux)
- **pnpm** package manager with workspace support
- **Fastify** web framework
- **TypeScript** compilation
- **Monorepo** structure with SDK dependencies

## Development Commands

### View logs

```bash
docker-compose logs -f
```

### Rebuild and restart

```bash
docker-compose up --build --force-recreate
```

### Stop service

```bash
docker-compose down
```

## Troubleshooting

1. **Empty reply from server**: Use valid IOTA name format (`name@iota` or `name.iota`)
2. **500 Internal Server Error**: Check name format and timestamp validity
3. **Container issues**: Check logs with `docker-compose logs`

4. **Access the service:**
   The service will be available at `http://localhost:3000`

5. **Test the API:**
   ```bash
   curl "http://localhost:3000/test/1234567890"
   ```

   This will return an SVG image with the name "test" and the timestamp "1234567890".

### API Endpoint

```
GET /:name/:timestamp
```

- `name`: The IOTA name to display
- `timestamp`: Unix timestamp for the date display

### Example Usage

```bash
# Generate SVG for name "myname" with current timestamp
curl "http://localhost:3000/myname/$(date +%s)" > myname.svg
```

### Docker Commands

- **Start the service:** `docker-compose up -d` (detached mode)
- **Stop the service:** `docker-compose down`
- **View logs:** `docker-compose logs -f names-display`
- **Rebuild:** `docker-compose up --build`

### Environment Variables

- `PORT`: Server port (default: 3000)
- `NODE_ENV`: Environment mode (default: production)

### Troubleshooting

1. **Build Issues:** Make sure you're running from the names-display directory and the monorepo dependencies are available.

2. **Port Conflicts:** If port 3000 is already in use, modify the port mapping in `docker-compose.yml`:
   ```yaml
   ports:
     - "3001:3000" # Maps host port 3001 to container port 3000
   ```

3. **Health Check Failures:** The service includes a health check that tests the API endpoint. If it fails, check the logs with `docker-compose logs names-display`.
