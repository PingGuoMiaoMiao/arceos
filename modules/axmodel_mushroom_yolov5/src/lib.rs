#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::cmp::Ordering;

#[cfg(feature = "hardware")]
pub mod engine;

pub const MODEL_WIDTH: usize = 640;
pub const MODEL_HEIGHT: usize = 640;
pub const MODEL_CHANNELS: usize = 3;
pub const MODEL_INPUT_LENGTH: usize = MODEL_WIDTH * MODEL_HEIGHT * MODEL_CHANNELS;
pub const MODEL_NAME: &str = "mushroom_yolov5s_cv181x_int8_sym";
pub const PAGE_SIZE: usize = 4096;
pub const TOTAL_MEMORY_SIZE: usize = 20 * 1024 * 1024;
pub const SHARED_SIZE: usize = 5_734_400;
pub const PRIVATE_SIZE: usize = MODEL_INPUT_LENGTH;
pub const INPUT_FP32_SIZE: usize = 4_915_200;
pub const OUTPUT_20_SIZE: usize = 28_800;
pub const OUTPUT_80_SIZE: usize = 460_800;
pub const OUTPUT_40_SIZE: usize = 115_200;
pub const CONFIDENCE_THRESHOLD: f32 = 0.25;
pub const IOU_THRESHOLD: f32 = 0.45;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceImageMeta {
    pub source_width: u32,
    pub source_height: u32,
    pub resized_width: u32,
    pub resized_height: u32,
    pub pad_x: u32,
    pub pad_y: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceImageMetaError {
    ZeroDimension { width: u32, height: u32 },
    UnrepresentableLetterbox { width: u32, height: u32 },
}

impl SourceImageMeta {
    pub fn centered_letterbox(
        source_width: u32,
        source_height: u32,
    ) -> Result<Self, SourceImageMetaError> {
        if source_width == 0 || source_height == 0 {
            return Err(SourceImageMetaError::ZeroDimension {
                width: source_width,
                height: source_height,
            });
        }

        let (resized_width, resized_height) = if source_width >= source_height {
            (
                MODEL_WIDTH as u32,
                ((source_height as u64 * MODEL_WIDTH as u64) / source_width as u64) as u32,
            )
        } else {
            (
                ((source_width as u64 * MODEL_HEIGHT as u64) / source_height as u64) as u32,
                MODEL_HEIGHT as u32,
            )
        };
        if resized_width == 0 || resized_height == 0 {
            return Err(SourceImageMetaError::UnrepresentableLetterbox {
                width: source_width,
                height: source_height,
            });
        }

        Ok(Self {
            source_width,
            source_height,
            resized_width,
            resized_height,
            pad_x: (MODEL_WIDTH as u32 - resized_width) / 2,
            pad_y: (MODEL_HEIGHT as u32 - resized_height) / 2,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detection {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub confidence: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct RgbChw640<'a> {
    pub meta: SourceImageMeta,
    pub bytes: &'a [u8; MODEL_INPUT_LENGTH],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InferenceTiming {
    pub quantize_us: u64,
    pub tpu_us: u64,
    pub output_sync_us: u64,
    pub postprocess_us: u64,
    pub total_us: u64,
}

#[derive(Debug)]
pub struct InferenceResult {
    pub detections: Vec<Detection>,
    pub timing: InferenceTiming,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryLayout {
    pub dmabuf: usize,
    pub weight: usize,
    pub shared: usize,
    pub private: usize,
    pub input_fp32: usize,
    pub output_20: usize,
    pub output_80: usize,
    pub output_40: usize,
    pub used: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutError {
    ArithmeticOverflow,
    ExceedsReservedMemory { required: usize, available: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelOutputError {
    OutputLength {
        output: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl MemoryLayout {
    pub fn new(dmabuf_size: usize, weight_size: usize) -> Result<Self, LayoutError> {
        let dmabuf = 0;
        let weight = checked_align_page(checked_add(dmabuf, dmabuf_size)?)?;
        let shared = checked_align_page(checked_add(weight, weight_size)?)?;
        let private = checked_align_page(checked_add(shared, SHARED_SIZE)?)?;
        let input_fp32 = checked_align_page(checked_add(private, PRIVATE_SIZE)?)?;
        let output_20 = checked_align_page(checked_add(input_fp32, INPUT_FP32_SIZE)?)?;
        let output_80 = checked_align_page(checked_add(output_20, OUTPUT_20_SIZE)?)?;
        let output_40 = checked_align_page(checked_add(output_80, OUTPUT_80_SIZE)?)?;
        let used = checked_align_page(checked_add(output_40, OUTPUT_40_SIZE)?)?;
        if used > TOTAL_MEMORY_SIZE {
            return Err(LayoutError::ExceedsReservedMemory {
                required: used,
                available: TOTAL_MEMORY_SIZE,
            });
        }
        Ok(Self {
            dmabuf,
            weight,
            shared,
            private,
            input_fp32,
            output_20,
            output_80,
            output_40,
            used,
        })
    }
}

pub fn quantize_rgb_u8_in_place(bytes: &mut [u8]) {
    for value in bytes {
        *value = ((*value as u16 * 127 + 127) / 255) as u8;
    }
}

pub fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            let low_bit_mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & low_bit_mask);
        }
    }
    !crc
}

pub fn map_detection_to_source(detection: Detection, meta: SourceImageMeta) -> Detection {
    let source_width = meta.source_width as f32;
    let source_height = meta.source_height as f32;
    let scale_x = source_width / meta.resized_width as f32;
    let scale_y = source_height / meta.resized_height as f32;
    Detection {
        x1: ((detection.x1 - meta.pad_x as f32) * scale_x).clamp(0.0, source_width),
        y1: ((detection.y1 - meta.pad_y as f32) * scale_y).clamp(0.0, source_height),
        x2: ((detection.x2 - meta.pad_x as f32) * scale_x).clamp(0.0, source_width),
        y2: ((detection.y2 - meta.pad_y as f32) * scale_y).clamp(0.0, source_height),
        confidence: detection.confidence,
    }
}

pub fn intersection_over_union(a: Detection, b: Detection) -> f32 {
    let width = (a.x2.min(b.x2) - a.x1.max(b.x1)).max(0.0);
    let height = (a.y2.min(b.y2) - a.y1.max(b.y1)).max(0.0);
    let intersection = width * height;
    let area_a = (a.x2 - a.x1).max(0.0) * (a.y2 - a.y1).max(0.0);
    let area_b = (b.x2 - b.x1).max(0.0) * (b.y2 - b.y1).max(0.0);
    intersection / (area_a + area_b - intersection + 1.0e-7)
}

pub fn non_maximum_suppression(mut detections: Vec<Detection>) -> Vec<Detection> {
    detections.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(Ordering::Equal)
    });
    let mut kept = Vec::new();
    for detection in detections {
        if kept
            .iter()
            .all(|existing| intersection_over_union(detection, *existing) <= IOU_THRESHOLD)
        {
            kept.push(detection);
        }
    }
    kept
}

pub fn decode_model_outputs(
    output_80: &[f32],
    output_40: &[f32],
    output_20: &[f32],
) -> Result<Vec<Detection>, ModelOutputError> {
    check_output_length("output_80", output_80, OUTPUT_80_SIZE / size_of::<f32>())?;
    check_output_length("output_40", output_40, OUTPUT_40_SIZE / size_of::<f32>())?;
    check_output_length("output_20", output_20, OUTPUT_20_SIZE / size_of::<f32>())?;

    let mut detections = Vec::new();
    decode_scale(
        output_80,
        80,
        8.0,
        [[10.0, 13.0], [16.0, 30.0], [33.0, 23.0]],
        &mut detections,
    );
    decode_scale(
        output_40,
        40,
        16.0,
        [[30.0, 61.0], [62.0, 45.0], [59.0, 119.0]],
        &mut detections,
    );
    decode_scale(
        output_20,
        20,
        32.0,
        [[116.0, 90.0], [156.0, 198.0], [373.0, 326.0]],
        &mut detections,
    );
    Ok(non_maximum_suppression(detections))
}

fn check_output_length(
    output_name: &'static str,
    output: &[f32],
    expected: usize,
) -> Result<(), ModelOutputError> {
    if output.len() != expected {
        return Err(ModelOutputError::OutputLength {
            output: output_name,
            expected,
            actual: output.len(),
        });
    }
    Ok(())
}

fn decode_scale(
    output: &[f32],
    side: usize,
    stride: f32,
    anchors: [[f32; 2]; 3],
    detections: &mut Vec<Detection>,
) {
    for (anchor_index, anchor) in anchors.iter().enumerate() {
        for y in 0..side {
            for x in 0..side {
                let base = ((anchor_index * side + y) * side + x) * 6;
                let objectness = sigmoid(output[base + 4]);
                let class_probability = sigmoid(output[base + 5]);
                let confidence = objectness * class_probability;
                if confidence < CONFIDENCE_THRESHOLD {
                    continue;
                }
                let center_x = (sigmoid(output[base]) * 2.0 - 0.5 + x as f32) * stride;
                let center_y = (sigmoid(output[base + 1]) * 2.0 - 0.5 + y as f32) * stride;
                let width_factor = sigmoid(output[base + 2]) * 2.0;
                let height_factor = sigmoid(output[base + 3]) * 2.0;
                let width = width_factor * width_factor * anchor[0];
                let height = height_factor * height_factor * anchor[1];
                detections.push(Detection {
                    x1: center_x - width / 2.0,
                    y1: center_y - height / 2.0,
                    x2: center_x + width / 2.0,
                    y2: center_y + height / 2.0,
                    confidence,
                });
            }
        }
    }
}

fn sigmoid(value: f32) -> f32 {
    1.0 / (1.0 + libm::expf(-value))
}

fn checked_add(left: usize, right: usize) -> Result<usize, LayoutError> {
    left.checked_add(right)
        .ok_or(LayoutError::ArithmeticOverflow)
}

fn checked_align_page(value: usize) -> Result<usize, LayoutError> {
    value
        .checked_add(PAGE_SIZE - 1)
        .map(|value| value & !(PAGE_SIZE - 1))
        .ok_or(LayoutError::ArithmeticOverflow)
}
