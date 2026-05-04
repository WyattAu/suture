# Deployment Guide

Suture Platform is a single static binary. Deploy it anywhere.

## Quick Start (Binary)

```bash
JWT_SECRET=$(openssl rand -hex 32) ./suture-platform --addr 0.0.0.0:8080
```

## Docker / Podman

```bash
docker run -d \
  --name suture \
  -p 8080:8080 \
  -e JWT_SECRET=$(openssl rand -hex 32) \
  -v suture-data:/data \
  ghcr.io/wyattau/suture-platform:latest
```

## Docker Compose

```bash
JWT_SECRET=$(openssl rand -hex 32) docker compose up -d
```

## Kubernetes (k3s, k8s, EKS, GKE, AKS)

```bash
kubectl apply -f k8s/
kubectl create secret generic suture-secrets \
  --from-literal=jwt-secret=$(openssl rand -hex 32)
```

## Terraform (Docker, Podman, Firecracker)

```bash
cd infra/terraform
terraform init
terraform apply -var='jwt_secret=YOUR_SECRET'
```

## Systemd (Bare Metal / VPS)

```bash
sudo JWT_SECRET=$(openssl rand -hex 32) ./suture-platform --addr 0.0.0.0:8080
# Or use the service installer:
sudo ./scripts/install-systemd.sh
```

## Nix

```bash
nix run github:WyattAu/suture
```

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `JWT_SECRET` | Yes | JWT signing secret (min 32 chars) |
| `STRIPE_KEY` | No | Stripe API key |
| `STRIPE_WEBHOOK_SECRET` | No | Stripe webhook signing secret |
| `PLATFORM_URL` | No | Public URL for CORS |
| `RUST_LOG` | No | Log level (default: info) |
| `CORS_ORIGINS` | No | Comma-separated allowed origins |
