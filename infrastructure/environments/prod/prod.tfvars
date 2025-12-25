region = "ap-singapore-1"

# Instance Configuration (AMD E5 Flex - High Performance)
instance_shape         = "VM.Standard.E5.Flex"
instance_ocpus         = 1
instance_memory_in_gbs = 4

# Note: Sensitive variables must be provided via environment variables (TF_VAR_...)
# - tenancy_ocid
# - user_ocid
# - fingerprint
# - private_key_path
# - compartment_ocid
# - ssh_public_key