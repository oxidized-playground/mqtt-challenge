#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod writers;

use adc_reader::AdcReader;
// peripherals-related imports
use hal::{
    adc::{AdcConfig, Attenuation, ADC, ADC2, self},
    clock::{ClockControl, CpuClock},
    peripherals::{Interrupt, Peripherals},
    prelude::*,
    timer::TimerGroup,
    Rng, Rtc, IO, {embassy, interrupt}, analog::AvailableAnalog,
};

// Wifi-related imports
use embedded_svc::wifi::{ClientConfiguration, Configuration, Wifi};
use esp_wifi::{
    wifi::{WifiController, WifiDevice, WifiEvent, WifiMode, WifiState},
    {initialize, EspWifiInitFor},
};

// embassy related imports
use embassy_executor::{Executor, _export::StaticCell};
use embassy_net::{
    tcp::TcpSocket,
    {Config, Stack, StackResources},
};
use embassy_time::{Duration, Timer};

// Formatting related imports
use core::fmt::Write;
use heapless::String;

use esp_backtrace as _;
use esp_println::println;
use hal::adc::AdcPin;
use hal::gpio::{Analog, GpioPin};

use crate::writers::MqttWriter::MqttWriter;

mod adc_reader;

static EXECUTOR: StaticCell<Executor> = StaticCell::new();


const MQTT_ADDRESS: [u8; 4] = [];

macro_rules! singleton {
    ($val:expr) => {{
        type T = impl Sized;
        static STATIC_CELL: StaticCell<T> = StaticCell::new();
        let (x,) = STATIC_CELL.init(($val,));
        x
    }};
}

#[entry]
fn main() -> ! {
    let peripherals = Peripherals::take();

    let mut system = peripherals.DPORT.split();
    let clocks = ClockControl::configure(system.clock_control, CpuClock::Clock240MHz).freeze();
    let mut rtc = Rtc::new(peripherals.RTC_CNTL);

    let timer = TimerGroup::new(
        peripherals.TIMG1,
        &clocks,
        &mut system.peripheral_clock_control,
    )
    .timer0;
    rtc.rwdt.disable();

    let init = initialize(
        EspWifiInitFor::Wifi,
        timer,
        Rng::new(peripherals.RNG),
        system.radio_clock_control,
        &clocks,
    )
    .expect("Failed to initialize Wifi");

    embassy::init(
        &clocks,
        TimerGroup::new(
            peripherals.TIMG0,
            &clocks,
            &mut system.peripheral_clock_control,
        )
        .timer0,
    );

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let (wifi, _) = peripherals.RADIO.split();
    let (wifi_interface, controller) =
        match esp_wifi::wifi::new_with_mode(&init, wifi, WifiMode::Sta) {
            Ok((wifi_interface, controller)) => (wifi_interface, controller),
            Err(..) => panic!("WiFi mode Error!"),
        };

    let config = Config::dhcpv4(Default::default());
    let seed = 1234; // very random, very secure seed

    // Init network stack
    let stack = &*singleton!(Stack::new(
        wifi_interface,
        config,
        singleton!(StackResources::<3>::new()),
        seed
    ));

    // Create ADC instances

    
    let analog = peripherals.SENS.split();


    let mut adc2_config = AdcConfig::new();
    let mut pin15 = adc2_config.enable_pin(io.pins.gpio15.into_analog(), Attenuation::Attenuation11dB);
    let mut adc2 = ADC::<ADC2>::adc(analog.adc2, adc2_config).unwrap();
{
    // let adc_value: u16 = nb::block!(adc2.read(pin)).unwrap();
    let adc_value: u16 = nb::block!(adc2.read(&mut pin15)).unwrap();
    println!("Current adc value: {}", adc_value);
}

    // let mut reader = adc_reader::AdcReader::new(adc2, pin15);


    interrupt::enable(Interrupt::I2C_EXT0, interrupt::Priority::Priority1)
        .expect("Invalid Interrupt Priority Error");

    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(connection(controller)).ok();
        spawner.spawn(net_task(&stack)).ok();
        // Add another spawn for `task`
    });
}
// maintains wifi connection, when it disconnects it tries to reconnect
#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.get_capabilities());
    loop {
        match esp_wifi::wifi::get_wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.into(),
                password: PASSWORD.into(),
                ..Default::default()
            });

            match controller.set_configuration(&client_config) {
                Ok(()) => {}
                Err(e) => {
                    println!("Failed to connect to wifi: {e:?}");
                    continue;
                }
            }
            println!("Starting wifi");
            match controller.start().await {
                Ok(()) => {}
                Err(e) => {
                    println!("Failed to connect to wifi: {e:?}");
                    continue;
                }
            }
            println!("Wifi started!");
        }
        println!("About to connect...");

        match controller.connect().await {
            Ok(_) => println!("Wifi connected!"),
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

// A background task, to process network events - when new packets, they need to processed, embassy-net, wraps smoltcp
#[embassy_executor::task]
async fn net_task(stack: &'static Stack<WifiDevice<'static>>) {
    stack.run().await
}

#[embassy_executor::task]
async fn task(stack: &'static Stack<WifiDevice<'static>>, mut reader: adc_reader::AdcReader) {
    // async fn task(stack: &'static Stack<WifiDevice<'static>>, mut adc2: ADC<'static, ADC2>, mut pin: AdcPin<GpioPin<Analog, 15>, ADC2>) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

    //wait until wifi connected
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("Got IP: {}", config.address); //dhcp IP address
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    loop {
        Timer::after(Duration::from_millis(1_000)).await;

        let mut socket = TcpSocket::new(&stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));
        let address: embassy_net::Ipv4Address = todo!("Get the address from constant above");


        let remote_endpoint = (address, 1883);
        println!("connecting to mqtt...");
        let connection = socket.connect(remote_endpoint).await;
        if let Err(e) = connection {
            println!("connect error: {:?}", e);
            continue;
        }
        println!("connected to mqtt!");

        // Todo make some buffers for data, see usage below

        let mut mqtt_writer = MqttWriter::new(todo!("Make a name for your client"), socket, &mut write_buffer, 80, &mut recv_buffer, 80);
        match mqtt_writer.connect().await {
            Err(e) => continue,
            _ => {}
        }

        loop {
            // TODO: Get analog value
            let adc_value: u16 = 10;

            // TODO: Convert temperature into String
            


            // TODO: Send to mqtt_writer, dont forget your async behaviour
            

            match result {
                Ok(_) => {println!("published temperature!");}
                Err(_) => {println!("failed to send temperature");}
            };

            Timer::after(Duration::from_millis(3000)).await;
        }
    }
}

pub async fn sleep(millis: u32) {
    Timer::after(Duration::from_millis(millis as u64)).await;
}
