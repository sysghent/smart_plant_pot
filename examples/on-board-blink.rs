#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::config::Config;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::InterruptHandler;
use embassy_time::{Duration, Timer};
use plant_pot::wifi::EasyWifi;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Config::default());

    let wifi_conf = EasyWifi {
        pio: p.PIO0,
        pwr_pin_23: p.PIN_23,
        cs_pin_25: p.PIN_25,
        dio_pin_24: p.PIN_24,
        clk_pin_29: p.PIN_29,
        dma_ch_0: p.DMA_CH0,
    };
    let mut control = wifi_conf.setup_wifi_controller(spawner, Irqs).await;

    let delay = Duration::from_secs(1);
    loop {
        info!("led on!");
        control.gpio_set(0, true).await;
        Timer::after(delay).await;

        info!("led off!");
        control.gpio_set(0, false).await;
        Timer::after(delay).await;
    }
}
