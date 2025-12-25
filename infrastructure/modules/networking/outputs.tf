output "subnet_id" {
  description = "The OCID of the subnet"
  value       = oci_core_subnet.bot_subnet.id
}
