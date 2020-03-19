// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use anyhow::{bail, format_err, Result};
use libra_logger::*;
use reqwest::{self, Url};
use rusoto_autoscaling::{
    AutoScalingGroupNamesType, Autoscaling, AutoscalingClient, SetDesiredCapacityType,
};
use rusoto_core::Region;
use rusoto_ec2::{DescribeInstancesRequest, Ec2, Ec2Client};
use rusoto_ecr::EcrClient;
use rusoto_ecs::EcsClient;
use rusoto_s3::{PutObjectRequest, S3Client, S3};
use rusoto_sts::WebIdentityProvider;
use std::{fs::File, io::Read, thread, time::Duration};
use util::retry;

#[derive(Clone)]
pub struct Aws {
    workspace: String,
    ec2: Ec2Client,
    ecr: EcrClient,
    ecs: EcsClient,
}

impl Aws {
    pub fn new(k8s: bool) -> Self {
        let ec2 = Ec2Client::new(Region::UsWest2);
        let workspace = if k8s {
            "k8s".to_string()
        } else {
            discover_workspace(&ec2)
        };
        Self {
            workspace,
            ec2,
            ecr: EcrClient::new(Region::UsWest2),
            ecs: EcsClient::new(Region::UsWest2),
        }
    }

    pub fn ec2(&self) -> &Ec2Client {
        &self.ec2
    }

    pub fn ecr(&self) -> &EcrClient {
        &self.ecr
    }

    pub fn ecs(&self) -> &EcsClient {
        &self.ecs
    }

    pub fn workspace(&self) -> &String {
        &self.workspace
    }

    pub fn region(&self) -> &str {
        Region::UsWest2.name()
    }
}

impl Default for Aws {
    fn default() -> Self {
        Self::new(false)
    }
}

fn discover_workspace(ec2: &Ec2Client) -> String {
    let instance_id = current_instance_id();
    let mut attempt = 0;
    loop {
        let result = match ec2
            .describe_instances(DescribeInstancesRequest {
                filters: None,
                max_results: None,
                dry_run: None,
                instance_ids: Some(vec![instance_id.clone()]),
                next_token: None,
            })
            .sync()
        {
            Ok(result) => result,
            Err(e) => {
                attempt += 1;
                if attempt > 10 {
                    panic!("Failed to discover workspace");
                }
                error!(
                    "Transient failure when discovering workspace(attempt {}): {}",
                    attempt, e
                );
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        };
        let reservation = result
            .reservations
            .expect("discover_workspace: no reservations")
            .remove(0)
            .instances
            .expect("discover_workspace: no instances")
            .remove(0);
        let tags = reservation.tags.expect("discover_workspace: no tags");
        for tag in tags.iter() {
            if tag.key == Some("Workspace".to_string()) {
                return tag
                    .value
                    .as_ref()
                    .expect("discover_workspace: no tag value")
                    .to_string();
            }
        }
        panic!(
            "discover_workspace: no workspace tag. Instance id: {}, tags: {:?}",
            instance_id, tags
        );
    }
}

fn current_instance_id() -> String {
    let client = reqwest::blocking::Client::new();
    let url = Url::parse("http://169.254.169.254/1.0/meta-data/instance-id");
    let url = url.expect("Failed to parse metadata url");
    let response = client.get(url).send();
    let response = response.expect("Metadata request failed");
    response.text().expect("Failed to parse metadata response")
}

pub fn autoscale(desired_capacity: i64, asg_name: &str) -> Result<()> {
    let set_desired_capacity_type = SetDesiredCapacityType {
        auto_scaling_group_name: asg_name.to_string(),
        desired_capacity,
        honor_cooldown: Some(false),
    };
    let credentials_provider = WebIdentityProvider::from_k8s_env();
    let dispatcher = rusoto_core::HttpClient::new().expect("failed to create request dispatcher");
    let asc = AutoscalingClient::new_with(dispatcher, credentials_provider, Region::UsWest2);
    asc.set_desired_capacity(set_desired_capacity_type)
        .sync()
        .map_err(|e| format_err!("set_desired_capacity failed: {:?}", e))?;
    retry::retry(retry::fixed_retry_strategy(10_000, 30), || {
        let auto_scaling_group_names_type = AutoScalingGroupNamesType {
            auto_scaling_group_names: Some(vec![asg_name.to_string()]),
            max_records: Some(desired_capacity),
            next_token: None,
        };
        let asgs = asc
            .describe_auto_scaling_groups(auto_scaling_group_names_type)
            .sync()?;
        if asgs.auto_scaling_groups.len() < 1 {
            bail!("");
        }
        let asg = &asgs.auto_scaling_groups[0];
        let count = asg
            .instances
            .clone()
            .ok_or_else(|| format_err!("instances not found for auto_scaling_group"))?
            .iter()
            .filter(|instance| instance.lifecycle_state == "InService")
            .count() as i64;
        if count < desired_capacity {
            info!(
                "Waiting for scale-up to complete. Current size: {}, Desired Size: {}",
                count, desired_capacity
            );
            info!(
                "Waiting for scale-up to complete. Current size: {}, Desired Size: {}",
                count, desired_capacity
            );
        }
        Ok(())
    })
}

pub fn upload_to_s3(
    local_filename: &str,
    bucket: &str,
    dest_filename: &str,
    content_type: Option<String>,
) -> Result<()> {
    let mut f = File::open(local_filename).unwrap();
    let mut contents: Vec<u8> = Vec::new();
    match f.read_to_end(&mut contents) {
        Err(e) => bail!("Error opening file to send to S3: {}", e),
        Ok(_) => {
            let req = PutObjectRequest {
                bucket: bucket.to_owned(),
                key: dest_filename.to_owned(),
                body: Some(contents.into()),
                content_type,
                ..Default::default()
            };
            S3Client::new(Region::UsWest2)
                .put_object(req)
                .sync()
                .map_err(|e| format_err!("Failed to upload to S3: {:?}", e))
                .map(|_| ())
        }
    }
}
