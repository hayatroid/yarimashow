variable "region" {
  type    = string
  default = "ap-northeast-1"
}

variable "name_prefix" {
  type    = string
  default = "yarimashow"
}

variable "use_local_aws" {
  type        = bool
  default     = false
  description = "Send the AWS provider to MiniStack instead of AWS."
}

variable "aws_endpoint" {
  type        = string
  default     = null
  description = "MiniStack endpoint, for example http://127.0.0.1:4566."

  validation {
    condition     = !var.use_local_aws || var.aws_endpoint != null
    error_message = "aws_endpoint is required when use_local_aws is true."
  }
}
