# Terraform Deployment

## Quick Start
```bash
cd infra/terraform
terraform init
terraform plan -var='jwt_secret=YOUR_SECRET_HERE'
terraform apply -var='jwt_secret=YOUR_SECRET_HERE'
```

## Firecracker
Terraform deploys via Docker, so it works with any Docker-compatible runtime:
- Docker: default
- Podman: set DOCKER_HOST=unix:///run/podman/podman.sock
- Firecracker (via kata-containers): no changes needed

## Kubernetes
For k8s, use the manifests in `../../k8s/` instead.
