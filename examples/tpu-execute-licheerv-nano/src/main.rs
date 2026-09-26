#![no_std]
#![no_main]

use axmodel_mushroom_yolov5::engine::TpuEngine;
use axmodel_mushroom_yolov5::{MODEL_INPUT_LENGTH, RgbChw640, SourceImageMeta, crc32_ieee};
use axstd::println;
use axstd::vec;

const INPUT_METADATA_WORDS: usize = 6;

fn receive_exact(bytes: &mut [u8]) {
    let mut received = 0;
    while received < bytes.len() {
        let count = axhal::console::read_bytes(&mut bytes[received..]);
        if count == 0 {
            core::hint::spin_loop();
        } else {
            received += count;
        }
    }
}

fn receive_metadata() -> SourceImageMeta {
    let mut metadata_bytes = [0_u8; INPUT_METADATA_WORDS * size_of::<u32>()];
    receive_exact(&mut metadata_bytes);
    let mut words = [0_u32; INPUT_METADATA_WORDS];
    for (index, word) in words.iter_mut().enumerate() {
        let offset = index * size_of::<u32>();
        *word = u32::from_le_bytes(
            metadata_bytes[offset..offset + size_of::<u32>()]
                .try_into()
                .unwrap(),
        );
    }
    let received = SourceImageMeta {
        source_width: words[0],
        source_height: words[1],
        resized_width: words[2],
        resized_height: words[3],
        pad_x: words[4],
        pad_y: words[5],
    };
    let expected =
        SourceImageMeta::centered_letterbox(received.source_width, received.source_height)
            .expect("invalid source image dimensions");
    assert_eq!(
        received, expected,
        "runtime RGB letterbox metadata mismatch"
    );
    received
}

#[unsafe(no_mangle)]
fn main() {
    println!("SG2002 mushroom TPU execution probe");
    let mut engine = TpuEngine::initialize().expect("failed to initialize reusable TPU engine");
    let layout = engine.layout();
    let physical_base = engine.physical_base();
    let init_timing = engine.initialization_timing();

    println!("READY RGB_U8 {MODEL_INPUT_LENGTH} META_U32 {INPUT_METADATA_WORDS}");
    let metadata = receive_metadata();
    let mut rgb_input = vec![0_u8; MODEL_INPUT_LENGTH];
    receive_exact(&mut rgb_input);
    let mut transmitted_crc_bytes = [0_u8; 4];
    receive_exact(&mut transmitted_crc_bytes);
    let transmitted_crc = u32::from_le_bytes(transmitted_crc_bytes);
    let received_crc = crc32_ieee(&rgb_input);
    assert_eq!(received_crc, transmitted_crc, "runtime RGB CRC32 mismatch");
    println!("RGB metadata = {:?}", metadata);
    println!("RGB_U8 received crc32={received_crc:08x}");

    println!("physical base             = {physical_base:#x}");
    println!("used bytes                = {}", layout.used);
    println!(
        "dmabuf/weight             = {:#x}/{:#x}",
        physical_base + layout.dmabuf,
        physical_base + layout.weight,
    );
    println!(
        "shared/private/input      = {:#x}/{:#x}/{:#x}",
        physical_base + layout.shared,
        physical_base + layout.private,
        physical_base + layout.input_fp32,
    );
    println!(
        "outputs 482/350/416       = {:#x}/{:#x}/{:#x}",
        physical_base + layout.output_20,
        physical_base + layout.output_80,
        physical_base + layout.output_40,
    );
    println!("submitting 3171 TIU and 6682 TDMA descriptors");

    let input_bytes: &[u8; MODEL_INPUT_LENGTH] = rgb_input
        .as_slice()
        .try_into()
        .expect("fixed RGB input length changed");
    let result = engine
        .infer(RgbChw640 {
            meta: metadata,
            bytes: input_bytes,
        })
        .expect("TPU inference failed");
    println!("TPU execution completed");

    let first_values = engine.output_first_values();
    println!("output 482 first values = {:?}", first_values[0]);
    println!("output 350 first values = {:?}", first_values[1]);
    println!("output 416 first values = {:?}", first_values[2]);
    println!("mushroom detections       = {}", result.detections.len());
    for (index, detection) in result.detections.iter().enumerate() {
        println!(
            "mushroom[{index}] confidence={:.6} source_xyxy=({:.2},{:.2},{:.2},{:.2})",
            detection.confidence, detection.x1, detection.y1, detection.x2, detection.y2,
        );
    }

    let measured_micros = init_timing.load_model_us
        + init_timing.prepare_dmabuf_us
        + init_timing.cache_and_hardware_us
        + result.timing.quantize_us
        + result.timing.tpu_us
        + result.timing.output_sync_us
        + result.timing.postprocess_us;
    println!("timing load/model us       = {}", init_timing.load_model_us);
    println!("timing RGB quantize us     = {}", result.timing.quantize_us);
    println!(
        "timing DMABUF prepare us   = {}",
        init_timing.prepare_dmabuf_us
    );
    println!(
        "timing cache/init us       = {}",
        init_timing.cache_and_hardware_us
    );
    println!("timing TPU execute us      = {}", result.timing.tpu_us);
    println!(
        "timing output sync us      = {}",
        result.timing.output_sync_us
    );
    println!(
        "timing decode/NMS us       = {}",
        result.timing.postprocess_us
    );
    println!("timing measured sum us     = {measured_micros}");
}
