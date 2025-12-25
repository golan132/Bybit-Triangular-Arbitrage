region = "ap-singapore-1"

# Instance Configuration (Free Tier Limits)
instance_shape         = "VM.Standard.A1.Flex"
instance_ocpus         = 4
instance_memory_in_gbs = 24

# Note: Sensitive variables must be provided via environment variables (TF_VAR_...)
# - tenancy_ocid
# - user_ocid
# - fingerprint
# - private_key_path
# - compartment_ocid
# - ssh_public_key