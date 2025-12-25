terraform {
  backend "oci" {
    bucket    = "terraform-states"
    namespace = "axspeicvchxp" # Your Namespace
    key       = "bybit-bot/terraform.tfstate"
    region    = "ap-singapore-1"
  }
}
