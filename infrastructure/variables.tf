variable "tenancy_ocid" {}
variable "user_ocid" {}
variable "fingerprint" {}
variable "private_key" {}
variable "region" {}
variable "ssh_public_key" {}

variable "instance_shape" {
  default = "VM.Standard.A1.Flex"
}
variable "instance_ocpus" {
  default = 1
}
variable "instance_memory_in_gbs" {
  default = 6
}
