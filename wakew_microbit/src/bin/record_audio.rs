#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_nrf::saadc::CallbackResult;
use embassy_nrf::timer::Frequency;
use embassy_nrf::{bind_interrupts, saadc};
use wakew_microbit::utils::prepare_mic_saadc;
use panic_probe as _;
use rtt_target::rprint;

const SAMPLES: usize = 18000;

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
});

#[embassy_executor::main]
async fn main(_s: Spawner) {
    rtt_target::rtt_init_print!(rtt_target::ChannelMode::BlockIfFull, 16000);
    let p = embassy_nrf::init(Default::default());
    let (mut saadc, _mic_pwr) = prepare_mic_saadc(p.SAADC, p.P0_20, p.P0_05, Irqs);
    // DMA buffers
    let mut dma_bufs = [[[0; 1]; 512]; 2];
    let mut final_buf = [0i16; SAMPLES];
    let mut written: usize = 0;

    saadc.calibrate().await;

    saadc
        .run_task_sampler(
            p.TIMER0,
            p.PPI_CH0,
            p.PPI_CH1,
            Frequency::F16MHz,
            1000,
            &mut dma_bufs,
            |b| {
                let offset = b.len().min(SAMPLES - written);
                final_buf[written..(written + offset)]
                    .as_mut()
                    .iter_mut()
                    .enumerate()
                    .for_each(|(i, v)| *v = b[i][0] << 4i16);
                written += offset;

                if written < final_buf.len() {
                    CallbackResult::Continue
                } else {
                    CallbackResult::Stop
                }
            },
        )
        .await;
    for (_i, v) in final_buf.into_iter().enumerate() {
        let b = v.to_le_bytes();
        rprint!("{:02x}{:02x}", b[0], b[1]);
    }
    rprint!("\n\n");
}
