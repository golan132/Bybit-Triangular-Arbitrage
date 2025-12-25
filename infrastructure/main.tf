module "networking" {
  source           = "./modules/networking"
  compartment_ocid = var.tenancy_ocid
}

module "computing" {
  source                 = "./modules/computing"
  compartment_ocid       = var.tenancy_ocid
  subnet_id              = module.networking.subnet_id
  ssh_public_key         = var.ssh_public_key
  instance_shape         = var.instance_shape
  instance_ocpus         = var.instance_ocpus
  instance_memory_in_gbs = var.instance_memory_in_gbs
  user_data_base64       = base64encode(file("${path.module}/cloud-init.yaml"))
}
