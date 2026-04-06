#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::saadc::{CallbackResult, ChannelConfig, Config, Gain, Saadc};
use embassy_nrf::timer::Frequency;
use embassy_nrf::{bind_interrupts, saadc};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Channel, Receiver};
use embassy_time::Instant;
use static_cell::StaticCell;
use wakew::dtw::dtw;
use wakew::mfcc::{FRAME_SIZE, Mfcc, NUM_MFCC, SHIFT_WIDTH};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
});

include!("../reference.rs");

const WINDOW_SIZE: usize = (24000 - FRAME_SIZE + SHIFT_WIDTH -1) / SHIFT_WIDTH;

static CHANNEL: StaticCell<Channel<NoopRawMutex, [f32; NUM_MFCC], 180>> = StaticCell::new();

struct MfccRingBuffer<const B: usize, const S: usize, T> {
    b: [T; B],
    start: usize,
    last_read: usize,
}

impl<const B: usize, const S: usize, T: Copy> MfccRingBuffer<B, S, T> {
    pub fn new(init: T) -> Self {
        let b = [init; B];
        let start = 0;
        let last_read = 0;
        MfccRingBuffer {
            b,
            start,
            last_read,
        }
    }

    pub fn update(&mut self, u: &[T]) {
        for i in 0..u.len() {
            self.b[(self.start + i) % B] = u[i];
        }
        self.start = (self.start + u.len()) % B;
    }

    pub fn frame(&mut self) -> Option<[T; B]> {
        let start = if self.start >= self.last_read {
            self.start
        } else {
            self.start + B
        };
        let to_read = start - self.last_read;
        if to_read > S {
            self.last_read = (self.last_read + S) % B;
            Some(core::array::from_fn(|i| {
                self.b[(self.last_read + i) % B]
            }))
        } else {
            None
        }
    }
}

#[embassy_executor::task]
async fn blink_led(receiver: Receiver<'static, NoopRawMutex, [f32; NUM_MFCC], 180>, mut pin: Output<'static>) {
    let mut buf = MfccRingBuffer::<WINDOW_SIZE, 180, [f32; NUM_MFCC]>::new([0f32; NUM_MFCC]);
    loop {
        let data = receiver.receive().await;
        buf.update(&[data]);
        if let Some(window) = buf.frame() {
            let start = Instant::now();
            let distance = dtw(&REFERENCE, &window).await;
            let elapsed = start.elapsed().as_millis();
            info!("Duration took {} ms", elapsed);
            info!("Distance: {}", distance);
            if distance < 0.04 {
                pin.toggle();
            }
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
    let mut bufs = [[[0; 1]; 100]; 2];
    let mut mfcc_buffer = MfccRingBuffer::<FRAME_SIZE, SHIFT_WIDTH, f32>::new(0f32);
    let mfcc = Mfcc::new();
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
                let mut u = [0f32; SHIFT_WIDTH];
                for i in 0..b.len() {
                    u[i] = b[i][0] as f32 / 32768f32;
                }
                mfcc_buffer.update(&u[0..b.len()]);
                while let Some(frame) = mfcc_buffer.frame() {
                    let mfcc_res = mfcc.mfcc(&frame);
                    let r = sender.try_send(mfcc_res);
                    if let Err(_) = r {
                        info!("Mccs were discarded!")
                    }
                }
                CallbackResult::Continue
            },
        )
        .await;
}
