#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::saadc::{CallbackResult, ChannelConfig, Config, Gain, Saadc};
use embassy_nrf::timer::Frequency;
use embassy_nrf::{bind_interrupts, saadc};
use embassy_sync::blocking_mutex::raw::{NoopRawMutex};
use embassy_sync::channel::{Channel, Receiver};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
});


static CHANNEL: StaticCell<Channel<NoopRawMutex, i16, 10>> = StaticCell::new();

#[embassy_executor::task]
async fn blink_led(receiver: Receiver<'static, NoopRawMutex, i16, 10>, mut pin: Output<'static>) {
    loop {
        let data = receiver.receive().await;
        if data > 100 {
            pin.set_high();
        } else {
            pin.set_low();
        }
    }
}

#[embassy_executor::main]
async fn main(s: Spawner) {
    let channel = CHANNEL.init(Channel::new());
    let receiver = channel.receiver();
    let sender = channel.sender();
    let p = embassy_nrf::init(Default::default());
    let _mic_pwr = Output::new(p.P0_20, Level::High, OutputDrive::Standard);
    let mut config = Config::default();
    config.resolution = saadc::Resolution::_12BIT;
    let mut channel_config = ChannelConfig::single_ended(p.P0_05);
    channel_config.gain = Gain::GAIN4;
    let mut saadc = Saadc::new(p.SAADC, Irqs, config, [channel_config]);
    let _col1 = Output::new(p.P0_28, Level::Low, OutputDrive::Standard);
    let row1 = Output::new(p.P0_21, Level::Low, OutputDrive::Standard);
    let mut bufs = [[[0; 1]; 512]; 2];
    saadc.calibrate().await;

    s.spawn(blink_led(receiver, row1).unwrap());

    saadc
        .run_task_sampler(
            p.TIMER0,
            p.PPI_CH0,
            p.PPI_CH1,
            Frequency::F16MHz,
            1000,
            &mut bufs,
            |b| {
                let max = b.iter().map(|a| a[0]).max().unwrap_or(0);
                let min = b.iter().map(|a| a[0]).min().unwrap_or(0);
                let amplitude = max - min;
                let _ = sender.try_send(amplitude);
                info!("
                    Max: {}, Min: {}
                ", max, min);
                CallbackResult::Continue
            },
        )
        .await;
}
