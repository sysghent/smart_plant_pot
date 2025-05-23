#![no_std]

pub mod control;
pub mod usb_input;

use defmt_rtt as _;
use embassy_rp::{bind_interrupts, peripherals::USB};

use panic_probe as _;

bind_interrupts!(
    pub struct Irqs {
        USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<USB>;
    }
);
