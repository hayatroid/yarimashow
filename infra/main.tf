resource "aws_s3_bucket" "images" {
  bucket = "${var.name_prefix}-images"
}

resource "aws_dynamodb_table" "notes" {
  name         = "${var.name_prefix}-notes"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "user_id"
  range_key    = "note_id"

  attribute {
    name = "user_id"
    type = "S"
  }

  attribute {
    name = "note_id"
    type = "S"
  }
}

resource "aws_sqs_queue" "image_jobs" {
  name = "${var.name_prefix}-image-jobs"
}

resource "aws_vpc" "main" {
  cidr_block = "10.20.0.0/16"

  tags = {
    Name = var.name_prefix
  }
}

resource "aws_subnet" "main" {
  vpc_id     = aws_vpc.main.id
  cidr_block = "10.20.1.0/24"

  tags = {
    Name = var.name_prefix
  }
}
