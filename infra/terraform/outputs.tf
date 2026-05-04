output "url" {
  description = "Access URL"
  value       = "http://localhost:${var.port}"
}

output "container_id" {
  description = "Docker container ID"
  value       = docker_container.suture.id
}
