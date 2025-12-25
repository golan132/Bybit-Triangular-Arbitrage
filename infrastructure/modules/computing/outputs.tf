output "instance_public_ip" {
  description = "Public IP of the instance"
  value       = oci_core_instance.bot_instance.public_ip
}
