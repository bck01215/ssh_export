extern crate yaml_rust;
use ssh2::Session;
use std::env;
use std::net::TcpStream;
use yaml_rust::{Yaml, YamlLoader};
#[macro_use(lazy_static)]
extern crate lazy_static;
mod collections;
use prometheus::{register_int_gauge_vec, IntCounter, IntGaugeVec, Registry};
use warp::{Filter, Rejection, Reply};
lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    pub static ref INCOMING_REQUESTS: IntCounter =
        IntCounter::new("incoming_requests", "Incoming Requests").unwrap();
    pub static ref SERVICES: IntGaugeVec = register_int_gauge_vec!(
        "service_state",
        "Gauges for services",
        &["name", "instance"]
    )
    .unwrap();
    pub static ref DISK_FREE: IntGaugeVec = register_int_gauge_vec!(
        "disk_space_free",
        "Space free in bytes",
        &["device", "mount", "instance"]
    )
    .unwrap();
    pub static ref DISK_USED: IntGaugeVec = register_int_gauge_vec!(
        "disk_space_used",
        "Space used in bytes",
        &["device", "mount", "instance"]
    )
    .unwrap();
}
fn get_yaml_data() -> std::vec::Vec<yaml_rust::Yaml> {
    let config_file =
        env::var("SSH_CONFIG_YAML").unwrap_or("none".to_string()) + "/ssh_config.yaml";
    let yaml_data = std::fs::read_to_string(config_file).expect("ERROR: Could not read yaml");
    YamlLoader::load_from_str(&yaml_data).unwrap()
}
fn new_channel(i: usize) -> ssh2::Channel {
    let data = &(get_yaml_data())[0][i];
    let login = data["login"].as_hash().unwrap();
    let host = format!("{}:22", data["host"].as_str().unwrap());
    let tcp = TcpStream::connect(host).unwrap();
    let mut sess = Session::new().unwrap();
    sess.set_tcp_stream(tcp);
    sess.handshake().unwrap();

    sess.userauth_password(
        login[&Yaml::from_str("user")].as_str().unwrap(),
        login[&Yaml::from_str("password")].as_str().unwrap(),
    )
    .unwrap();
    assert!(sess.authenticated());
    let channel = sess.channel_session().unwrap();
    channel
}
fn register_service_checks(service: &str, host: &str, i: usize) {
    let result: i64 = collections::get_service_status(service, new_channel(i));
    SERVICES.with_label_values(&[service, host]).set(result);
}

fn register_disk_checks(host: &str, i: usize) {
    let result = collections::get_disks_status(new_channel(i));
    for i in result {
        DISK_FREE
            .with_label_values(&[&i.device, &i.mount, host])
            .set(i.free);
        DISK_USED
            .with_label_values(&[&i.device, &i.mount, host])
            .set(i.used);
    }
}

fn register_custom_metrics(i: usize) {
    let data = &(get_yaml_data())[0][i];
    let services = data["services"].as_vec().unwrap();
    let get_disks = data["check_disk_usage"].as_bool().unwrap();
    let host = data["host"].as_str().unwrap();

    for service in services {
        register_service_checks(service.as_str().unwrap(), host, i);
    }
    if get_disks == true {
        register_disk_checks(host, i);
    }
}

#[tokio::main]
async fn main() {
    let mut i: usize = 0;
    let datas = &(get_yaml_data())[0];
    REGISTRY
        .register(Box::new(INCOMING_REQUESTS.clone()))
        .expect("collector cannot be registered");
    for _data in datas.as_vec().unwrap() {
        register_custom_metrics(i);
        i += 1;
    }
    let metrics_route = warp::path!("metrics").and_then(metrics_handler);
    println!("Started on port 7222");
    warp::serve(metrics_route).run(([0, 0, 0, 0], 7222)).await;
}

async fn metrics_handler() -> Result<impl Reply, Rejection> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&REGISTRY.gather(), &mut buffer) {
        eprintln!("could not encode custom metrics: {}", e);
    };
    let mut res = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("custom metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
        eprintln!("could not encode prometheus metrics: {}", e);
    };
    let res_custom = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("prometheus metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    res.push_str(&res_custom);
    INCOMING_REQUESTS.inc();

    let datas = &(get_yaml_data())[0];
    let mut i: usize = 0;

    for data in datas.as_vec().unwrap() {
        let services = data["services"].as_vec().unwrap();
        let get_disks = data["check_disk_usage"].as_bool().unwrap();
        let host = data["host"].as_str().unwrap();
        for service in services {
            register_service_checks(service.as_str().unwrap(), host, i);
        }
        if get_disks == true {
            register_disk_checks(host, i);
        }
        i += 1;
    }
    Ok(res)
}
