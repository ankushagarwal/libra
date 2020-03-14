// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Usage: ./safety-rules node.config

#![forbid(unsafe_code)]

use libra_config::config::NodeConfig;
use safety_rules::{simple_push_metrics::Metrics, Process};
use std::{env, process, sync::Arc};

fn main() {
    let cp = safety_rules::simple_push_metrics::CountersPusher {};
    let metrics: Arc<dyn Metrics + Send + Sync> =
        Arc::new(safety_rules::counters::SafetyRulesMetrics::new());
    let handle = cp.start(Arc::clone(&metrics));
    handle.join().unwrap();
    // let args: Vec<String> = env::args().collect();
    //
    // if args.len() != 2 {
    //     eprintln!("Incorrect number of parameters, expected a path to a config file");
    //     process::exit(1);
    // }
    //
    // let config = NodeConfig::load(&args[1]).unwrap_or_else(|e| {
    //     eprintln!("Unable to read provided config: {}", e);
    //     process::exit(1);
    // });
    //
    // libra_logger::Logger::new()
    //     .channel_size(config.logger.chan_size)
    //     .is_async(config.logger.is_async)
    //     .level(config.logger.level)
    //     .init();
    //
    // let mut service = Process::new(config);
    // service.start();
}
