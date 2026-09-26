use axalloc::GlobalPage;
use axhal::mem::virt_to_phys;
use axhal::time::monotonic_time_nanos;
use axplat_riscv64_licheerv_nano::tpu::{
    DmaBufferError, TpuExecutionError, cache_writeback_for_device, cache_writeback_invalidate,
    execute_dma_buffer, initialize_clocks_and_resets, relocate_dma_buffer, set_array_bases,
    validate_dma_buffer,
};

use crate::{
    InferenceResult, InferenceTiming, LayoutError, MODEL_INPUT_LENGTH, MemoryLayout,
    ModelOutputError, OUTPUT_20_SIZE, OUTPUT_40_SIZE, OUTPUT_80_SIZE, PAGE_SIZE, RgbChw640,
    TOTAL_MEMORY_SIZE, decode_model_outputs, map_detection_to_source, quantize_rgb_u8_in_place,
};

pub const MODEL_DMABUF: &[u8] = include_bytes!(
    "../../../examples/tpu-execute-licheerv-nano/mushroom_yolov5s_program_0_dmabuf_subfunc_1.bin"
);
pub const MODEL_WEIGHT: &[u8] =
    include_bytes!("../../../examples/tpu-execute-licheerv-nano/mushroom_yolov5s_weight.bin");

