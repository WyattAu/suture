terraform {
  required_version = ">= 1.0"
  required_providers {
    docker = {
      source  = "kreuzwerker/docker"
      version = "~> 3.0"
    }
  }
}

variable "image" {
  description = "Container image to deploy"
  type        = string
  default     = "ghcr.io/wyattau/suture-platform:latest"
}

variable "port" {
  description = "Port to expose"
  type        = number
  default     = 8080
}

variable "jwt_secret" {
  description = "JWT signing secret (min 32 chars)"
  type        = string
  sensitive   = true
}

variable "stripe_key" {
  description = "Stripe API key"
  type        = string
  default     = ""
  sensitive   = true
}

variable "stripe_webhook_secret" {
  description = "Stripe webhook signing secret"
  type        = string
  default     = ""
  sensitive   = true
}

variable "data_dir" {
  description = "Host path for persistent data"
  type        = string
  default     = "/var/lib/suture"
}

variable "platform_url" {
  description = "Public URL of the platform"
  type        = string
  default     = ""
}

variable "environment" {
  description = "Additional environment variables"
  type        = map(string)
  default     = {}
}

locals {
  container_name = "suture-platform"
  env = merge({
    SUTURE_DATA_DIR       = "/data"
    RUST_LOG              = "info"
    JWT_SECRET            = var.jwt_secret
    STRIPE_KEY            = var.stripe_key
    STRIPE_WEBHOOK_SECRET = var.stripe_webhook_secret
    PLATFORM_URL          = var.platform_url
  }, var.environment)
}

provider "docker" {
  host = "unix:///var/run/docker.sock"
}

resource "docker_image" "suture" {
  name = var.image
}

resource "docker_container" "suture" {
  name  = local.container_name
  image = docker_image.suture.image_id
  restart = "unless-stopped"

  ports {
    internal = var.port
    external = var.port
  }

  env = local.env

  volumes {
    host_path      = var.data_dir
    container_path = "/data"
  }

  healthcheck {
    test     = ["CMD", "curl", "-f", "http://localhost:${var.port}/health"]
    interval = "30s"
    timeout  = "5s"
    retries  = 3
  }
}
