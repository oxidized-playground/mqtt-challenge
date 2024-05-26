#![allow(unused_imports)]
#![allow(clippy::single_component_path_imports)]


use core::cell::RefCell;
use core::ffi::{self, CStr};
use core::fmt::{self, Debug};
use core::sync::atomic::*;

use std::fs;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::os::fd::{AsRawFd, IntoRawFd};
use std::path::PathBuf;
use std::sync::{Condvar, Mutex};
use std::{env, sync::Arc, thread, time::*};

use anyhow::{bail, Result};

use async_io::{Async, Timer};
use esp_idf_svc::io::{EspIOError, Write};
use log::*;

use esp_idf_svc::sys::EspError;

use esp_idf_svc::hal::adc;
use esp_idf_svc::hal::delay;
use esp_idf_svc::hal::peripheral;
use esp_idf_svc::hal::prelude::*;

use esp_idf_svc::eventloop::*;
use esp_idf_svc::ipv4;
use esp_idf_svc::mqtt::client::*;
use esp_idf_svc::systime::EspSystemTime;
use esp_idf_svc::timer::*;
use esp_idf_svc::wifi::*;

#[allow(dead_code)]
const SSID: &str = "";
#[allow(dead_code)]
const PASS: &str = "";

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    #[allow(unused)]
    let peripherals = Peripherals::take().unwrap();
    #[allow(unused)]
    let pins = peripherals.pins;

    #[allow(unused)]
    let sysloop = EspSystemEventLoop::take()?;


    #[allow(clippy::redundant_clone)]
    #[allow(unused_mut)]
    let mut wifi = wifi(peripherals.modem, sysloop.clone())?;
    

    let sensor_value = 42;
    test_mqtt_client(sensor_value)?;

    for s in 0..3 {
        info!("Shutting down in {} secs", 3 - s);
        thread::sleep(Duration::from_secs(1));
    }

    drop(wifi);
    info!("Wifi stopped");

    Ok(())
}

fn test_mqtt_client(sensor_reading: u16) -> Result<EspMqttClient> {
    info!("About to start MQTT client");

    let conf = MqttClientConfiguration {
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        ..Default::default()
    };

    let (mut client, _) = EspMqttClient::new("mqtt://192.168.8.1:1883", &conf)?;

    info!("MQTT client started");
    client.publish(
        "MqttChallenge",
        QoS::AtMostOnce,
        false,
        create_creative_message(todo!("a sensor reading formatted here")),
    )?;

    info!("Published a hello message to topic \"rust-esp32-std-demo\"");

    Ok(client)
}

fn create_creative_message(message: String) -> String {
    todo!("Do we have some emoji?")
}

#[allow(dead_code)]
fn wifi(
    modem: impl peripheral::Peripheral<P = esp_idf_svc::hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,
) -> Result<Box<EspWifi<'static>>> {
    let mut esp_wifi = EspWifi::new(modem, sysloop.clone(), None)?;

    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration::default()))?;

    info!("Starting wifi...");

    wifi.start()?;

    info!("Scanning...");

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == SSID);

    let channel = if let Some(ours) = ours {
        info!(
            "Found configured access point {} on channel {}",
            SSID, ours.channel
        );
        Some(ours.channel)
    } else {
        info!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            SSID
        );
        None
    };

    wifi.set_configuration(&Configuration::Mixed(
        ClientConfiguration {
            ssid: todo!("Give a network name, should be defined a the top. See AccessPointConfiguration below on how to convert"),
            password: todo!("Give a password"),
            channel,
            ..Default::default()
        },
        AccessPointConfiguration {
            ssid: "aptest".try_into().unwrap(),
            channel: channel.unwrap_or(1),
            ..Default::default()
        },
    ))?;

    info!("Connecting wifi...");

    wifi.connect()?;

    info!("Waiting for DHCP lease...");

    wifi.wait_netif_up()?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

    Ok(Box::new(esp_wifi))
}

