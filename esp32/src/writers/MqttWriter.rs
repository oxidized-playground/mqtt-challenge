use heapless::String;
use core::fmt::Write;

use rust_mqtt::{
    client::{client::MqttClient, client_config::ClientConfig},
    packet::v5::reason_codes::ReasonCode,
    utils::rng_generator::CountingRng,
};

#[derive(Debug, PartialEq)]
pub enum MqttError {
    NetworkError,
    OtherError(String<32>)
}

pub(crate) struct MqttWriter<'a, T: embedded_svc::io::asynch::Read + embedded_svc::io::asynch::Write> {
    client: MqttClient<'a, T, 5, CountingRng>,
}

impl<'a, T: embedded_svc::io::asynch::Read + embedded_svc::io::asynch::Write> MqttWriter<'a, T> {
    pub fn new(client_id: &'a str,
               socket: T,
               write_buffer: &'a mut [u8],
               buffer_len: usize,
               recv_buffer: &'a mut [u8],
               recv_buffer_len: usize) -> MqttWriter<'a, T> {

        let mut config = ClientConfig::new(
            rust_mqtt::client::client_config::MqttVersion::MQTTv5,
            CountingRng(20000),
        );
        config.add_max_subscribe_qos(rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS1);
        config.add_client_id(client_id);
        config.max_packet_size = 100;

        let client =
            MqttClient::<_, 5, _>::new(socket, write_buffer, buffer_len, recv_buffer, recv_buffer_len, config);

        Self {client}
    }

    pub async fn connect(self: &mut Self) -> Result<(), MqttError> {
        match self.client.connect_to_broker().await {
            Ok(()) => {Ok(()) }
            Err(mqtt_error) => match mqtt_error {
                ReasonCode::NetworkError => {
                    Err(MqttError::NetworkError)
                }
                _ => {
                    let mut error_string: String<32> = String::new();
                    write!(error_string, "Other MQTT Error: {:?}", mqtt_error).expect("Mqtt error format failed!");
                    Err(MqttError::OtherError(error_string))
                }
            },
        }
    }

    pub async fn send(self: &mut Self, topic: &str, content: String<32>) -> Result<(), MqttError> {
        let result = self.client.send_message(
            topic,
            content.as_bytes(),
            rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS0,
        true).await;

        result.map_err(|e| {
            let mut error_string: String<32> = String::new();
            write!(error_string, "{:?}", e).expect("Mqtt error format failed!");
            MqttError::OtherError(error_string)
        })
    }
}