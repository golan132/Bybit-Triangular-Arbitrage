output "bot_public_ip" {
  value = module.computing.instance_public_ip
}

output "ssh_command" {
  value = "ssh opc@${module.computing.instance_public_ip}"
}
