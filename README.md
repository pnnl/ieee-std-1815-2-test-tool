# 1815.2 Test Tool

The 1815.2 Test Tool is an [1815.2](https://standards.ieee.org/ieee/1815.2/7731/) profile editor with a reference control station and outstation. Control station and outstation developers can test their devices against the reference implementations in this repository.

## Features
- Connection via IP address to your outstation
- 

## Resources
- IEEE 1815.2: https://standards.ieee.org/ieee/1815.2/7731/
- Developer contribution guide: [CONTRIBUTING.md](CONTRIBUTING.md)
- [frontend/README.md](frontend/README.md) - Frontend documentation

## Quick Start: Docker

Install:

- Make: https://www.gnu.org/software/make/#download
- Docker: https://docs.docker.com/get-started/get-docker/

### Using Make (Recommended)

The project includes a Makefile for common operations:

```bash
make dev         # Build and start application at localhost:3000
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

## License

See [LICENSE](LICENSE)