const _: () = assert!(MODEL_DMABUF.len() == 783_488);
const _: () = assert!(MODEL_WEIGHT.len() == 7_108_528);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitializationTiming {
    pub load_model_us: u64,
    pub prepare_dmabuf_us: u64,
    pub cache_and_hardware_us: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TpuEngineError {
    Layout(LayoutError),
    AllocationFailed,
    DmaBuffer(DmaBufferError),
    Execution(TpuExecutionError),
    Output(ModelOutputError),
}

impl From<LayoutError> for TpuEngineError {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}

impl From<DmaBufferError> for TpuEngineError {
    fn from(value: DmaBufferError) -> Self {
        Self::DmaBuffer(value)
    }
}

impl From<TpuExecutionError> for TpuEngineError {
    fn from(value: TpuExecutionError) -> Self {
        Self::Execution(value)
    }
}

impl From<ModelOutputError> for TpuEngineError {
    fn from(value: ModelOutputError) -> Self {
        Self::Output(value)
    }
}

pub struct TpuEngine {
    pages: GlobalPage,
    physical_base: usize,
    layout: MemoryLayout,
    initialization_timing: InitializationTiming,
}

impl TpuEngine {
    pub fn initialize() -> Result<Self, TpuEngineError> {
        let layout = MemoryLayout::new(MODEL_DMABUF.len(), MODEL_WEIGHT.len())?;

        let load_start = monotonic_time_nanos();
        let mut pages = GlobalPage::alloc_contiguous(TOTAL_MEMORY_SIZE / PAGE_SIZE, PAGE_SIZE)
            .map_err(|_| TpuEngineError::AllocationFailed)?;
        let physical_base = pages.start_paddr(virt_to_phys).as_usize();
        let memory = pages.as_slice_mut();
        memory[..layout.used].fill(0);
        memory[layout.dmabuf..layout.dmabuf + MODEL_DMABUF.len()].copy_from_slice(MODEL_DMABUF);
        memory[layout.weight..layout.weight + MODEL_WEIGHT.len()].copy_from_slice(MODEL_WEIGHT);
        let load_model_us = elapsed_micros(load_start);

        let prepare_start = monotonic_time_nanos();
        let dmabuf_physical = physical_base + layout.dmabuf;
        let array_bases = [
            physical_base + layout.shared,
            physical_base + layout.weight,
            physical_base + layout.private,
            physical_base + layout.input_fp32,
            physical_base + layout.output_20,
            physical_base + layout.output_80,
            physical_base + layout.output_40,
            0,
        ];
        {
            let dmabuf = &mut memory[layout.dmabuf..layout.dmabuf + MODEL_DMABUF.len()];
            validate_dma_buffer(dmabuf_physical, dmabuf)?;
            relocate_dma_buffer(dmabuf_physical, dmabuf)?;
            set_array_bases(dmabuf, array_bases)?;
        }
        let prepare_dmabuf_us = elapsed_micros(prepare_start);

        let hardware_start = monotonic_time_nanos();
        cache_writeback_for_device(physical_base, layout.used);
        initialize_clocks_and_resets();
        let cache_and_hardware_us = elapsed_micros(hardware_start);

        Ok(Self {
            pages,
            physical_base,
            layout,
            initialization_timing: InitializationTiming {
                load_model_us,
                prepare_dmabuf_us,
                cache_and_hardware_us,
            },
        })
    }

    pub const fn initialization_timing(&self) -> InitializationTiming {
        self.initialization_timing
    }

    pub const fn physical_base(&self) -> usize {
        self.physical_base
    }

    pub const fn layout(&self) -> MemoryLayout {
        self.layout
    }

    pub fn output_first_values(&mut self) -> [[f32; 4]; 3] {
        let memory = self.pages.as_slice_mut();
        let output_20 = f32_slice(memory, self.layout.output_20, OUTPUT_20_SIZE);
        let output_80 = f32_slice(memory, self.layout.output_80, OUTPUT_80_SIZE);
        let output_40 = f32_slice(memory, self.layout.output_40, OUTPUT_40_SIZE);
        [
            output_20[..4].try_into().unwrap(),
            output_80[..4].try_into().unwrap(),
            output_40[..4].try_into().unwrap(),
        ]
    }

    pub fn infer(&mut self, input: RgbChw640<'_>) -> Result<InferenceResult, TpuEngineError> {
        let inference_start = monotonic_time_nanos();
        let private_physical = self.physical_base + self.layout.private;

        let quantize_start = monotonic_time_nanos();
        {
            let memory = self.pages.as_slice_mut();
            let private =
                &mut memory[self.layout.private..self.layout.private + MODEL_INPUT_LENGTH];
            private.copy_from_slice(input.bytes);
            quantize_rgb_u8_in_place(private);
        }
        cache_writeback_for_device(private_physical, MODEL_INPUT_LENGTH);
        let quantize_us = elapsed_micros(quantize_start);

        let tpu_start = monotonic_time_nanos();
        {
            let memory = self.pages.as_slice_mut();
            let dmabuf = &memory[self.layout.dmabuf..self.layout.dmabuf + MODEL_DMABUF.len()];
            execute_dma_buffer(self.physical_base + self.layout.dmabuf, dmabuf)?;
        }
        let tpu_us = elapsed_micros(tpu_start);

        let sync_start = monotonic_time_nanos();
        cache_writeback_invalidate(self.physical_base + self.layout.output_20, OUTPUT_20_SIZE);
        cache_writeback_invalidate(self.physical_base + self.layout.output_80, OUTPUT_80_SIZE);
        cache_writeback_invalidate(self.physical_base + self.layout.output_40, OUTPUT_40_SIZE);
        let output_sync_us = elapsed_micros(sync_start);

        let postprocess_start = monotonic_time_nanos();
        let detections = {
            let memory = self.pages.as_slice_mut();
            let output_20 = f32_slice(memory, self.layout.output_20, OUTPUT_20_SIZE);
            let output_80 = f32_slice(memory, self.layout.output_80, OUTPUT_80_SIZE);
            let output_40 = f32_slice(memory, self.layout.output_40, OUTPUT_40_SIZE);
            decode_model_outputs(output_80, output_40, output_20)?
                .into_iter()
                .map(|detection| map_detection_to_source(detection, input.meta))
                .collect()
        };
        let postprocess_us = elapsed_micros(postprocess_start);
        let total_us = elapsed_micros(inference_start);

        Ok(InferenceResult {
            detections,
            timing: InferenceTiming {
                quantize_us,
                tpu_us,
                output_sync_us,
                postprocess_us,
                total_us,
            },
        })
    }
}

fn elapsed_micros(start_nanos: u64) -> u64 {
    (monotonic_time_nanos() - start_nanos) / 1_000
}

fn f32_slice(memory: &[u8], offset: usize, byte_length: usize) -> &[f32] {
    // SAFETY: every fixed output offset is page-aligned, the backing allocation
    // is page-aligned, and each byte length is a multiple of `size_of::<f32>()`.
    unsafe {
        core::slice::from_raw_parts(
            memory.as_ptr().add(offset) as *const f32,
            byte_length / size_of::<f32>(),
        )
    }
}
