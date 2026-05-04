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
