resource "oci_core_vcn" "bot_vcn" {
  cidr_block     = "10.0.0.0/16"
  compartment_id = var.compartment_ocid
  display_name   = "bybit-bot-vcn"
  dns_label      = "bybitbotvcn"
}

resource "oci_core_internet_gateway" "bot_igw" {
  compartment_id = var.compartment_ocid
  vcn_id         = oci_core_vcn.bot_vcn.id
  display_name   = "bybit-bot-igw"
}

resource "oci_core_route_table" "bot_rt" {
  compartment_id = var.compartment_ocid
  vcn_id         = oci_core_vcn.bot_vcn.id
  display_name   = "bybit-bot-rt"

  route_rules {
    destination       = "0.0.0.0/0"
    destination_type  = "CIDR_BLOCK"
    network_entity_id = oci_core_internet_gateway.bot_igw.id
  }
}

resource "oci_core_security_list" "bot_sl" {
  compartment_id = var.compartment_ocid
  vcn_id         = oci_core_vcn.bot_vcn.id
  display_name   = "bybit-bot-sl"

  egress_security_rules {
    destination = "0.0.0.0/0"
    protocol    = "all"
  }

  ingress_security_rules {
    protocol = "6" # TCP
    source   = "0.0.0.0/0"
    tcp_options {
      max = 22
      min = 22
    }
  }
}

resource "oci_core_subnet" "bot_subnet" {
  cidr_block        = "10.0.1.0/24"
  compartment_id    = var.compartment_ocid
  vcn_id            = oci_core_vcn.bot_vcn.id
  display_name      = "bybit-bot-subnet"
  dns_label         = "botsubnet"
  route_table_id    = oci_core_route_table.bot_rt.id
  security_list_ids = [oci_core_security_list.bot_sl.id]
}
