provider "aws" {
  region     = var.region
  access_key = var.use_local_aws ? "test" : null
  secret_key = var.use_local_aws ? "test" : null

  s3_use_path_style           = var.use_local_aws
  skip_credentials_validation = var.use_local_aws
  skip_metadata_api_check     = var.use_local_aws
  skip_requesting_account_id  = var.use_local_aws

  endpoints {
    s3       = var.aws_endpoint
    dynamodb = var.aws_endpoint
    sqs      = var.aws_endpoint
    ec2      = var.aws_endpoint
    sts      = var.aws_endpoint
    iam      = var.aws_endpoint
  }
}
