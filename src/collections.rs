use std::io::prelude::*;

pub struct Disk {
    pub device: String,
    pub mount: String,
    pub used: i64,
    pub free: i64,
}

pub fn get_service_status(service: &str, mut channel: ssh2::Channel) -> i64 {
    let mut string = "systemctl status ".to_owned();
    string.push_str(service);
    string.push_str(" > /dev/null; echo $?");
    channel.exec(&string).unwrap();
    let mut s = String::new();
    channel.read_to_string(&mut s).unwrap();
    let state: i64;
    let resp: i64 = s.chars().nth(0).unwrap().to_string().parse().unwrap();
    if resp == 0 {
        state = 1;
    } else {
        state = 0;
    }
    channel.wait_close().expect("Could not close session");
    state
}

pub fn get_disks_status(mut channel: ssh2::Channel) -> Vec<Disk> {
    channel
        .exec("df | sed -e /^Filesystem/d| awk '{print $1,$6,$3,$4}'")
        .unwrap();
    let mut s = String::new();
    channel.read_to_string(&mut s).unwrap();
    channel.wait_close().expect("Could not close session");
    string_to_disks(s)
}

fn string_to_disks(data: String) -> Vec<Disk> {
    let mut disks: Vec<Disk> = Vec::new();
    for i in data.lines() {
        let contents: Vec<String> = i
            .split(" ")
            .into_iter()
            .map(|s| s.parse().unwrap())
            .collect();
        let disk = Disk {
            device: contents[0].to_string(),
            mount: contents[1].to_string(),
            used: contents[2].to_string().parse().unwrap(),
            free: contents[3].to_string().parse().unwrap(),
        };
        disks.push(disk);
    }
    disks
}
