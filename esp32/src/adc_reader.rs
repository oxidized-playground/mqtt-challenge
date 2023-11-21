use esp_println::println;
use hal::{adc::{ADC, ADC2, AdcPin}, gpio::{Analog, GpioPin}, prelude::_embedded_hal_adc_OneShot};



pub struct AdcReader {

    adc2: ADC<'static, ADC2>,
    pin: AdcPin<GpioPin<Analog, 15>, ADC2>

}

impl AdcReader {
    pub fn new(adc2: ADC<'static, ADC2>, pin: AdcPin<GpioPin<Analog, 15>, ADC2>) -> Self {
        Self {
            adc2, pin
        }
    }

    pub fn read(&mut self) -> u16 {
        let adc_value: u16 = nb::block!(self.adc2.read(&mut self.pin)).unwrap();
        println!("Current adc value: {}", adc_value);
        adc_value
    }
}