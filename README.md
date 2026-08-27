# 1815.2 Test Tool

The 1815.2 Test Tool is an [1815.2](https://standards.ieee.org/ieee/1815.2/7731/) profile editor with a reference control station and outstation. Control station and outstation developers can test their devices against the reference implementations in this repository.

For developer setup and workflows, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Quick Start

Install:

- Make
- Docker: https://docs.docker.com/get-started/get-docker/

### Using Make (Recommended)

The project includes a Makefile for common operations:

```bash
make dev         # Build and start application
make dev-logs    # View logs
make dev-down    # Stop application

# Cleanup
make clean        # Stop all containers and remove volumes
make help         # Show all available commands
```

Access the Profile Editor at:
- **Development**: http://localhost:3000

Your profile data will be saved to `./data/working/`.

To stop:

```bash
make dev-down
# or
docker compose -f docker-compose.dev.yml down
```

### Local Development

#### Frontend Profile Editor

For local development with hot reload:

```bash
cd frontend
npm install
npm run dev
```

Access at http://localhost:3000

#### Backend (Rust)

**Web server:**
```bash
cargo run -p web_server
```

**Reference stations (for compliance testing):**
```bash
make run-reference-stations
```

## Documentation

- [CONTRIBUTING.md](CONTRIBUTING.md) - Contribution and development guide
- [frontend/README.md](frontend/README.md) - Frontend documentation
- [frontend/EXCEL_TEMPLATE.md](frontend/EXCEL_TEMPLATE.md) - Excel template format

## License

See [LICENSE](LICENSE)
