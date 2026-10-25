#!/usr/bin/env python3
"""Read and write the yarimashow resources on MiniStack."""

import sys
import uuid

import boto3

ENDPOINT = "http://127.0.0.1:4566"
REGION = "ap-northeast-1"
PREFIX = "yarimashow"


def client(service: str):
    return boto3.client(
        service,
        region_name=REGION,
        endpoint_url=ENDPOINT,
        aws_access_key_id="test",
        aws_secret_access_key="test",
    )


def main() -> None:
    token = f"probe-{uuid.uuid4().hex}"
    body = token.encode()

    s3 = client("s3")
    bucket = f"{PREFIX}-images"
    s3.put_object(Bucket=bucket, Key="probe.txt", Body=body)
    got = s3.get_object(Bucket=bucket, Key="probe.txt")["Body"].read()
    if got != body:
        sys.exit(f"s3 body mismatch: {got!r}")

    ddb = client("dynamodb")
    table = f"{PREFIX}-notes"
    ddb.put_item(
        TableName=table,
        Item={
            "user_id": {"S": "probe"},
            "note_id": {"S": token},
            "body": {"S": token},
        },
    )
    item = ddb.get_item(
        TableName=table,
        Key={"user_id": {"S": "probe"}, "note_id": {"S": token}},
        ConsistentRead=True,
    )["Item"]
    if item["body"]["S"] != token:
        sys.exit(f"dynamodb item mismatch: {item!r}")

    sqs = client("sqs")
    queue_url = sqs.get_queue_url(QueueName=f"{PREFIX}-image-jobs")["QueueUrl"]
    sqs.send_message(QueueUrl=queue_url, MessageBody=token)
    deadline_messages = []
    for _ in range(10):
        batch = sqs.receive_message(
            QueueUrl=queue_url,
            MaxNumberOfMessages=10,
            WaitTimeSeconds=1,
        ).get("Messages", [])
        deadline_messages.extend(batch)
        if any(message["Body"] == token for message in batch):
            break
    else:
        sys.exit(f"sqs missing {token}: {deadline_messages!r}")

    ec2 = client("ec2")
    vpcs = ec2.describe_vpcs(
        Filters=[{"Name": "tag:Name", "Values": [PREFIX]}]
    )["Vpcs"]
    match = [vpc for vpc in vpcs if vpc["CidrBlock"] == "10.20.0.0/16"]
    if not match:
        sys.exit(f"vpc missing: {vpcs!r}")
    subnets = ec2.describe_subnets(
        Filters=[{"Name": "vpc-id", "Values": [match[0]["VpcId"]]}]
    )["Subnets"]
    if not any(subnet["CidrBlock"] == "10.20.1.0/24" for subnet in subnets):
        sys.exit(f"subnet missing: {subnets!r}")

    print(
        "ministack roundtrip ok",
        bucket,
        table,
        queue_url,
        match[0]["VpcId"],
    )


if __name__ == "__main__":
    main()
