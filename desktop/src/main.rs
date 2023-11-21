use std::io::Error;
use tokio::net::TcpStream;
use mqtt_core::add;

use mqtt_core::writers::MqttWriter::MqttWriter;

use embedded_svc::io::asynch::{Read, Write};
use embedded_svc::io::{Io, ReadExactError};


#[tokio::main]
async fn main() -> Result<(), Error> {

    let mut recv_buffer = [0; 80];
    let mut write_buffer = [0; 80];
    let socket = TcpStream::connect("127.0.0.1:8080").await?;
    let mut mqtt_writer = MqttWriter::new("clientId-8rhWgBODCl", socket, &mut write_buffer, 80, &mut recv_buffer, 80);

    Ok(())
}
