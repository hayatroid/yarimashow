output "images_bucket" {
  value = aws_s3_bucket.images.bucket
}

output "notes_table" {
  value = aws_dynamodb_table.notes.name
}

output "image_jobs_queue_url" {
  value = aws_sqs_queue.image_jobs.url
}

output "vpc_id" {
  value = aws_vpc.main.id
}

output "subnet_id" {
  value = aws_subnet.main.id
}
