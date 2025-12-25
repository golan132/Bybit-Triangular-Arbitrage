variable "compartment_ocid" {
  description = "OCI Compartment OCID"
  type        = string
}

variable "instance_shape" {
  description = "Compute instance shape"
  type        = string
  default     = "VM.Standard.A1.Flex"
}

variable "instance_ocpus" {
  description = "Number of OCPUs"
  type        = number
  default     = 1
}

variable "instance_memory_in_gbs" {
  description = "Memory in GBs"
  type        = number
  default     = 6
}

variable "ssh_public_key" {
  description = "SSH Public Key"
  type        = string
}

variable "subnet_id" {
  description = "Subnet OCID where the instance will be created"
  type        = string
}

variable "user_data_base64" {
  description = "Base64 encoded user data for cloud-init"
  type        = string
}
