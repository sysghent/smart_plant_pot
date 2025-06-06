use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::{
    interrupt::typelevel::Binding,
    peripherals::{self, USB},
    usb::{Driver, InterruptHandler},
};
use embassy_usb::{
    UsbDevice,
    class::cdc_acm::{CdcAcmClass, State},
};

const MAX_USB_PACKET_SIZE: usize = 64;

use static_cell::StaticCell;

pub type StaticUsbDriver = Driver<'static, USB>;
pub type StaticUsbDevice = UsbDevice<'static, StaticUsbDriver>;

pub struct BasicUsbSetup {
    pub usb_runtime: StaticUsbDevice,
    pub usb_io_handle: CdcAcmClass<'static, StaticUsbDriver>,
}

impl BasicUsbSetup {
    pub fn new(
        usb_pin: USB,
        irqs_binding: impl Binding<
            <embassy_rp::peripherals::USB as embassy_rp::usb::Instance>::Interrupt,
            InterruptHandler<peripherals::USB>,
        >,
    ) -> Self {
        // Create the driver, from the HAL.
        let usb_driver = Driver::new(usb_pin, irqs_binding);

        // Create embassy-usb Config
        let usb_config = {
            let mut usb_config: embassy_usb::Config<'static> =
                embassy_usb::Config::new(0xc0de, 0xcafe);
            usb_config.manufacturer = Some("SysGhent");
            usb_config.product = Some("Plant Watering System");
            usb_config.serial_number = Some("00000000");
            usb_config.max_power = 100;
            usb_config.max_packet_size_0 = 64;
            usb_config
        };

        // Create embassy-usb DeviceBuilder using the driver and config.
        // It needs some buffers for building the descriptors.
        let mut usb_runtime_builder = {
            static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
            static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
            static CONTROL_BUF: StaticCell<[u8; MAX_USB_PACKET_SIZE]> = StaticCell::new();

            embassy_usb::Builder::new(
                usb_driver,
                usb_config,
                CONFIG_DESCRIPTOR.init([0; 256]),
                BOS_DESCRIPTOR.init([0; 256]),
                &mut [], // no msos descriptors
                CONTROL_BUF.init([0; MAX_USB_PACKET_SIZE]),
            )
        };

        info!("Create classes on the builder.");
        let usb_io_handle = {
            static USB_STATE: StaticCell<State> = StaticCell::new();
            let usb_state_ref = USB_STATE.init(State::new());
            #[allow(clippy::cast_possible_truncation)]
            CdcAcmClass::new(
                &mut usb_runtime_builder,
                usb_state_ref,
                MAX_USB_PACKET_SIZE as u16,
            )
        };

        info!("Building the builder.");
        let usb_runtime = usb_runtime_builder.build();
        info!("Done building the builder.");
        Self {
            usb_runtime,
            usb_io_handle,
        }
    }

    pub async fn process(
        self,
        mut process: impl AsyncFnMut(&[u8], &mut [u8]) + 'static,
        spawner: Spawner,
    ) -> ! {
        let mut serial_in_buf = [0u8; MAX_USB_PACKET_SIZE];
        let mut serial_out_buf = [0u8; MAX_USB_PACKET_SIZE];

        let Self {
            mut usb_io_handle,
            usb_runtime,
        } = self;

        spawner.spawn(maintain_usb_connection(usb_runtime)).unwrap();

        loop {
            match usb_io_handle.read_packet(&mut serial_in_buf).await {
                Ok(n) => {
                    // TODO: Wait to reply until a newline is received.
                    info!("Received {} bytes from USB", n);
                    process(&serial_in_buf[0..n], &mut serial_out_buf).await;
                    info!("Sending {} bytes to USB", serial_out_buf.len());
                    let _ = usb_io_handle.write_packet(&serial_out_buf).await;
                    serial_in_buf.fill(0);
                    serial_out_buf.fill(0);
                }
                Err(_) => todo!("Handle USB read error"),
            }
        }
    }

    pub async fn receive(self, mut parse: impl AsyncFnMut(&[u8]) + 'static, spawner: Spawner) -> ! {
        let mut serial_in_buf = [0u8; MAX_USB_PACKET_SIZE];

        let Self {
            mut usb_io_handle,
            usb_runtime,
        } = self;

        spawner.spawn(maintain_usb_connection(usb_runtime)).unwrap();

        loop {
            match usb_io_handle.read_packet(&mut serial_in_buf).await {
                Ok(n) => {
                    parse(&serial_in_buf[0..n]).await;
                    serial_in_buf.fill(0);
                }
                Err(_) => todo!("Handle USB read error"),
            }
        }
    }

    pub async fn send(
        self,
        mut wait_ready_send: impl (AsyncFnMut(&mut [u8]) -> ()),
        spawner: Spawner,
    ) -> ! {
        let Self {
            mut usb_io_handle,
            usb_runtime,
        } = self;

        spawner.spawn(maintain_usb_connection(usb_runtime)).unwrap();

        let mut serial_out_buf = [0u8; MAX_USB_PACKET_SIZE];

        loop {
            wait_ready_send(&mut serial_out_buf).await;

            let _ = usb_io_handle.write_packet(&serial_out_buf).await;
            serial_out_buf.fill(0);
        }
    }
}

#[embassy_executor::task]
pub async fn maintain_usb_connection(mut usb_runtime: StaticUsbDevice) -> ! {
    usb_runtime.run().await
}
